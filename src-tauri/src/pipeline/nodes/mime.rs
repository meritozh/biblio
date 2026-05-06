use crate::pipeline::{ExtractedField, FileContext, NodeError, Phase1Node, PipelineEnv};

/// Detect MIME type from the first few KB of a file (via `infer`), falling
/// back to the extension when the magic bytes don't match anything known.
/// Writes to `ctx.mime` and appends a `mime_type` extracted field.
pub struct MimeDetectNode;

impl Phase1Node for MimeDetectNode {
    fn name(&self) -> &'static str {
        "MimeDetect"
    }

    /// Skip when the source is a directory — the auto-zipped image-folder
    /// import has no single file to sniff, and the comic pipeline downstream
    /// doesn't need the mime to make decisions.
    fn applies(&self, ctx: &FileContext, _env: &PipelineEnv) -> bool {
        !ctx.file_path.is_dir()
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        // Read only the bytes `infer` actually inspects. The previous
        // `std::fs::read` slurped the whole file (multi-GB for comic
        // archives) — with 8-way Phase-1 concurrency that put enough
        // pressure on RAM to spill into macOS swap, which is why disk
        // usage grew per import and only recovered on reboot.
        use std::io::Read;
        let mut file = std::fs::File::open(&ctx.file_path)
            .map_err(|e| NodeError(format!("Failed to open file: {e}")))?;
        let mut buf = [0u8; 8192];
        let mut filled = 0usize;
        while filled < buf.len() {
            match file.read(&mut buf[filled..]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) => return Err(NodeError(format!("Failed to read file: {e}"))),
            }
        }
        let sample = &buf[..filled];

        let mime_type = match infer::get(sample) {
            Some(t) => t.mime_type().to_string(),
            None => ctx
                .file_path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| match ext.to_lowercase().as_str() {
                    "pdf" => "application/pdf",
                    "jpg" | "jpeg" => "image/jpeg",
                    "png" => "image/png",
                    "gif" => "image/gif",
                    "txt" => "text/plain",
                    "html" | "htm" => "text/html",
                    "epub" => "application/epub+zip",
                    _ => "application/octet-stream",
                })
                .unwrap_or("application/octet-stream")
                .to_string(),
        };

        ctx.mime = Some(mime_type.clone());
        upsert_field(&mut ctx.extracted_metadata, "mime_type", &mime_type);

        Ok(())
    }
}

pub(super) fn upsert_field(fields: &mut Vec<ExtractedField>, key: &str, value: &str) {
    if let Some(existing) = fields.iter_mut().find(|f| f.key == key) {
        existing.value = value.to_string();
    } else {
        fields.push(ExtractedField {
            key: key.to_string(),
            value: value.to_string(),
            data_type: "text".to_string(),
        });
    }
}

/// Does `mime` match any entry in `patterns`? Used by later Phase-1 nodes
/// to gate themselves on the MIME detected upstream. Supports the wildcards
/// `*` (match anything) and `image/*`.
pub(super) fn mime_matches(patterns: &[&str], known_mime: Option<&str>) -> bool {
    if patterns.contains(&"*") {
        return true;
    }
    match known_mime {
        Some(mime) => patterns.iter().any(|t| {
            if let Some(prefix) = t.strip_suffix("/*") {
                mime.starts_with(prefix)
            } else {
                *t == mime
            }
        }),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_matches_anything() {
        assert!(mime_matches(&["*"], None));
        assert!(mime_matches(&["*"], Some("application/pdf")));
    }

    #[test]
    fn specific_mime_matches_exact_only() {
        assert!(mime_matches(&["application/pdf"], Some("application/pdf")));
        assert!(!mime_matches(&["application/pdf"], Some("image/jpeg")));
        assert!(!mime_matches(&["application/pdf"], None));
    }

    #[test]
    fn prefix_wildcard_matches_family() {
        assert!(mime_matches(&["image/*"], Some("image/jpeg")));
        assert!(mime_matches(&["image/*"], Some("image/png")));
        assert!(!mime_matches(&["image/*"], Some("application/pdf")));
    }
}
