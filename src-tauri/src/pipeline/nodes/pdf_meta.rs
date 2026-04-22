use super::mime::{mime_matches, upsert_field};
use crate::pipeline::{ExtractedField, FileContext, NodeError, Phase1Node, PipelineEnv};

/// Pull Title/Author/Subject/Keywords/Creator/Producer and page count from
/// a PDF's `Info` dictionary. Runs only when MimeDetectNode saw a PDF.
pub struct PdfMetaNode;

impl Phase1Node for PdfMetaNode {
    fn name(&self) -> &'static str {
        "PdfMeta"
    }

    fn applies(&self, ctx: &FileContext, _env: &PipelineEnv) -> bool {
        mime_matches(&["application/pdf"], ctx.mime.as_deref())
    }

    fn run(&self, ctx: &mut FileContext, _env: &PipelineEnv) -> Result<(), NodeError> {
        let doc = lopdf::Document::load(&ctx.file_path)
            .map_err(|e| NodeError(format!("Failed to load PDF: {e}")))?;

        let page_count = doc.get_pages().len();
        upsert_field(&mut ctx.extracted_metadata, "page_count", &page_count.to_string());

        let info_ref = match doc.trailer.get(b"Info") {
            Ok(obj) => obj,
            Err(_) => return Ok(()),
        };

        match info_ref {
            lopdf::Object::Reference(id) => {
                if let Some(info_obj) = doc.objects.get(id) {
                    if let Ok(info_dict) = info_obj.as_dict() {
                        extract_info(
                            info_dict,
                            &mut ctx.extracted_metadata,
                            &mut ctx.display_name,
                            &mut ctx.suggested_author_names,
                        );
                    }
                }
            }
            lopdf::Object::Dictionary(dict) => {
                extract_info(
                    dict,
                    &mut ctx.extracted_metadata,
                    &mut ctx.display_name,
                    &mut ctx.suggested_author_names,
                );
            }
            _ => {}
        }

        Ok(())
    }
}

fn extract_info(
    dict: &lopdf::Dictionary,
    metadata: &mut Vec<ExtractedField>,
    display_name: &mut Option<String>,
    suggested_authors: &mut Vec<String>,
) {
    let get_str = |key: &[u8]| -> Option<String> {
        dict.get(key)
            .ok()
            .and_then(|v| v.as_str().ok())
            .map(|s| String::from_utf8_lossy(s).to_string())
    };

    if display_name.is_none() {
        if let Some(title) = get_str(b"Title") {
            if !title.is_empty() {
                *display_name = Some(title);
            }
        }
    }
    if let Some(author) = get_str(b"Author") {
        if !author.is_empty() && !suggested_authors.contains(&author) {
            suggested_authors.push(author);
        }
    }

    let fields: &[(&[u8], &str)] = &[
        (b"Subject", "subject"),
        (b"Keywords", "keywords"),
        (b"Creator", "creator"),
        (b"Producer", "producer"),
    ];
    for &(key, label) in fields {
        if let Some(val) = get_str(key) {
            if !val.is_empty() {
                upsert_field(metadata, label, &val);
            }
        }
    }
}
