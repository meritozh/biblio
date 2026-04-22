use std::path::Path;

use crate::pipeline::{FileContext, NodeError, Phase1Node, PipelineEnv};

/// Sample representative text from novel-like files (.txt / .epub / .pdf)
/// for downstream LLM classification. Writes to `ctx.content_sample`.
///
/// `.txt` is always eligible; `.epub` and `.pdf` are gated by the user's
/// per-format toggles in `PipelineEnv::settings`. When
/// `settings.analyze_content` is false the node is skipped entirely.
pub struct ContentSampleNode;

impl Phase1Node for ContentSampleNode {
    fn name(&self) -> &'static str {
        "ContentSample"
    }

    fn applies(&self, ctx: &FileContext, env: &PipelineEnv) -> bool {
        if !env.settings.analyze_content {
            return false;
        }
        is_novel_file(
            &ctx.file_path.to_string_lossy(),
            env.settings.process_novel_epub,
            env.settings.process_novel_pdf,
        )
    }

    fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        let mime = ctx.mime.as_deref().unwrap_or("");
        let ext_lower = ctx
            .file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase());

        let sample = if (ext_lower.as_deref() == Some("epub") || mime == "application/epub+zip")
            && env.settings.process_novel_epub
        {
            sample_epub_content(&ctx.file_path, 5, 1000)
        } else if (ext_lower.as_deref() == Some("pdf") || mime == "application/pdf")
            && env.settings.process_novel_pdf
        {
            sample_pdf_content(&ctx.file_path, 5, 1000)
        } else if mime == "text/plain" || ext_lower.as_deref() == Some("txt") {
            sample_text_content(&ctx.file_path, 5, 1000)
        } else {
            None
        };

        ctx.content_sample = sample;
        Ok(())
    }
}

/// Is this path a novel (text-based book) given the per-format toggles?
/// `.txt` is always eligible; `.epub` and `.pdf` are gated.
pub fn is_novel_file(path: &str, process_epub: bool, process_pdf: bool) -> bool {
    let lower = path.to_lowercase();
    if lower.ends_with(".txt") {
        return true;
    }
    if process_epub && lower.ends_with(".epub") {
        return true;
    }
    if process_pdf && lower.ends_with(".pdf") {
        return true;
    }
    false
}

/// Detect encoding and decode bytes to UTF-8.
/// Returns None on low detection confidence (<0.7) or an unknown encoding
/// label. Replacement characters during decode are accepted: for a large
/// real-world text file a single stray byte (mid-stream BOM, anomalous
/// codepoint, etc.) is common, and bailing on it would discard an
/// otherwise-usable body of text. The content is only used for LLM
/// sampling downstream, which is lossy anyway.
pub(super) fn decode_to_utf8(bytes: &[u8]) -> Option<String> {
    let detected = chardet::detect_bytes(bytes, chardet::EncodingEra::All, 200_000);
    if detected.confidence < 0.7 {
        return None;
    }

    let encoding_label = detected.encoding?;
    let encoding = encoding_rs::Encoding::for_label(encoding_label.as_bytes())?;
    let (text, _, _had_errors) = encoding.decode(bytes);
    Some(text.into_owned())
}

/// Pick `num_samples` evenly-spaced chunks of `sample_size` characters from
/// a string. Pure function — used by both .txt and .epub samplers so the
/// downstream LLM prompt receives a consistent shape.
pub(super) fn sample_from_text(
    content: &str,
    num_samples: usize,
    sample_size: usize,
) -> Option<String> {
    let total_chars: usize = content.chars().count();
    if total_chars == 0 {
        return None;
    }

    let total_needed = num_samples * sample_size;
    if total_chars <= total_needed {
        return Some(content.to_string());
    }

    let chars: Vec<char> = content.chars().collect();
    let mut result = String::new();
    let labels = ["Beginning", "25%", "50%", "75%", "Near End"];

    for i in 0..num_samples {
        let position = if i == num_samples - 1 {
            (total_chars as f64 * 0.95) as usize
        } else {
            (total_chars as f64 * (i as f64 / (num_samples - 1) as f64)) as usize
        };

        let start = position.min(total_chars.saturating_sub(sample_size));
        let end = (start + sample_size).min(total_chars);
        let sample: String = chars[start..end].iter().collect();

        let label = labels.get(i).unwrap_or(&"Sample");
        result.push_str(&format!("[Sample {} - {}]\n{}\n\n", i + 1, label, sample));
    }

    Some(result)
}

