use async_trait::async_trait;

use super::archive_unzip::guess_image_mime;
use crate::pipeline::runner::emit_progress;
use crate::pipeline::{Cover, FileContext, NodeError, Phase2Node, PipelineEnv};

/// Walk the LLM-ranked candidates in order, fire a multimodal "is this a
/// cover?" call for each, and set `ctx.cover` on the first yes. If no
/// candidate answers yes, the first candidate is used unconditionally so
/// we never end up without a cover on a comic that had extractable images.
///
/// On vision endpoint failure (e.g. text-only model, network error), we
/// abandon the vision check entirely and use the first candidate directly
/// — a degraded but still-useful result.
pub struct LlmVisionCoverCheckNode;

#[async_trait]
impl Phase2Node for LlmVisionCoverCheckNode {
    fn name(&self) -> &'static str {
        "LlmVisionCoverCheck"
    }

    fn applies(&self, ctx: &FileContext, env: &PipelineEnv) -> bool {
        env.llm_config.enabled && !ctx.cover_candidates.is_empty()
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        emit_progress(
            &env.app,
            ctx.processed_ordinal,
            ctx.total,
            &ctx.file_name,
            "checking_cover_vision",
        );

        // Snapshot the candidate list so we can mutate ctx.cover inside
        // the loop without borrowing conflicts.
        let candidates = ctx.cover_candidates.clone();

        for candidate_name in &candidates {
            let Some(entry) = ctx
                .archive_entries
                .iter()
                .find(|e| &e.basename == candidate_name)
            else {
                continue;
            };

            let Ok(bytes) = std::fs::read(&entry.extracted_path) else {
                continue;
            };
            let mime = guess_image_mime(&entry.extracted_path);

            match crate::commands::llm::check_is_cover(&env.llm_config, &bytes, &mime).await {
                Ok(true) => {
                    ctx.cover = Some(Cover {
                        data: bytes,
                        mime_type: mime,
                    });
                    return Ok(());
                }
                Ok(false) => {
                    // LLM confidently says "not a cover" — keep scanning.
                    continue;
                }
                Err(e) => {
                    // First vision failure aborts the check entirely —
                    // text-only models and unreachable endpoints would
                    // fail the same way on every candidate. Use whichever
                    // candidate we're currently holding as the cover and
                    // move on.
                    eprintln!(
                        "Vision cover check failed on {} ({}); falling back to first candidate: {}",
                        candidate_name, ctx.file_name, e
                    );
                    ctx.cover = Some(Cover {
                        data: bytes,
                        mime_type: mime,
                    });
                    return Ok(());
                }
            }
        }

        // No candidate confirmed as a cover and no vision error either —
        // use the first candidate as a last-resort pick so comic rows
        // never end up without thumbnails.
        if ctx.cover.is_none() {
            if let Some(first) = ctx.cover_candidates.first() {
                if let Some(entry) = ctx
                    .archive_entries
                    .iter()
                    .find(|e| &e.basename == first)
                {
                    if let Ok(bytes) = std::fs::read(&entry.extracted_path) {
                        let mime = guess_image_mime(&entry.extracted_path);
                        ctx.cover = Some(Cover {
                            data: bytes,
                            mime_type: mime,
                        });
                    }
                }
            }
        }

        Ok(())
    }
}
