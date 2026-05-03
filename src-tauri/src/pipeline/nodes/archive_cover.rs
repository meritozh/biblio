use crate::pipeline::archive::pick_first_cover;
use crate::pipeline::{FileContext, NodeError, Phase1Node, PipelineEnv};

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