fn sample_text_content(file_path: &Path, num_samples: usize, sample_size: usize) -> Option<String> {
    let bytes = std::fs::read(file_path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let content = decode_to_utf8(&bytes)?;
    sample_from_text(&content, num_samples, sample_size)
}

/// Strip HTML/XML tags and decode the most common entities. Crude but
/// adequate for feeding EPUB body text to the content-analysis LLM — we
/// only need readable prose, not a faithful DOM.
pub(super) fn strip_html_tags(html: &str) -> String {
    let script_re = regex::Regex::new(r"(?is)<(script|style)[^>]*>.*?</\s*(script|style)\s*>")
        .expect("script/style regex is valid");
    let without_scripts = script_re.replace_all(html, " ");

    let tag_re = regex::Regex::new(r"<[^>]+>").expect("tag regex is valid");
    let no_tags = tag_re.replace_all(&without_scripts, " ");

    let decoded = no_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");

    let ws_re = regex::Regex::new(r"\s+").expect("whitespace regex is valid");
    ws_re.replace_all(&decoded, " ").trim().to_string()
}

/// Unzip an EPUB, collect the XHTML/HTML body files in archive order, strip
/// markup, and run the result through `sample_from_text`. Archive order
/// isn't guaranteed to be reading order, but for sampling-to-classify
/// purposes any contiguous prose is fine — the LLM doesn't need chapter
/// structure, just representative text.
fn sample_epub_content(file_path: &Path, num_samples: usize, sample_size: usize) -> Option<String> {
    let file = std::fs::File::open(file_path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;

    let mut html_names: Vec<String> = archive
        .file_names()
        .filter(|n| {
            let lower = n.to_lowercase();
            lower.ends_with(".xhtml") || lower.ends_with(".html") || lower.ends_with(".htm")
        })
        .map(String::from)
        .collect();
    html_names.sort();
    if html_names.is_empty() {
        return None;
    }

    // Collect enough decoded text to sample from — cap early so we don't
    // read the entire novel when the first couple of chapters will do.
    let target_chars = num_samples.saturating_mul(sample_size).saturating_mul(4);
    let mut text_buf = String::new();

    for name in &html_names {
        use std::io::Read;
        let Ok(mut entry) = archive.by_name(name) else {
            continue;
        };
        let mut raw = Vec::new();
        if entry.read_to_end(&mut raw).is_err() {
            continue;
        }
        let html = match String::from_utf8(raw) {
            Ok(s) => s,
            Err(e) => String::from_utf8_lossy(&e.into_bytes()).into_owned(),
        };
        let stripped = strip_html_tags(&html);
        if !stripped.is_empty() {
            text_buf.push_str(&stripped);
            text_buf.push('\n');
        }
        if text_buf.chars().count() >= target_chars {
            break;
        }
    }

    if text_buf.trim().is_empty() {
        return None;
    }
    sample_from_text(&text_buf, num_samples, sample_size)
}

/// Extract text from the first pages of a PDF via lopdf and sample it for
/// LLM content analysis. We cap the page count so a 1000-page scientific
/// PDF doesn't stall the import pipeline — the classifier only needs
/// representative prose, not the whole document.
fn sample_pdf_content(file_path: &Path, num_samples: usize, sample_size: usize) -> Option<String> {
    const MAX_PAGES: usize = 20;

    let doc = lopdf::Document::load(file_path).ok()?;
    let pages = doc.get_pages();
    let mut page_nums: Vec<u32> = pages.keys().copied().collect();
    page_nums.sort_unstable();
    if page_nums.is_empty() {
        return None;
    }
    page_nums.truncate(MAX_PAGES);

    let text = doc.extract_text(&page_nums).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    sample_from_text(&text, num_samples, sample_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_novel_file_txt_is_always_eligible() {
        assert!(is_novel_file("book.txt", true, true));
        assert!(is_novel_file("book.TXT", false, false));
        assert!(is_novel_file("/path/to/book.txt", false, false));
    }

    #[test]
    fn is_novel_file_epub_gated_by_toggle() {
        assert!(is_novel_file("book.epub", true, false));
        assert!(!is_novel_file("book.epub", false, false));
    }

    #[test]
    fn is_novel_file_pdf_gated_by_toggle() {
        assert!(is_novel_file("book.pdf", false, true));
        assert!(!is_novel_file("book.pdf", false, false));
        assert!(!is_novel_file("book.pdf", true, false));
    }

    #[test]
    fn is_novel_file_unsupported_rejected() {
        assert!(!is_novel_file("book.zip", true, true));
        assert!(!is_novel_file("book", true, true));
    }

    #[test]
    fn decode_plain_ascii() {
        let bytes = b"Hello, world!";
        assert_eq!(decode_to_utf8(bytes).as_deref(), Some("Hello, world!"));
    }

    #[test]
    fn decode_utf8_chinese() {
        let text = "你好，世界！这是一段中文测试内容，包含足够的字符让 chardet 有信心识别为 UTF-8。三体，刘慈欣。";
        let bytes = text.as_bytes();
        assert_eq!(decode_to_utf8(bytes).as_deref(), Some(text));
    }

    #[test]
    fn decode_gb18030_chinese() {
        let sample = "这是一段较长的中文测试内容，用于验证 GB18030 编码的文件能够被正确识别和转换为 UTF-8。内容包含了标点符号、数字 123、以及一些常见的汉字，比如：你好世界、春夏秋冬、日月星辰、山川河流。小说标题示例：三体、流浪地球、活着、平凡的世界。作者示例：刘慈欣、余华、路遥、莫言。";
        let full_text = sample.repeat(5);
        let gb18030 = encoding_rs::GB18030;
        let (encoded_bytes, _, had_errors) = gb18030.encode(&full_text);
        assert!(!had_errors);

        let decoded = decode_to_utf8(&encoded_bytes).expect("should decode");
        assert!(decoded.contains("三体") && decoded.contains("刘慈欣"));
    }

    #[test]
    fn decode_garbage_returns_none() {
        let bytes: Vec<u8> = (0..200).map(|i| (i as u8).wrapping_mul(31)).collect();
        assert!(decode_to_utf8(&bytes).is_none());
    }

    #[test]
    fn sample_from_text_empty_input() {
        assert_eq!(sample_from_text("", 5, 100), None);
    }

    #[test]
    fn sample_from_text_returns_full_content_when_short() {
        let text = "hello";
        assert_eq!(sample_from_text(text, 5, 10).as_deref(), Some("hello"));
    }

    #[test]
    fn sample_from_text_produces_labeled_samples_when_long() {
        let text: String = "x".repeat(10_000);
        let result = sample_from_text(&text, 5, 1000).expect("should sample");
        assert_eq!(result.matches("[Sample ").count(), 5);
        assert!(result.contains("Beginning"));
        assert!(result.contains("Near End"));
    }

    #[test]
    fn strip_html_removes_tags() {
        let html = "<p>Hello <em>world</em></p>";
        assert_eq!(strip_html_tags(html), "Hello world");
    }

    #[test]
    fn strip_html_drops_script_and_style() {
        let html = "<p>keep</p><script>var x = 1; if (a < b) { /* drop */ }</script><p>keep too</p>";
        let out = strip_html_tags(html);
        assert!(out.contains("keep"));
        assert!(out.contains("keep too"));
        assert!(!out.contains("var x"));
    }

    #[test]
    fn strip_html_decodes_entities() {
        let html = "<p>Tom &amp; Jerry &nbsp;&quot;hi&quot;</p>";
        assert_eq!(strip_html_tags(html), "Tom & Jerry \"hi\"");
    }

    #[test]
    fn strip_html_collapses_whitespace() {
        let html = "<p>line1</p>\n\n<p>line2</p>";
        assert_eq!(strip_html_tags(html), "line1 line2");
    }
}
