//! Write side of runtime-defined schemas: create / update / delete.
//!
//! Read model + read-side validators live in `commands::schemas`; this
//! module owns the mutations and their validation. Split out because
//! the repo enforces an 800-line per-file guard
//! (tests/unit/sourceFileSize.test.ts).
//!
//! Invariants enforced here:
//!   - slug (`schemas.id`) is immutable after creation and must match
//!     `^[a-z][a-z0-9_]*$` — it is referenced by categories and prompts
//!     as a plain string.
//!   - built-in schemas (is_builtin = 1) are editable but not
//!     deletable.
//!   - fields carrying a `semantic` (code behavior attaches to them)
//!     cannot be removed via update; custom fields can, and their
//!     stored values (metadata EAV rows for files under this schema)
//!     are deleted with them — the frontend warns with the affected
//!     count from `schema_field_data_count` first.
//!   - deleting a schema is blocked while any category references it.

use serde::Deserialize;
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

use super::schemas::{SchemaDefinition, SchemaFieldRow, SchemaPipelineStepRow, SchemaRow};

/// Built-in pipeline compositions a schema can route through.
pub const PIPELINE_TEMPLATES: &[&str] = &["novel", "comic", "galgame"];

/// Renderer types allowed for user-defined (semantic-less) fields.
pub const CUSTOM_FIELD_TYPES: &[&str] = &["text", "number", "rating", "date", "enum", "bool"];

/// Pipeline steps the schema editor can toggle/reorder. The set a
/// schema carries is copied from its pipeline template at creation;
/// this vocab is the universe those steps come from.
pub const PIPELINE_STEP_VOCAB: &[&str] =
    &["filename", "content", "cover_pick", "filename_folder"];

/// Code-behavior markers a field can carry. Code keys off these, never
/// off the field key itself; users cannot invent new ones.
pub const SEMANTICS: &[&str] = &[
    "display_name",
    "category",
    "authors",
    "tags",
    "progress",
    "cover",
    "volume",
    "file_path",
];

#[derive(Debug, Deserialize, Clone)]
pub struct SchemaFieldInput {
    pub field_key: String,
    pub semantic: Option<String>,
    pub field_type: String,
    pub label: String,
    pub options: Option<String>,
    pub form_visible: bool,
    pub card_visible: bool,
    pub sortable: bool,
    pub filterable: bool,
    pub required: bool,
    pub order_index: i64,
}

