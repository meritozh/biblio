use std::path::Path;

use crate::pipeline::{FileContext, NodeError, Phase1Node, PipelineEnv};

/// Sample representative text from .txt novels for downstream LLM
/// classification. Writes to `ctx.content_sample`. When
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
        is_novel_file(&ctx.file_path.to_string_lossy())
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        let mime = ctx.mime.as_deref().unwrap_or("");
        let ext_lower = ctx
            .file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase());

        let sample = if mime == "text/plain" || ext_lower.as_deref() == Some("txt") {
            sample_text_content(&ctx.file_path, 5, 1000)
        } else {
            None
        };

        ctx.content_sample = sample;
        Ok(())
    }
}

/// Is this path a supported novel input? Only .txt qualifies after the
/// epub/pdf removal — kept as a helper so callers stay symmetric.
pub fn is_novel_file(path: &str) -> bool {
    path.to_lowercase().ends_with(".txt")
}

/// Detect encoding and decode bytes to UTF-8 via `charset-normalizer-rs`,
/// the Rust port of the Python `charset_normalizer` library. Returns
/// `None` when no candidate match clears the library's internal quality
/// thresholds (chaos + coherence scores).
///
/// The library already returns the decoded payload as a UTF-8 string, so
/// we don't run a second pass through `encoding_rs::Encoding::decode`.
/// Replacement characters in the decoded output are still tolerated:
/// for a large real-world text file a single stray byte is common, and
/// the downstream LLM sampling is lossy anyway.
pub fn decode_to_utf8(bytes: &[u8]) -> Option<String> {
    let matches = charset_normalizer_rs::from_bytes(bytes, None).ok()?;
    let best = matches.get_best()?;
    best.decoded_payload().map(|s| s.to_string())
}

/// Pick `num_samples` evenly-spaced chunks of `sample_size` characters from
/// a string. Pure function — kept generic so future text formats can reuse
/// the same shape.
pub fn sample_from_text(
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

/// Read a .txt file, decode to UTF-8, and produce a samples blob shaped
/// for LLM content classification. Exposed so the cleanup page's
/// "re-analyze novels with no tags" debug action can re-run the same
/// sampling the import pipeline uses.
pub fn sample_text_content(file_path: &Path, num_samples: usize, sample_size: usize) -> Option<String> {
    let bytes = std::fs::read(file_path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let content = decode_to_utf8(&bytes)?;
    sample_from_text(&content, num_samples, sample_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_novel_file_only_matches_txt() {
        assert!(is_novel_file("book.txt"));
        assert!(is_novel_file("book.TXT"));
        assert!(is_novel_file("/path/to/book.txt"));
        assert!(!is_novel_file("book.epub"));
        assert!(!is_novel_file("book.pdf"));
        assert!(!is_novel_file("book.zip"));
        assert!(!is_novel_file("book"));
    }

    #[test]
    fn decode_plain_ascii() {
        let bytes = b"Hello, world!";
        assert_eq!(decode_to_utf8(bytes).as_deref(), Some("Hello, world!"));
    }

    #[test]
    fn decode_utf8_chinese() {
        let text = "你好，世界！这是一段中文测试内容，包含足够的字符让编码探测器有信心识别为 UTF-8。三体，刘慈欣。";
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
}
