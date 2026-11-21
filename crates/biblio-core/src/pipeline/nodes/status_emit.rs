use async_trait::async_trait;

use crate::pipeline::runner::emit_progress;
use crate::pipeline::{FileContext, NodeError, NodeStatus, Phase2Node, PipelineEnv};

/// Final Phase-2 node: synthesize the user-facing status from the outcomes
/// of `FilenameLlmNode` and `ContentLlmNode`, then emit one last
/// `processing-progress` event.
///
/// Matrix:
/// | filename | content          | result  |
/// |----------|------------------|---------|
/// | Ok       | Ok / Skipped     | ready   |
/// | Ok       | Err              | partial |
/// | Err      | Ok               | partial |
/// | Err      | Skipped / Err    | error   |
/// | Skipped  | (any)            | ready   | ← non-novel or LLM disabled
pub struct StatusEmitNode;

#[async_trait]
impl Phase2Node for StatusEmitNode {
    fn name(&self) -> &'static str {
        "StatusEmit"
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        let status = decide_status(
            ctx.outcome_of("FilenameLlm"),
            ctx.outcome_of("ContentLlm"),
        );

        emit_progress(
            &env.app,
            ctx.processed_ordinal,
            ctx.total,
            &ctx.event_key(),
            status,
        );
        Ok(())
    }
}

fn decide_status(filename: Option<&NodeStatus>, content: Option<&NodeStatus>) -> &'static str {
    use NodeStatus::*;
    match (filename, content) {
        // FilenameLlm didn't apply (non-novel or LLM off) → no error path.
        (None, _) | (Some(Skipped), _) => "ready",

        (Some(Ok), Some(Ok) | Some(Skipped) | None) => "ready",
        (Some(Ok), Some(Err(_))) => "partial",
        (Some(Err(_)), Some(Ok)) => "partial",
        (Some(Err(_)), _) => "error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use NodeStatus::*;

    #[test]
    fn non_novel_is_ready() {
        assert_eq!(decide_status(None, None), "ready");
        assert_eq!(decide_status(Some(&Skipped), None), "ready");
    }

    #[test]
    fn both_ok_is_ready() {
        assert_eq!(decide_status(Some(&Ok), Some(&Ok)), "ready");
    }

    #[test]
    fn name_ok_content_absent_is_ready() {
        assert_eq!(decide_status(Some(&Ok), None), "ready");
        assert_eq!(decide_status(Some(&Ok), Some(&Skipped)), "ready");
    }

    #[test]
    fn name_ok_content_err_is_partial() {
        assert_eq!(
            decide_status(Some(&Ok), Some(&Err("boom".into()))),
            "partial"
        );
    }

    #[test]
    fn name_err_content_ok_is_partial() {
        assert_eq!(
            decide_status(Some(&Err("boom".into())), Some(&Ok)),
            "partial"
        );
    }

    #[test]
    fn name_err_content_missing_or_err_is_error() {
        assert_eq!(decide_status(Some(&Err("boom".into())), None), "error");
        assert_eq!(
            decide_status(Some(&Err("a".into())), Some(&Skipped)),
            "error"
        );
        assert_eq!(
            decide_status(Some(&Err("a".into())), Some(&Err("b".into()))),
            "error"
        );
    }
}
