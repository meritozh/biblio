use crate::pipeline::{ArchiveEntry, FileContext, NodeError, Phase1Node, PipelineEnv};

/// Enumerate image entries inside a ZIP archive without extracting them.
/// Captures `(basename, archive_index)` so the Phase-2 vision check can
/// re-open the archive and read only the candidate images it actually
/// needs (typically ≤5 of N). Applies only when the LLM is enabled —
/// otherwise the listing has no consumer and `ArchiveFirstImageCoverNode`
/// alone covers the archive-cover need.
pub struct ArchiveListImagesNode;

impl Phase1Node for ArchiveListImagesNode {
    fn name(&self) -> &'static str {
        "ArchiveListImages"
    }

    fn applies(&self, _ctx: &FileContext, env: &PipelineEnv) -> bool {
        env.llm_config.enabled
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        let file = std::fs::File::open(&ctx.file_path)
            .map_err(|e| NodeError(format!("Failed to open archive: {e}")))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| NodeError(format!("Failed to read ZIP archive: {e}")))?;

        let mut entries: Vec<ArchiveEntry> = Vec::new();
        for i in 0..archive.len() {
            let entry = archive
                .by_index_raw(i)
                .map_err(|e| NodeError(format!("ZIP entry error: {e}")))?;
            let raw_name = entry.name().to_string();
            let basename = std::path::Path::new(&raw_name)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| raw_name.clone());

            if !is_image_entry(&basename) {
                continue;
            }

            entries.push(ArchiveEntry {
                basename,
                archive_index: i,
            });
        }

        ctx.archive_entries = entries;
        Ok(())
    }
}

fn is_image_entry(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".webp")
        || lower.ends_with(".gif")
}

/// MIME type to attach when uploading one of the listed image entries to
/// a vision LLM. Defaults to JPEG when the extension is unknown because
/// most comic dumps use JPEG.
pub(super) fn guess_image_mime(basename: &str) -> String {
    let lower = basename.to_lowercase();
    if lower.ends_with(".png") {
        "image/png".into()
    } else if lower.ends_with(".webp") {
        "image/webp".into()
    } else if lower.ends_with(".gif") {
        "image/gif".into()
    } else {
        "image/jpeg".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_image_entry_accepts_common_formats() {
        for ext in ["jpg", "JPG", "jpeg", "png", "PNG", "webp", "gif"] {
            assert!(is_image_entry(&format!("001.{ext}")));
        }
    }

    #[test]
    fn is_image_entry_rejects_non_images() {
        for name in ["README.txt", "meta.json", "001", "folder/", "001.bmp"] {
            assert!(!is_image_entry(name), "{name} should not be image");
        }
    }
}
