//! Schema export/import: share a schema definition as a JSON file.
//!
//! The export envelope carries basic info, fields, and pipeline steps.
//! Prompts are deliberately NOT included — a schema without custom
//! prompts falls back to its template's prompts by design (see
//! `prompt_with_fallback`), which is exactly what a recipient wants.
//!
//! Import (`schema_import_read`) reads and validates the file, then
//! returns the definition *unpersisted*; the frontend prefills the
//! schema editor with it, so actual creation rides the existing
//! `schema_create` + `schema_steps_update` commands and every
//! create-path validation applies unchanged. Nothing here writes rows.
//!
//! Split into its own module (rather than `schema_admin`) because that
//! file sits close to the repo's 800-line per-file guard.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

use super::schema_admin::{
    validate_payload, validate_schema_id, SchemaFieldInput, SchemaStepInput, SchemaUpsertPayload,
    PIPELINE_STEP_VOCAB,
};
use super::schemas::{SchemaDefinition, SchemaFieldRow, SchemaPipelineStepRow, SchemaRow};

/// Envelope tag; files without it fail import with
/// `IMPORT_NOT_A_SCHEMA_FILE`.
pub const EXPORT_FORMAT: &str = "biblio-schema";
/// Portable-shape version; import rejects any other value with
/// `IMPORT_UNSUPPORTED_VERSION:<n>`. Bump on breaking changes.
pub const EXPORT_VERSION: u32 = 1;

/// The shareable document. Field/step entries reuse the editor's wire
/// types (`SchemaFieldInput` / `SchemaStepInput`) so export and manual
/// editing produce the same shapes import validates.
#[derive(Debug, Serialize, Deserialize)]
struct SchemaExportFile {
    format: String,
    version: u32,
    schema: PortableSchema,
}

