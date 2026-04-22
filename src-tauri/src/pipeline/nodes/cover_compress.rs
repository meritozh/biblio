use async_trait::async_trait;
use image::ImageReader;
use std::io::Cursor;

use crate::pipeline::{Cover, FileContext, NodeError, Phase2Node, PipelineEnv};

/// Max cover size allowed to bypass compression. Larger covers get
/// re-encoded as JPEG q=75 with width clamped to 1600 px, which typically
/// brings multi-MB comic pages down to < 500 KB without a visible drop
/// in thumbnail quality.
const PASSTHROUGH_THRESHOLD_BYTES: usize = 800 * 1024;
const MAX_WIDTH: u32 = 1600;
const JPEG_QUALITY: u8 = 75;

/// Re-encode oversized covers as JPEG to keep the `covers` BLOB table from
/// ballooning. Only runs when a cover is set and noticeably large; leaves
/// small, already-compressed covers alone.
pub struct CoverCompressNode;

#[async_trait]
impl Phase2Node for CoverCompressNode {
    fn name(&self) -> &'static str {
        "CoverCompress"
    }

    fn applies(&self, ctx: &FileContext, _env: &PipelineEnv) -> bool {
        ctx.cover
            .as_ref()
            .map(|c| c.data.len() > PASSTHROUGH_THRESHOLD_BYTES)
            .unwrap_or(false)
    }

    async fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        let Some(original) = ctx.cover.as_ref() else {
            return Ok(());
        };

        // Run the decode/encode on a blocking thread — image decoding is
        // CPU-heavy and can take 50-200 ms on a 5 MB jpeg, which would
        // stall the async Phase-2 worker while other LLM calls wait.
        let original_data = original.data.clone();
        let compressed = tokio::task::spawn_blocking(move || compress_to_jpeg(&original_data))
            .await
            .map_err(|e| NodeError(format!("Cover compress join error: {e}")))?;

        match compressed {
            Ok(bytes) if bytes.len() < original.data.len() => {
                ctx.cover = Some(Cover {
                    data: bytes,
                    mime_type: "image/jpeg".to_string(),
                });
                Ok(())
            }
            // Re-encode made it bigger — keep the original.
            Ok(_) => Ok(()),
            Err(e) => {
                // Decode failure on an exotic format: log and leave the
                // original bytes in place. Not a hard failure.
                eprintln!("Cover compress failed for {}: {}", ctx.file_name, e);
                Ok(())
            }
        }
    }
}

fn compress_to_jpeg(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("Cover format detect failed: {e}"))?;
    let img = reader
        .decode()
        .map_err(|e| format!("Cover decode failed: {e}"))?;

    let resized = if img.width() > MAX_WIDTH {
        let ratio = MAX_WIDTH as f32 / img.width() as f32;
        let new_h = (img.height() as f32 * ratio).round() as u32;
        img.resize(MAX_WIDTH, new_h, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    // Convert to RGB8 (JPEG doesn't support alpha), then encode.
    let rgb = resized.to_rgb8();
    let mut out = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    rgb.write_with_encoder(encoder)
        .map_err(|e| format!("JPEG encode failed: {e}"))?;
    Ok(out.into_inner())
}
