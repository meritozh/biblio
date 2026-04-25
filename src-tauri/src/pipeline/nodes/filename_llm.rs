use async_trait::async_trait;

use super::content_sample::is_novel_file;
use crate::pipeline::{FileContext, NodeError, Phase2Node, PipelineEnv};
use crate::pipeline::runner::emit_progress;

/// Resolve which prompt group applies to this file: `text` for novels,
/// `archive` for comic-style zip/cbz, or `None` if FilenameLlmNode
/// shouldn't run at all (e.g. a bare image).
fn mime_group_for(ctx: &FileContext, env: &PipelineEnv) -> Option<&'static str> {
    let path_str = ctx.file_path.to_string_lossy();
    if is_novel_file(
        &path_str,
        env.settings.process_novel_epub,
        env.settings.process_novel_pdf,
    ) {
        return Some("text");
    }
    let mime = ctx.mime.as_deref().unwrap_or("");
    if mime.contains("zip") || mime.contains("cbz") {
        return Some("archive");
    }
    None
}

/// LLM Call 1: extract display_name / authors / progress from the filename.
/// Runs only for novel-like files when the LLM is enabled. The actual
/// network call is gated by a 60 s timeout inside `extract_filename_metadata`.
pub struct FilenameLlmNode;

#[async_trait]
impl Phase2Node for FilenameLlmNode {
    fn name(&self) -> &'static str {
        "FilenameLlm"
    }

    fn applies(&self, ctx: &FileContext, env: &PipelineEnv) -> bool {
        if !env.llm_config.enabled {
            return false;
        }
        mime_group_for(ctx, env).is_some()
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        emit_progress(
            &env.app,
            ctx.processed_ordinal,
            ctx.total,
            &ctx.file_name,
            "extracting_name",
        );

        // Pick the matching prompt group: text for novels, archive for
        // comics. `applies()` already gated this so unwrap is safe — but
        // fall back to `text` if a future caller adds a new path without
        // updating the dispatcher, so we don't hard-fail on an unknown
        // mime.
        let mime_group = mime_group_for(ctx, env).unwrap_or("text");

        let meta = crate::commands::llm::extract_filename_metadata(
            &env.llm_config,
            &env.pool,
            &ctx.file_name,
            mime_group,
        )
        .await
        .map_err(|e| {
            // Keep the log path identical to the old monolith so ops-style
            // grepping for "LLM filename extraction failed" still works.
            eprintln!("LLM filename extraction failed for {}: {}", ctx.file_name, e);
            NodeError(e)
        })?;

        if let Some(name) = meta.display_name {
            if !name.is_empty() {
                ctx.display_name = Some(name);
            }
        }
        if let Some(p) = meta.progress {
            if !p.is_empty() {
                ctx.progress = Some(p);
            }
        }
        for author in meta.authors {
            if !ctx.suggested_author_names.contains(&author) {
                ctx.suggested_author_names.push(author);
            }
        }

        Ok(())
    }
}
