use std::path::PathBuf;

use crate::pipeline::{ArchiveEntry, FileContext, NodeError, Phase1Node, PipelineEnv};

/// Unzip image entries of an archive to a per-file temp dir so downstream
/// Phase-2 LLM vision nodes can load them by path. Applies only when the
/// LLM is enabled — otherwise `ArchiveFirstImageCoverNode` alone covers
/// the archive-cover need and there's no point paying the extraction cost.
///
/// Non-image entries are skipped; we only need images for cover picking.
/// Paired with `CleanupTempDirNode` which removes the directory after
/// Phase 2.
pub struct ArchiveUnzipNode;

impl Phase1Node for ArchiveUnzipNode {
    fn name(&self) -> &'static str {
        "ArchiveUnzip"
    }

    fn applies(&self, _ctx: &FileContext, env: &PipelineEnv) -> bool {
        env.llm_config.enabled
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        let file = std::fs::File::open(&ctx.file_path)
            .map_err(|e| NodeError(format!("Failed to open archive: {e}")))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| NodeError(format!("Failed to read ZIP archive: {e}")))?;

        // Unique per-file temp dir under the OS temp root so parallel
        // imports don't collide. We include the PID and a sanitized file
        // name so debugging `ls /tmp/biblio-*` is readable.
        let sanitized: String = ctx
            .file_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let temp_dir = std::env::temp_dir().join(format!(
            "biblio-archive-{}-{}-{}",
            std::process::id(),
            ctx.input_index,
            sanitized,
        ));
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| NodeError(format!("Failed to create temp dir: {e}")))?;

        let mut entries: Vec<ArchiveEntry> = Vec::new();
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| NodeError(format!("ZIP entry error: {e}")))?;
            let raw_name = entry.name().to_string();
            let basename = std::path::Path::new(&raw_name)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| raw_name.clone());

            if !is_image_entry(&basename) {
                continue;
            }

            let out_path = temp_dir.join(&basename);
            let mut out_file = std::fs::File::create(&out_path)
                .map_err(|e| NodeError(format!("Failed to create {}: {e}", out_path.display())))?;
            std::io::copy(&mut entry, &mut out_file)
                .map_err(|e| NodeError(format!("Failed to extract {basename}: {e}")))?;

            entries.push(ArchiveEntry {
                basename,
                extracted_path: out_path,
            });
        }

        if entries.is_empty() {
            // Archive has no images — clean up the empty temp dir now so
            // CleanupTempDirNode doesn't have to find it.
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Ok(());
        }

        ctx.archive_temp_dir = Some(temp_dir);
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

/// Return the MIME type to attach when uploading one of the extracted
/// image entries to a vision LLM. Defaults to JPEG when the extension is
/// unknown because most comic dumps use JPEG.
pub(super) fn guess_image_mime(path: &PathBuf) -> String {
    let lower = path.to_string_lossy().to_lowercase();
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
