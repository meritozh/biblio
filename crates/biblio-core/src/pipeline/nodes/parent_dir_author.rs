use crate::pipeline::{FileContext, NodeError, Phase1Node, PipelineEnv};

/// Push every entry in `ctx.parent_author_candidates` onto
/// `suggested_author_names` so AuthorResolveNode gets a chance at them.
/// The list is filled once per batch in `file_prepare_import` from the
/// user-picked folder name, after LLM cleanup strips brackets / braces
/// (`[作者] 系列` → `["作者"]`). When the LLM is disabled or returns
/// nothing, the raw folder name is the sole candidate, matching the old
/// behavior. Empty for single-file picks, drag-drop, and the trivial
/// folder-to-zip case where the picked folder IS the comic.
pub struct ParentDirAuthorHintNode;

impl Phase1Node for ParentDirAuthorHintNode {
    fn name(&self) -> &'static str {
        "ParentDirAuthorHint"
    }

    fn applies(&self, ctx: &FileContext, _env: &PipelineEnv) -> bool {
        !ctx.parent_author_candidates.is_empty()
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        // Clone first to avoid double-borrow of ctx (mut for push, ref for
        // contains check).
        let candidates = ctx.parent_author_candidates.clone();
        for candidate in candidates {
            let trimmed = candidate.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            if !ctx.suggested_author_names.contains(&trimmed) {
                ctx.suggested_author_names.push(trimmed);
            }
        }
        Ok(())
    }
}