#[derive(Debug, Serialize, Deserialize)]
struct PortableSchema {
    id: String,
    name: String,
    icon: Option<String>,
    description: Option<String>,
    accepted_extensions: Vec<String>,
    pipeline_template: String,
    fields: Vec<SchemaFieldInput>,
    pipeline_steps: Vec<SchemaStepInput>,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

fn export_file_from(def: &SchemaDefinition) -> SchemaExportFile {
    // accepted_extensions is a JSON string column; the portable file
    // carries a real array. Malformed storage degrades to "none".
    let extensions: Vec<String> =
        serde_json::from_str(&def.schema.accepted_extensions).unwrap_or_default();
    SchemaExportFile {
        format: EXPORT_FORMAT.to_string(),
        version: EXPORT_VERSION,
        schema: PortableSchema {
            id: def.schema.id.clone(),
            name: def.schema.name.clone(),
            icon: def.schema.icon.clone(),
            description: def.schema.description.clone(),
            accepted_extensions: extensions,
            pipeline_template: def.schema.pipeline_template.clone(),
            fields: def
                .fields
                .iter()
                .map(|f| SchemaFieldInput {
                    field_key: f.field_key.clone(),
                    semantic: f.semantic.clone(),
                    field_type: f.field_type.clone(),
                    label: f.label.clone(),
                    options: f.options.clone(),
                    form_visible: f.form_visible,
                    card_visible: f.card_visible,
                    sortable: f.sortable,
                    filterable: f.filterable,
                    required: f.required,
                    order_index: f.order_index,
                })
                .collect(),
            pipeline_steps: def
                .pipeline_steps
                .iter()
                .map(|s| SchemaStepInput {
                    step_key: s.step_key.clone(),
                    label: s.label.clone(),
                    enabled: s.enabled,
                    order_index: s.order_index,
                })
                .collect(),
        },
    }
}

/// Parse and fully validate an export document. Runs the same rules
/// the editor's save path runs (slug shape, extension normalization,
/// field types, enum options, step vocab) so a hand-edited file fails
/// with the same error codes the UI already knows how to explain.
fn parse_and_validate(raw: &str) -> Result<SchemaDefinition, String> {
    let file: SchemaExportFile =
        serde_json::from_str(raw).map_err(|_| "IMPORT_NOT_A_SCHEMA_FILE".to_string())?;
    if file.format != EXPORT_FORMAT {
        return Err("IMPORT_NOT_A_SCHEMA_FILE".to_string());
    }
    if file.version != EXPORT_VERSION {
        return Err(format!("IMPORT_UNSUPPORTED_VERSION:{}", file.version));
    }

    let s = file.schema;
    validate_schema_id(&s.id)?;

    let payload = SchemaUpsertPayload {
        name: s.name,
        icon: s.icon,
        description: s.description,
        accepted_extensions: s.accepted_extensions,
        pipeline_template: s.pipeline_template,
        fields: s.fields,
    };
    // Returns the normalized extension list.
    let extensions = validate_payload(&payload)?;
    let extensions_json = serde_json::to_string(&extensions).map_err(|e| e.to_string())?;

    let steps = s.pipeline_steps;
    let mut seen = std::collections::HashSet::new();
    for step in &steps {
        if !PIPELINE_STEP_VOCAB.contains(&step.step_key.as_str()) {
            return Err("INVALID_STEP_KEY".to_string());
        }
        if step.label.trim().is_empty() {
            return Err("INVALID_STEP_LABEL".to_string());
        }
        if !seen.insert(step.step_key.as_str()) {
            return Err("DUPLICATE_STEP_KEY".to_string());
        }
    }

    // Synthesize the definition the editor expects. DB-only columns
    // get placeholder values — the editor never reads them, and rows
    // only materialize when the user hits Create (via schema_create).
    let id = s.id;
    Ok(SchemaDefinition {
        schema: SchemaRow {
            id: id.clone(),
            name: payload.name,
            icon: payload.icon,
            description: payload.description,
            accepted_extensions: extensions_json,
            pipeline_template: payload.pipeline_template,
            is_builtin: false,
            sort_order: 0,
            created_at: String::new(),
            updated_at: String::new(),
        },
        fields: payload
            .fields
            .into_iter()
            .map(|f| SchemaFieldRow {
                id: 0,
                schema_id: id.clone(),
                field_key: f.field_key,
                semantic: f.semantic,
                field_type: f.field_type,
                label: f.label,
                options: f.options,
                form_visible: f.form_visible,
                card_visible: f.card_visible,
                sortable: f.sortable,
                filterable: f.filterable,
                required: f.required,
                order_index: f.order_index,
            })
            .collect(),
        pipeline_steps: steps
            .into_iter()
            .map(|st| SchemaPipelineStepRow {
                id: 0,
                schema_id: id.clone(),
                step_key: st.step_key,
                label: st.label,
                enabled: st.enabled,
                order_index: st.order_index,
            })
            .collect(),
    })
}

/// Serialize a schema to a JSON file. `path` should come from a native
/// save dialog. Built-ins are exportable too — importing one lands as
/// an editable custom copy (the slug must differ, `schema_create`
/// enforces `SCHEMA_ID_EXISTS`).
#[tauri::command]
pub async fn schema_export(app: tauri::AppHandle, id: String, path: String) -> Result<(), String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let def = super::schema_admin::fetch_definition(&pool, &id).await?;
    let json = serde_json::to_string_pretty(&export_file_from(&def))
        .map_err(|e| format!("EXPORT_SERIALIZE_FAILED: {e}"))?;
    std::fs::write(Path::new(&path), json)
        .map_err(|e| format!("EXPORT_WRITE_FAILED: {e}"))
}

