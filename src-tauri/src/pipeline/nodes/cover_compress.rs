use async_trait::async_trait;

use crate::commands::cover::compress_cover_bytes;
use crate::pipeline::{Cover, FileContext, NodeError, Phase2Node, PipelineEnv};

/// Re-encode every cover the pipeline produces through the shared helper
/// (`commands::cover::compress_cover_bytes`). Single enforcement point
/// keeps stored covers ≤ ~200 KB regardless of the source format —
/// pipeline-extracted covers, user-uploaded covers, and the one-shot
/// migration all go through the same path.
///
/// On decode failure we leave the original bytes in place: a tiny number
/// of exotic formats may not survive a round-trip through `image` (e.g.
/// progressive-only or unusual chroma subsampling). The original is
/// already in memory; better to keep it than to drop the cover entirely.
pub struct CoverCompressNode;

#[async_trait]
impl Phase2Node for CoverCompressNode {
    fn name(&self) -> &'static str {
        "CoverCompress"
    }

    fn applies(&self, ctx: &FileContext, _env: &PipelineEnv) -> bool {
        ctx.cover.is_some()
    }

    async fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        let Some(original) = ctx.cover.as_ref() else {
            return Ok(());
        };

        // Decode/encode is CPU-heavy (50–200 ms on a multi-MB JPEG) and
        // would stall the async Phase-2 worker; offload to the blocking
        // pool so other LLM calls keep progressing.
        let original_data = original.data.clone();
        let compressed = tokio::task::spawn_blocking(move || compress_cover_bytes(&original_data))
            .await
            .map_err(|e| NodeError(format!("Cover compress join error: {e}")))?;

        match compressed {
            Ok(bytes) => {
                ctx.cover = Some(Cover {
                    data: bytes,
                    mime_type: "image/jpeg".to_string(),
                });
                Ok(())
            }
            Err(e) => {
                eprintln!("Cover compress failed for {}: {}", ctx.file_name, e);
                Ok(())
            }
        }
    }
}
