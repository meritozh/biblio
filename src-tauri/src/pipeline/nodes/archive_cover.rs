use super::mime::mime_matches;
use crate::pipeline::archive::pick_first_cover;
use crate::pipeline::{Cover, FileContext, NodeError, Phase1Node, PipelineEnv};

/// Extract the first/alphabetically-first image out of a comic archive
/// (ZIP/CBZ or RAR/CBR) and use it as the file's cover. Acts as a baseline
/// fallback for comics; the Phase-2 LLM vision path may override it.
pub struct ArchiveFirstImageCoverNode;

impl Phase1Node for ArchiveFirstImageCoverNode {
    fn name(&self) -> &'static str {
        "ArchiveFirstImageCover"
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        match pick_first_cover(&ctx.file_path) {
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
