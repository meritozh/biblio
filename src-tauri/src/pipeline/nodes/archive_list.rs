use crate::pipeline::archive::{list_image_entries, ArchiveImageEntry};
use crate::pipeline::{ArchiveEntry, FileContext, NodeError, Phase1Node, PipelineEnv};

/// Enumerate image entries inside a comic archive (ZIP/CBZ or RAR/CBR)
/// without extracting them. Captures `(basename, archive_index)` so the
/// Phase-2 vision check can pull only the candidate images it actually
/// needs (typically ≤5 of N) by re-opening the archive. Applies only when
/// the LLM is enabled — otherwise the listing has no consumer and
/// `ArchiveFirstImageCoverNode` alone covers the archive-cover need.
pub struct ArchiveListImagesNode;

impl Phase1Node for ArchiveListImagesNode {
    fn name(&self) -> &'static str {
        "ArchiveListImages"
    }

    fn applies(&self, _ctx: &FileContext, env: &PipelineEnv) -> bool {
        env.llm_config.enabled
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        let listed = list_image_entries(&ctx.file_path).map_err(NodeError)?;
        ctx.archive_entries = listed
            .into_iter()
            .map(
                |ArchiveImageEntry {
                     basename,
                     archive_index,
                 }| ArchiveEntry {
                    basename,
                    archive_index,
                },
            )
            .collect();
        Ok(())
    }
}
