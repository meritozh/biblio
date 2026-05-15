use async_trait::async_trait;

use super::content_sample::is_novel_file;
use crate::pipeline::runner::emit_progress;
use crate::pipeline::{FileContext, NodeError, Phase2Node, PipelineEnv};
use crate::schema::SchemaSlug;

/// Which input shape this node is responsible for. Drives the
/// `applies()` gate so a single comic pipeline can carry both an
/// archive- and a folder-flavored filename node, with exactly one
/// firing per file. The other is recorded as `Skipped` by the runner.
#[derive(Debug, Clone, Copy)]
enum FilenameSource {
    /// Plain text files (.txt). `applies` falls back to `is_novel_file`
    /// as a final sanity check on top of the dispatcher's extension filter.
    Text,
    /// Comic archives (.zip / .cbz / .rar / .cbr).
    Archive,
    /// Image-folder imports — directories of loose images that the
    /// commit step zips into a `.zip`. Uses a dedicated prompt step
    /// (`filename_folder` under the comic schema) that returns no
    /// `authors` (folder name is the work title; author candidates
    /// come from the picked-root LLM cleanup).
    Folder,
}

/// LLM Call 1: extract display_name / authors / progress from the filename.
/// The (schema_slug, step) pair is fixed at construction time so the
/// dispatcher decides which prompt to use, not the runtime MIME check.
///
/// The actual network call is gated by a 60 s timeout inside
/// `extract_filename_metadata`.
pub struct FilenameLlmNode {
    schema_slug: SchemaSlug,
    step: &'static str,
    source: FilenameSource,
}

impl FilenameLlmNode {
    pub fn text() -> Self {
        Self {
            schema_slug: SchemaSlug::Novel,
            step: "filename",
            source: FilenameSource::Text,
        }
    }
    pub fn archive() -> Self {
        Self {
            schema_slug: SchemaSlug::Comic,
            step: "filename",
            source: FilenameSource::Archive,
        }
    }
    pub fn folder() -> Self {
        Self {
            schema_slug: SchemaSlug::Comic,
            step: "filename_folder",
            source: FilenameSource::Folder,
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
        let is_dir = ctx.file_path.is_dir();
        match self.source {
            FilenameSource::Text => {
                if is_dir {
                    return false;
                }
                is_novel_file(&ctx.file_path.to_string_lossy())
            }
            // Archive variant only fires for real archive files; the
            // comic pipeline also carries the Folder variant, which
            // catches directory inputs.
            FilenameSource::Archive => !is_dir,
            // Folder variant only fires for directory inputs.
            FilenameSource::Folder => is_dir,
        }
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
            self.schema_slug,
            self.step,
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
