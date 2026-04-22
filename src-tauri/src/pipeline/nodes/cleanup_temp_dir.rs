use async_trait::async_trait;

use crate::pipeline::{FileContext, NodeError, Phase2Node, PipelineEnv};

/// Remove the temp dir that `ArchiveUnzipNode` populated with extracted
/// comic pages. Runs late in Phase 2 so every LLM node that needs on-disk
/// image bytes has already finished. Never surfaces a NodeError — a
/// failed cleanup is logged but shouldn't fail the overall import.
pub struct CleanupTempDirNode;

#[async_trait]
impl Phase2Node for CleanupTempDirNode {
    fn name(&self) -> &'static str {
        "CleanupTempDir"
    }

    fn applies(&self, ctx: &FileContext, _env: &PipelineEnv) -> bool {
        ctx.archive_temp_dir.is_some()
    }

    async fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        if let Some(dir) = ctx.archive_temp_dir.take() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                eprintln!("Failed to clean temp dir {}: {e}", dir.display());
            }
        }
        // Entries reference files that no longer exist — clear them so
        // later code doesn't accidentally try to read them.
        ctx.archive_entries.clear();
        Ok(())
    }
}
