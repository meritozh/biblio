use async_trait::async_trait;

use super::content_sample::is_novel_file;
use super::mime::upsert_field;
use crate::pipeline::{FileContext, NodeError, Phase2Node, PipelineEnv};
use crate::pipeline::runner::emit_progress;

/// LLM Call 2: classify the novel via content samples (category / tags /
/// description). Runs only when the filename LLM applies *and* the user
/// has opted in via `settings.analyze_content` *and* a content sample is
/// available. The 180 s timeout lives inside `extract_content_metadata`.
pub struct ContentLlmNode;

#[async_trait]
impl Phase2Node for ContentLlmNode {
    fn name(&self) -> &'static str {
        "ContentLlm"
    }

    fn applies(&self, ctx: &FileContext, env: &PipelineEnv) -> bool {
        if !env.llm_config.enabled {
            return false;
        }
        if !env.llm_config.analyze_content {
            return false;
        }
        if ctx.content_sample.is_none() {
            return false;
        }
        is_novel_file(
            &ctx.file_path.to_string_lossy(),
            env.settings.process_novel_epub,
            env.settings.process_novel_pdf,
        )
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        let Some(sample) = ctx.content_sample.clone() else {
            return Ok(());
        };

        emit_progress(
            &env.app,
            ctx.processed_ordinal,
            ctx.total,
            &ctx.file_name,
            "analyzing_content",
        );

        let meta = crate::commands::llm::extract_content_metadata(
            &env.llm_config,
            &env.pool,
            &sample,
            ctx.display_name.as_deref(),
            &env.category_names,
            &env.tag_names,
        )
        .await
        .map_err(|e| {
            eprintln!("LLM content analysis failed for {}: {}", ctx.file_name, e);
            NodeError(e)
        })?;

        if let Some(cat) = meta.category {
            // LLM output sometimes includes the parenthesized hint we passed
            // in (e.g. "h-novel (novel with sexual content)"); strip it.
            let cat_clean = cat.split('(').next().unwrap_or(&cat).trim().to_lowercase();
            ctx.category_id = env.category_map.get(&cat_clean).copied();
        }
        for tag in meta.tags {
            if let Some(&id) = env.tag_map.get(&tag.to_lowercase()) {
                if !ctx.tag_ids.contains(&id) {
                    ctx.tag_ids.push(id);
                }
            } else if !ctx.suggested_tags.contains(&tag) {
                ctx.suggested_tags.push(tag);
            }
        }
        if let Some(desc) = meta.description {
            if !desc.is_empty() {
                upsert_field(&mut ctx.extracted_metadata, "description", &desc);
            }
        }

        Ok(())
    }
}
