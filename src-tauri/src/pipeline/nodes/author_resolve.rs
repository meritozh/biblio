use async_trait::async_trait;

use crate::pipeline::{FileContext, NodeError, Phase2Node, PipelineEnv};

/// Resolve every name collected in `ctx.suggested_author_names` — whether
/// surfaced by a Phase-1 processor (PDF Info.Author, EXIF Artist) or by the
/// filename LLM — against `env.author_map`. Hits become `author_ids`,
/// misses become `unresolved_authors`. Runs once, near the end of Phase 2.
pub struct AuthorResolveNode;

#[async_trait]
impl Phase2Node for AuthorResolveNode {
    fn name(&self) -> &'static str {
        "AuthorResolve"
    }

    fn applies(&self, ctx: &FileContext, _env: &PipelineEnv) -> bool {
        !ctx.suggested_author_names.is_empty()
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        for name in &ctx.suggested_author_names {
            if let Some(&id) = env.author_map.get(&name.to_lowercase()) {
                if !ctx.author_ids.contains(&id) {
                    ctx.author_ids.push(id);
                }
            } else if !ctx.unresolved_authors.contains(name) {
                ctx.unresolved_authors.push(name.clone());
            }
        }
        Ok(())
    }
}
