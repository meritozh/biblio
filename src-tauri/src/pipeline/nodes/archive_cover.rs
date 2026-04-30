use std::path::Path;

use super::mime::mime_matches;
use crate::pipeline::{Cover, FileContext, NodeError, Phase1Node, PipelineEnv};

/// Extract the first/alphabetically-first image out of a ZIP/CBZ archive
/// and use it as the file's cover. Acts as a baseline fallback for
/// comics; the Phase-2 LLM vision path may override it.
pub struct ArchiveFirstImageCoverNode;

impl Phase1Node for ArchiveFirstImageCoverNode {
    fn name(&self) -> &'static str {
        "ArchiveFirstImageCover"
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        match extract_cover_from_archive(&ctx.file_path) {
            Ok(cover) => {
                ctx.cover = Some(cover);
                Ok(())
            }
            // Archives without images are common (source dumps etc.); treat
            // as a non-failure so the pipeline keeps going.
            Err(_) => Ok(()),
        }
    }
}

/// For a standalone image file, read the bytes and use the image itself as
/// its own cover. Applies only when no cover has been extracted yet.
pub struct SingleImageCoverNode;

impl Phase1Node for SingleImageCoverNode {
    fn name(&self) -> &'static str {
        "SingleImageCover"
    }

    fn applies(&self, ctx: &FileContext, _env: &PipelineEnv) -> bool {
        if ctx.cover.is_some() {
            return false;
        }
        mime_matches(&["image/*"], ctx.mime.as_deref())
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        let mime = ctx.mime.clone().unwrap_or_else(|| "image/jpeg".to_string());
        let data = std::fs::read(&ctx.file_path)
            .map_err(|e| NodeError(format!("Failed to read image: {e}")))?;
        ctx.cover = Some(Cover { data, mime_type: mime });
        Ok(())
    }
}

fn extract_cover_from_archive(file_path: &Path) -> Result<Cover, String> {
    use std::io::Read;
    use zip::ZipArchive;

    let file = std::fs::File::open(file_path)
        .map_err(|e| format!("Failed to open archive: {e}"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("Failed to read ZIP archive: {e}"))?;

    let mut image_entries: Vec<(String, usize)> = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| format!("ZIP entry error: {e}"))?;
        let name = file.name().to_string();
        let lower = name.to_lowercase();
        if lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".png")
            || lower.ends_with(".webp")
            || lower.ends_with(".gif")
        {
            image_entries.push((name, i));
        }
    }

    if image_entries.is_empty() {
        return Err("No images found in archive".to_string());
    }

    image_entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Prefer files named "cover" or starting with "000"/"001" so comics with
    // a dedicated cover page (common for scanlation releases) pick it up.
    let cover_idx = image_entries
        .iter()
        .find(|(name, _)| {
            let lower = name.to_lowercase();
            lower.contains("cover") || lower.starts_with("000") || lower.starts_with("001")
        })
        .map(|&(_, idx)| idx)
        .unwrap_or(image_entries[0].1);

    let mut file = archive
        .by_index(cover_idx)
        .map_err(|e| format!("Failed to read cover entry: {e}"))?;
    let name = file.name().to_string();

    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| format!("Failed to read cover data: {e}"))?;

    let mime_type = if name.to_lowercase().ends_with(".png") {
        "image/png".to_string()
    } else if name.to_lowercase().ends_with(".webp") {
        "image/webp".to_string()
    } else if name.to_lowercase().ends_with(".gif") {
        "image/gif".to_string()
    } else {
        "image/jpeg".to_string()
    };

    Ok(Cover { data, mime_type })
}