#[derive(Debug, Deserialize)]
pub struct SchemaUpsertPayload {
    pub name: String,
    pub icon: Option<String>,
    pub description: Option<String>,
    /// Lowercased extensions without the dot. Normalized (trimmed,
    /// lowercased, deduped) before storage as a JSON array.
    pub accepted_extensions: Vec<String>,
    pub pipeline_template: String,
    pub fields: Vec<SchemaFieldInput>,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

// ── Validation ──────────────────────────────────────────────────────

fn is_valid_key(s: &str) -> bool {
    // Slug-safe identifier: starts lowercase alpha, then alnum/underscore.
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn validate_schema_id(id: &str) -> Result<(), String> {
    if id.len() > 50 || !is_valid_key(id) {
        return Err("INVALID_SCHEMA_ID".to_string());
    }
    Ok(())
}

/// Trim + lowercase + dedupe, preserving first-seen order. Rejects
/// empties and anything with a dot or non-alphanumeric characters.
fn normalize_extensions(raw: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for ext in raw {
        let e = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        if e.is_empty() {
            continue;
        }
        if !e.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err("INVALID_EXTENSIONS".to_string());
        }
        if !out.contains(&e) {
            out.push(e);
        }
    }
    Ok(out)
}

fn validate_payload(payload: &SchemaUpsertPayload) -> Result<Vec<String>, String> {
    if payload.name.trim().is_empty() {
        return Err("INVALID_SCHEMA_NAME".to_string());
    }
    if !PIPELINE_TEMPLATES.contains(&payload.pipeline_template.as_str()) {
        return Err("INVALID_PIPELINE_TEMPLATE".to_string());
    }
    let extensions = normalize_extensions(&payload.accepted_extensions)?;
    if extensions.is_empty() {
        return Err("INVALID_EXTENSIONS".to_string());
    }

    for field in &payload.fields {
        if !is_valid_key(&field.field_key) {
            return Err("INVALID_FIELD_KEY".to_string());
        }
        if field.label.trim().is_empty() {
            return Err("INVALID_FIELD_LABEL".to_string());
        }
        match &field.semantic {
            Some(sem) => {
                // Semantic fields are code-backed: fixed vocab, builtin
                // renderer, no options list.
                if !SEMANTICS.contains(&sem.as_str()) {
                    return Err("INVALID_FIELD_SEMANTIC".to_string());
                }
                if field.field_type != "builtin" {
                    return Err("INVALID_FIELD_TYPE".to_string());
                }
            }
            None => {
                if !CUSTOM_FIELD_TYPES.contains(&field.field_type.as_str()) {
                    return Err("INVALID_FIELD_TYPE".to_string());
                }
                if field.field_type == "enum" {
                    let ok = field
                        .options
                        .as_deref()
                        .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
                        .map(|opts| !opts.is_empty())
                        .unwrap_or(false);
                    if !ok {
                        return Err("INVALID_FIELD_OPTIONS".to_string());
                    }
                }
            }
        }
    }

    // Keys must be unique within the payload (the table has
    // UNIQUE(schema_id, field_key), but fail with a clean error).
    let mut seen = std::collections::HashSet::new();
    for field in &payload.fields {
        if !seen.insert(field.field_key.as_str()) {
            return Err("DUPLICATE_FIELD_KEY".to_string());
        }
    }

    Ok(extensions)
}

// ── Read-back helper ────────────────────────────────────────────────

async fn fetch_definition(
    pool: &sqlx::SqlitePool,
    id: &str,
) -> Result<SchemaDefinition, String> {
    let schema: SchemaRow = sqlx::query_as(
        "SELECT id, name, icon, description, accepted_extensions, pipeline_template, \
                is_builtin, sort_order, created_at, updated_at \
         FROM schemas WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "SCHEMA_NOT_FOUND".to_string())?;

    let fields: Vec<SchemaFieldRow> = sqlx::query_as(
        "SELECT id, schema_id, field_key, semantic, field_type, label, options, \
                form_visible, card_visible, sortable, filterable, required, order_index \
         FROM schema_fields WHERE schema_id = ? ORDER BY order_index, id",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let pipeline_steps: Vec<SchemaPipelineStepRow> = sqlx::query_as(
        "SELECT id, schema_id, step_key, label, enabled, order_index \
         FROM schema_pipeline_steps WHERE schema_id = ? ORDER BY order_index, id",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(SchemaDefinition {
        schema,
        fields,
        pipeline_steps,
    })
}

// ── Commands ────────────────────────────────────────────────────────

/// Create a custom schema. Pipeline steps are copied from the chosen
/// template schema so the prompts page (and later pipeline editing)
/// starts from a sane default set.
#[tauri::command]
pub async fn schema_create(
    app: tauri::AppHandle,
    id: String,
    payload: SchemaUpsertPayload,
) -> Result<SchemaDefinition, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    validate_schema_id(&id)?;
    if super::schemas::schema_exists(&pool, &id).await? {
        return Err("SCHEMA_ID_EXISTS".to_string());
    }
    let extensions = validate_payload(&payload)?;
    let extensions_json = serde_json::to_string(&extensions).map_err(|e| e.to_string())?;

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let next_order: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM schemas",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "INSERT INTO schemas (id, name, icon, description, accepted_extensions, \
                              pipeline_template, is_builtin, sort_order) \
         VALUES (?, ?, ?, ?, ?, ?, 0, ?)",
    )
    .bind(&id)
    .bind(payload.name.trim())
    .bind(&payload.icon)
    .bind(&payload.description)
    .bind(&extensions_json)
    .bind(&payload.pipeline_template)
    .bind(next_order)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    insert_fields(&mut tx, &id, &payload.fields).await?;

    // Copy the template's enabled steps as the starting point.
    sqlx::query(
        "INSERT INTO schema_pipeline_steps (schema_id, step_key, label, enabled, order_index) \
         SELECT ?, step_key, label, enabled, order_index \
         FROM schema_pipeline_steps WHERE schema_id = ? ORDER BY order_index",
    )
    .bind(&id)
    .bind(&payload.pipeline_template)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    fetch_definition(&pool, &id).await
}

/// Update a schema's basic info and replace its field list. Pipeline
/// steps are untouched (step editing is a separate milestone). Fields
/// with a `semantic` from the current definition must all survive the
/// update; removed custom fields have their stored values deleted.
#[tauri::command]
pub async fn schema_update(
    app: tauri::AppHandle,
    id: String,
    payload: SchemaUpsertPayload,
) -> Result<SchemaDefinition, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let existing = fetch_definition(&pool, &id).await?;
    let extensions = validate_payload(&payload)?;
    let extensions_json = serde_json::to_string(&extensions).map_err(|e| e.to_string())?;

    // Every semantic-carrying field must survive, with its semantic
    // intact — pipeline code attaches behavior to these.
    for old in existing.fields.iter().filter(|f| f.semantic.is_some()) {
        let kept = payload
            .fields
            .iter()
            .find(|f| &f.field_key == &old.field_key && f.semantic == old.semantic);
        if kept.is_none() {
            return Err("BUILTIN_FIELD_REMOVED".to_string());
        }
    }

    // Custom fields dropped by this update: their values die with them.
    let new_keys: std::collections::HashSet<&str> = payload
        .fields
        .iter()
        .map(|f| f.field_key.as_str())
        .collect();
    let removed_custom: Vec<&str> = existing
        .fields
        .iter()
        .filter(|f| f.semantic.is_none() && !new_keys.contains(f.field_key.as_str()))
        .map(|f| f.field_key.as_str())
        .collect();

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    sqlx::query(
        "UPDATE schemas SET name = ?, icon = ?, description = ?, accepted_extensions = ?, \
                            pipeline_template = ?, updated_at = datetime('now') \
         WHERE id = ?",
    )
    .bind(payload.name.trim())
    .bind(&payload.icon)
    .bind(&payload.description)
    .bind(&extensions_json)
    .bind(&payload.pipeline_template)
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    for key in &removed_custom {
        delete_field_data(&mut tx, &id, key).await?;
    }

    sqlx::query("DELETE FROM schema_fields WHERE schema_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    insert_fields(&mut tx, &id, &payload.fields).await?;

    tx.commit().await.map_err(|e| e.to_string())?;
    fetch_definition(&pool, &id).await
}

/// Delete a custom schema. Built-ins are not deletable; a schema still
/// referenced by any category is not deletable either (the error names
/// the referencing count so the UI can explain). Prompts stored under
/// the slug go with it — they are unreachable once the schema is gone.
#[tauri::command]
pub async fn schema_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let row: Option<(bool,)> = sqlx::query_as("SELECT is_builtin FROM schemas WHERE id = ?")
        .bind(&id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;
    let Some((is_builtin,)) = row else {
        return Err("SCHEMA_NOT_FOUND".to_string());
    };
    if is_builtin {
        return Err("SCHEMA_BUILTIN_NOT_DELETABLE".to_string());
    }

    let referencing: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM categories WHERE schema_slug = ?",
    )
    .bind(&id)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    if referencing > 0 {
        return Err(format!("SCHEMA_IN_USE:{referencing}"));
    }

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM prompts WHERE schema_slug = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    // Children (fields, steps) cascade from the schemas row.
    sqlx::query("DELETE FROM schemas WHERE id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// How many metadata values exist for a field, scoped to files under
/// this schema. The field designer calls this before removing a custom
/// field to render the "N files will lose this value" warning.
#[tauri::command]
pub async fn schema_field_data_count(
    app: tauri::AppHandle,
    schema_id: String,
    field_key: String,
) -> Result<i64, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    count_field_data(&pool, &schema_id, &field_key).await
}

#[derive(Debug, Deserialize)]
pub struct SchemaStepInput {
    pub step_key: String,
    pub label: String,
    pub enabled: bool,
    pub order_index: i64,
}

/// Replace a schema's pipeline steps (toggle + reorder from the schema
/// editor). Step keys must come from the fixed vocab — the UI never
/// invents steps, it only enables/disables and reorders the ones the
/// schema's template provides. Note `order_index` is presentational
/// (prompts page + editor ordering); execution order inside a pipeline
/// composition stays code-defined. `enabled` is the functional bit:
/// pipeline nodes consult it via `PipelineEnv::enabled_steps`.
#[tauri::command]
pub async fn schema_steps_update(
    app: tauri::AppHandle,
    id: String,
    steps: Vec<SchemaStepInput>,
) -> Result<SchemaDefinition, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    if !super::schemas::schema_exists(&pool, &id).await? {
        return Err("SCHEMA_NOT_FOUND".to_string());
    }
    for step in &steps {
        if !PIPELINE_STEP_VOCAB.contains(&step.step_key.as_str()) {
            return Err("INVALID_STEP_KEY".to_string());
        }
        if step.label.trim().is_empty() {
            return Err("INVALID_STEP_LABEL".to_string());
        }
    }

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM schema_pipeline_steps WHERE schema_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    for step in &steps {
        sqlx::query(
            "INSERT INTO schema_pipeline_steps (schema_id, step_key, label, enabled, order_index) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&step.step_key)
        .bind(step.label.trim())
        .bind(step.enabled)
        .bind(step.order_index)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    fetch_definition(&pool, &id).await
}

// ── Shared internals ────────────────────────────────────────────────

async fn count_field_data(
    pool: &sqlx::SqlitePool,
    schema_id: &str,
    field_key: &str,
) -> Result<i64, String> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM metadata \
         WHERE key = ? AND file_id IN ( \
             SELECT f.id FROM files f JOIN categories c ON f.category_id = c.id \
             WHERE c.schema_slug = ? \
         )",
    )
    .bind(field_key)
    .bind(schema_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())
}

async fn delete_field_data(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    schema_id: &str,
    field_key: &str,
) -> Result<(), String> {
    sqlx::query(
        "DELETE FROM metadata \
         WHERE key = ? AND file_id IN ( \
             SELECT f.id FROM files f JOIN categories c ON f.category_id = c.id \
             WHERE c.schema_slug = ? \
         )",
    )
    .bind(field_key)
    .bind(schema_id)
    .execute(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn insert_fields(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    schema_id: &str,
    fields: &[SchemaFieldInput],
) -> Result<(), String> {
    for field in fields {
        sqlx::query(
            "INSERT INTO schema_fields (schema_id, field_key, semantic, field_type, label, \
                                        options, form_visible, card_visible, sortable, \
                                        filterable, required, order_index) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(schema_id)
        .bind(&field.field_key)
        .bind(&field.semantic)
        .bind(&field.field_type)
        .bind(field.label.trim())
        .bind(&field.options)
        .bind(field.form_visible)
        .bind(field.card_visible)
        .bind(field.sortable)
        .bind(field.filterable)
        .bind(field.required)
        .bind(field.order_index)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory DB with the v6 tables (plus the categories/files/
    /// metadata slices the data-count and delete paths touch) and the
    /// built-in seeds.
    async fn seeded_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE schemas (\
                id TEXT PRIMARY KEY,\
                name TEXT NOT NULL,\
                icon TEXT,\
                description TEXT,\
                accepted_extensions TEXT NOT NULL DEFAULT '[]',\
                pipeline_template TEXT NOT NULL DEFAULT 'novel',\
                is_builtin BOOLEAN NOT NULL DEFAULT 0,\
                sort_order INTEGER NOT NULL DEFAULT 0,\
                created_at TEXT NOT NULL DEFAULT (datetime('now')),\
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))\
            );\
            CREATE TABLE schema_fields (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                schema_id TEXT NOT NULL REFERENCES schemas(id) ON DELETE CASCADE,\
                field_key TEXT NOT NULL,\
                semantic TEXT,\
                field_type TEXT NOT NULL DEFAULT 'builtin',\
                label TEXT NOT NULL,\
                options TEXT,\
                form_visible BOOLEAN NOT NULL DEFAULT 1,\
                card_visible BOOLEAN NOT NULL DEFAULT 0,\
                sortable BOOLEAN NOT NULL DEFAULT 0,\
                filterable BOOLEAN NOT NULL DEFAULT 0,\
                required BOOLEAN NOT NULL DEFAULT 0,\
                order_index INTEGER NOT NULL DEFAULT 0,\
                UNIQUE(schema_id, field_key)\
            );\
            CREATE TABLE schema_pipeline_steps (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                schema_id TEXT NOT NULL REFERENCES schemas(id) ON DELETE CASCADE,\
                step_key TEXT NOT NULL,\
                label TEXT NOT NULL,\
                enabled BOOLEAN NOT NULL DEFAULT 1,\
                order_index INTEGER NOT NULL DEFAULT 0,\
                UNIQUE(schema_id, step_key)\
            );\
            CREATE TABLE categories (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                name TEXT NOT NULL UNIQUE,\
                schema_slug TEXT NOT NULL DEFAULT 'novel'\
            );\
            CREATE TABLE files (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                category_id INTEGER\
            );\
            CREATE TABLE metadata (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                file_id INTEGER,\
                key TEXT,\
                value TEXT\
            );\
            CREATE TABLE prompts (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                schema_slug TEXT\
            );\
            INSERT INTO schemas (id, name, accepted_extensions, pipeline_template, is_builtin, sort_order) VALUES \
                ('novel', 'Novel', '[\"txt\"]', 'novel', 1, 0),\
                ('comic', 'Comic', '[\"cbz\",\"zip\",\"cbr\",\"rar\"]', 'comic', 1, 1);\
            INSERT INTO schema_fields (schema_id, field_key, semantic, field_type, label, form_visible, card_visible, order_index) VALUES \
                ('novel', 'display_name', 'display_name', 'builtin', 'Display Name', 1, 0, 0),\
                ('novel', 'authors', 'authors', 'builtin', 'Authors', 1, 1, 1);\
            INSERT INTO schema_pipeline_steps (schema_id, step_key, label, enabled, order_index) VALUES \
                ('novel', 'filename', 'Filename extraction', 1, 0),\
                ('novel', 'content', 'Content analysis', 1, 1);",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    fn builtin_field(key: &str, order: i64) -> SchemaFieldInput {
        SchemaFieldInput {
            field_key: key.to_string(),
            semantic: Some(key.to_string()),
            field_type: "builtin".to_string(),
            label: key.to_string(),
            options: None,
            form_visible: true,
            card_visible: false,
            sortable: false,
            filterable: false,
            required: false,
            order_index: order,
        }
    }

    fn custom_field(key: &str, field_type: &str, order: i64) -> SchemaFieldInput {
        SchemaFieldInput {
            field_key: key.to_string(),
            semantic: None,
            field_type: field_type.to_string(),
            label: key.to_string(),
            options: if field_type == "enum" {
                Some("[\"a\",\"b\"]".to_string())
            } else {
                None
            },
            form_visible: true,
            card_visible: false,
            sortable: false,
            filterable: false,
            required: false,
            order_index: order,
        }
    }

    fn payload(fields: Vec<SchemaFieldInput>) -> SchemaUpsertPayload {
        SchemaUpsertPayload {
            name: "Podcast".to_string(),
            icon: None,
            description: None,
            accepted_extensions: vec!["mp3".to_string()],
            pipeline_template: "novel".to_string(),
            fields,
        }
    }

    #[test]
    fn schema_id_rules() {
        assert!(validate_schema_id("podcast").is_ok());
        assert!(validate_schema_id("audio_book2").is_ok());
        assert!(validate_schema_id("Podcast").is_err());
        assert!(validate_schema_id("2books").is_err());
        assert!(validate_schema_id("has-dash").is_err());
        assert!(validate_schema_id("").is_err());
    }

    #[test]
    fn extension_normalization() {
        let exts = normalize_extensions(&[
            " MP3 ".to_string(),
            ".mp3".to_string(),
            "Flac".to_string(),
        ])
        .unwrap();
        assert_eq!(exts, vec!["mp3", "flac"]);
        assert!(normalize_extensions(&["a b".to_string()]).is_err());
    }

    #[test]
    fn payload_validation_rejects_bad_fields() {
        assert_eq!(
            validate_payload(&payload(vec![custom_field("Bad", "text", 0)])).unwrap_err(),
            "INVALID_FIELD_KEY"
        );
        assert_eq!(
            validate_payload(&payload(vec![custom_field("x", "widget", 0)])).unwrap_err(),
            "INVALID_FIELD_TYPE"
        );
        assert_eq!(
            validate_payload(&payload(vec![SchemaFieldInput {
                options: None,
                ..custom_field("status", "enum", 0)
            }]))
            .unwrap_err(),
            "INVALID_FIELD_OPTIONS"
        );
        let mut dup = payload(vec![custom_field("a", "text", 0), custom_field("a", "text", 1)]);
        assert_eq!(
            validate_payload(&dup).unwrap_err(),
            "DUPLICATE_FIELD_KEY"
        );
        dup.pipeline_template = "weird".to_string();
        assert_eq!(
            validate_payload(&dup).unwrap_err(),
            "INVALID_PIPELINE_TEMPLATE"
        );
    }

    #[tokio::test]
    async fn count_field_data_scopes_to_schema_files() {
        let pool = seeded_pool().await;
        // Two categories on different schemas, one file each, both with
        // a 'publisher' metadata row — only the novel one counts.
        sqlx::query(
            "INSERT INTO categories (id, name, schema_slug) VALUES (1, 'Books', 'novel'), (2, 'Manga', 'comic');\
             INSERT INTO files (id, category_id) VALUES (10, 1), (20, 2);\
             INSERT INTO metadata (file_id, key, value) VALUES (10, 'publisher', 'A'), (20, 'publisher', 'B');",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(
            count_field_data(&pool, "novel", "publisher").await.unwrap(),
            1
        );
        assert_eq!(
            count_field_data(&pool, "comic", "publisher").await.unwrap(),
            1
        );
        assert_eq!(
            count_field_data(&pool, "novel", "missing").await.unwrap(),
            0
        );
    }
}
