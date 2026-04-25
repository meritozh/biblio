use async_trait::async_trait;

use crate::pipeline::runner::emit_progress;
use crate::pipeline::{FileContext, NodeError, Phase2Node, PipelineEnv};

/// LLM-based top-5 cover filename picker. Feeds the list of extracted
/// image basenames (archive order) to the text LLM and stores the ranking
/// in `ctx.cover_candidates`. The vision node downstream picks the actual
/// cover; this step is just to avoid sending N images to the vision model.
pub struct LlmCoverCandidatesNode;

#[async_trait]
impl Phase2Node for LlmCoverCandidatesNode {
    fn name(&self) -> &'static str {
        "LlmCoverCandidates"
    }

    fn applies(&self, ctx: &FileContext, env: &PipelineEnv) -> bool {
        env.llm_config.enabled && !ctx.archive_entries.is_empty()
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        emit_progress(
            &env.app,
            ctx.processed_ordinal,
            ctx.total,
            &ctx.file_name,
            "picking_cover_candidates",
        );

        let names: Vec<&str> = ctx
            .archive_entries
            .iter()
            .map(|e| e.basename.as_str())
            .collect();

        match crate::commands::llm::extract_cover_candidates(&env.llm_config, &env.pool, &names).await {
            Ok(mut candidates) => {
                // Defensive: the LLM may hallucinate filenames. Keep only
                // names that actually appear in archive_entries.
                let valid: std::collections::HashSet<&str> =
                    ctx.archive_entries.iter().map(|e| e.basename.as_str()).collect();
                candidates.retain(|c| valid.contains(c.as_str()));
                ctx.cover_candidates = candidates;
                Ok(())
            }
            Err(e) => {
                eprintln!(
                    "LLM cover-candidate pick failed for {}: {}",
                    ctx.file_name, e
                );
                // Fall back to archive order so the vision node still has
                // something to try; take up to 5 entries.
                ctx.cover_candidates = ctx
                    .archive_entries
                    .iter()
                    .take(5)
                    .map(|e| e.basename.clone())
                    .collect();
                Err(NodeError(e))
            }
        }
    }
}
