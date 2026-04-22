use super::mime::{mime_matches, upsert_field};
use crate::pipeline::{FileContext, NodeError, Phase1Node, PipelineEnv};

/// Extract ImageDescription, Model, DateTimeOriginal, and pixel dimensions
/// from EXIF tags on jpeg/png/tiff/webp images. Writes ImageDescription
/// into `ctx.display_name` if it's still empty.
pub struct ExifNode;

const SUPPORTED: &[&str] = &["image/jpeg", "image/png", "image/tiff", "image/webp"];

impl Phase1Node for ExifNode {
    fn name(&self) -> &'static str {
        "Exif"
    }

    fn applies(&self, ctx: &FileContext, _env: &PipelineEnv) -> bool {
        mime_matches(SUPPORTED, ctx.mime.as_deref())
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        let file = std::fs::File::open(&ctx.file_path)
            .map_err(|e| NodeError(format!("Failed to open file: {e}")))?;
        let mut buf_reader = std::io::BufReader::new(file);

        let exif_reader = exif::Reader::new();
        let exif_data = match exif_reader.read_from_container(&mut buf_reader) {
            Ok(data) => data,
            // Missing or unparseable EXIF is common; not a pipeline error.
            Err(_) => return Ok(()),
        };

        if ctx.display_name.is_none() {
            if let Some(field) = exif_data.get_field(exif::Tag::ImageDescription, exif::In::PRIMARY)
            {
                let val = field.display_value().to_string();
                if !val.is_empty() {
                    ctx.display_name = Some(val);
                }
            }
        }

        if let Some(field) = exif_data.get_field(exif::Tag::Model, exif::In::PRIMARY) {
            upsert_field(&mut ctx.extracted_metadata, "camera", &field.display_value().to_string());
        }

        if let Some(field) = exif_data.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
            let val = field.display_value().to_string();
            // Keep data_type "date" to match old ExifProcessor output.
            if let Some(existing) = ctx
                .extracted_metadata
                .iter_mut()
                .find(|f| f.key == "date_taken")
            {
                existing.value = val;
                existing.data_type = "date".to_string();
            } else {
                ctx.extracted_metadata.push(crate::pipeline::ExtractedField {
                    key: "date_taken".to_string(),
                    value: val,
                    data_type: "date".to_string(),
                });
            }
        }

        let w = exif_data.get_field(exif::Tag::PixelXDimension, exif::In::PRIMARY);
        let h = exif_data.get_field(exif::Tag::PixelYDimension, exif::In::PRIMARY);
        if let (Some(wf), Some(hf)) = (w, h) {
            upsert_field(
                &mut ctx.extracted_metadata,
                "dimensions",
                &format!("{}x{}", wf.display_value(), hf.display_value()),
            );
        }

        Ok(())
    }
}
