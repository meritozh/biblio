use async_trait::async_trait;

use super::content_sample::is_novel_file;
use crate::pipeline::runner::emit_progress;
use crate::pipeline::{FileContext, NodeError, Phase2Node, PipelineEnv};

/// LLM Call 1: extract display_name / authors / progress from the filename.
/// The prompt group is fixed at construction time — `"text"` for the novel
/// pipeline, `"archive"` for the comic pipeline — so the dispatcher decides
/// which prompt to use, not the runtime MIME check.
///
/// For the `"text"` variant we still gate on `is_novel_file` so the per-
/// format toggles (`process_novel_epub` / `process_novel_pdf`) take effect
/// even when the file made it past the frontend filter.
///
/// The actual network call is gated by a 60 s timeout inside
/// `extract_filename_metadata`.
pub struct FilenameLlmNode {
    mime_group: &'static str,
}

impl FilenameLlmNode {
    pub fn text() -> Self {
        Self { mime_group: "text" }
    }
    pub fn archive() -> Self {
        Self {
            mime_group: "archive",
        }
    }
}

#[async_trait]
impl Phase2Node for FilenameLlmNode {
    fn name(&self) -> &'static str {
        "FilenameLlm"
    }

    fn applies(&self, ctx: &FileContext, env: &PipelineEnv) -> bool {
        if !env.llm_config.enabled {
            return false;
        }
        if self.mime_group == "text" {
            return is_novel_file(
                &ctx.file_path.to_string_lossy(),
                env.settings.process_novel_epub,
                env.settings.process_novel_pdf,
            );
        }
        // archive: dispatcher already routed only zip/cbz/rar/cbr here.
        true
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        emit_progress(
            &env.app,
            ctx.processed_ordinal,
            ctx.total,
            &ctx.file_name,
            "extracting_name",
        );

        let meta = crate::commands::llm::extract_filename_metadata(
            &env.llm_config,
            &env.pool,
            &ctx.file_name,
            self.mime_group,
        )
        .await
        .map_err(|e| {
            // Keep the log path identical to the old monolith so ops-style
            // grepping for "LLM filename extraction failed" still works.
            eprintln!(
                "LLM filename extraction failed for {}: {}",
                ctx.file_name, e
            );
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