/// Read and validate an exported schema file. Returns the definition
/// unpersisted — no rows are written; the caller prefills the schema
/// editor and creation rides `schema_create` + `schema_steps_update`.
#[tauri::command]
pub async fn schema_import_read(path: String) -> Result<SchemaDefinition, String> {
    let raw = std::fs::read_to_string(Path::new(&path))
        .map_err(|e| format!("IMPORT_READ_FAILED: {e}"))?;
    parse_and_validate(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition() -> SchemaDefinition {
        SchemaDefinition {
            schema: SchemaRow {
                id: "podcast".to_string(),
                name: "Podcast".to_string(),
                icon: Some("mic".to_string()),
                description: Some("Audio episodes".to_string()),
                accepted_extensions: r#"["mp3","m4b"]"#.to_string(),
                pipeline_template: "novel".to_string(),
                is_builtin: false,
                sort_order: 4,
                created_at: "2026-01-01".to_string(),
                updated_at: "2026-01-01".to_string(),
            },
            fields: vec![
                SchemaFieldRow {
                    id: 1,
                    schema_id: "podcast".to_string(),
                    field_key: "display_name".to_string(),
                    semantic: Some("display_name".to_string()),
                    field_type: "builtin".to_string(),
                    label: "Display Name".to_string(),
                    options: None,
                    form_visible: true,
                    card_visible: false,
                    sortable: true,
                    filterable: false,
                    required: false,
                    order_index: 0,
                },
                SchemaFieldRow {
                    id: 2,
                    schema_id: "podcast".to_string(),
                    field_key: "episode".to_string(),
                    semantic: None,
                    field_type: "number".to_string(),
                    label: "Episode".to_string(),
                    options: None,
                    form_visible: true,
                    card_visible: true,
                    sortable: true,
                    filterable: true,
                    required: false,
                    order_index: 1,
                },
            ],
            pipeline_steps: vec![SchemaPipelineStepRow {
                id: 1,
                schema_id: "podcast".to_string(),
                step_key: "filename".to_string(),
                label: "Filename extraction".to_string(),
                enabled: true,
                order_index: 0,
            }],
        }
    }

    #[test]
    fn round_trip_preserves_everything() {
        let def = definition();
        let json = serde_json::to_string(&export_file_from(&def)).unwrap();
        let back = parse_and_validate(&json).unwrap();

        assert_eq!(back.schema.id, "podcast");
        assert_eq!(back.schema.name, "Podcast");
        assert_eq!(back.schema.icon.as_deref(), Some("mic"));
        assert_eq!(back.schema.description.as_deref(), Some("Audio episodes"));
        assert_eq!(back.schema.pipeline_template, "novel");
        assert!(!back.schema.is_builtin);
        assert_eq!(back.fields.len(), 2);
        assert_eq!(back.fields[0].field_key, "display_name");
        assert_eq!(back.fields[0].semantic.as_deref(), Some("display_name"));
        assert_eq!(back.fields[1].field_key, "episode");
        assert_eq!(back.fields[1].field_type, "number");
        assert!(back.fields[1].card_visible);
        assert_eq!(back.pipeline_steps.len(), 1);
        assert_eq!(back.pipeline_steps[0].step_key, "filename");
        assert!(back.pipeline_steps[0].enabled);
        // Extensions survived as a parseable JSON array.
        let exts: Vec<String> = serde_json::from_str(&back.schema.accepted_extensions).unwrap();
        assert_eq!(exts, vec!["mp3", "m4b"]);
    }

    #[test]
    fn import_rejects_wrong_envelope() {
        assert_eq!(
            parse_and_validate("not json").unwrap_err(),
            "IMPORT_NOT_A_SCHEMA_FILE"
        );
        assert_eq!(
            parse_and_validate(r#"{"format":"other","version":1,"schema":null}"#).unwrap_err(),
            "IMPORT_NOT_A_SCHEMA_FILE"
        );
        let mut file = export_file_from(&definition());
        file.version = 2;
        let json = serde_json::to_string(&file).unwrap();
        assert_eq!(
            parse_and_validate(&json).unwrap_err(),
            "IMPORT_UNSUPPORTED_VERSION:2"
        );
    }

    #[test]
    fn import_reuses_create_validation_rules() {
        // Uppercase slug.
        let mut file = export_file_from(&definition());
        file.schema.id = "Podcast".to_string();
        assert_eq!(
            parse_and_validate(&serde_json::to_string(&file).unwrap()).unwrap_err(),
            "INVALID_SCHEMA_ID"
        );

        // Unknown field type.
        let mut file = export_file_from(&definition());
        file.schema.fields[1].field_type = "widget".to_string();
        assert_eq!(
            parse_and_validate(&serde_json::to_string(&file).unwrap()).unwrap_err(),
            "INVALID_FIELD_TYPE"
        );

        // Duplicate step keys.
        let mut file = export_file_from(&definition());
        file.schema.pipeline_steps.push(SchemaStepInput {
            step_key: "filename".to_string(),
            label: "Duplicate".to_string(),
            enabled: true,
            order_index: 1,
        });
        assert_eq!(
            parse_and_validate(&serde_json::to_string(&file).unwrap()).unwrap_err(),
            "DUPLICATE_STEP_KEY"
        );

        // Step outside the fixed vocab.
        let mut file = export_file_from(&definition());
        file.schema.pipeline_steps[0].step_key = "custom_step".to_string();
        assert_eq!(
            parse_and_validate(&serde_json::to_string(&file).unwrap()).unwrap_err(),
            "INVALID_STEP_KEY"
        );
    }

    #[test]
    fn import_normalizes_extensions() {
        let mut file = export_file_from(&definition());
        file.schema.accepted_extensions = vec![" MP3 ".to_string(), ".m4b".to_string()];
        let back = parse_and_validate(&serde_json::to_string(&file).unwrap()).unwrap();
        let exts: Vec<String> = serde_json::from_str(&back.schema.accepted_extensions).unwrap();
        assert_eq!(exts, vec!["mp3", "m4b"]);
    }
}
