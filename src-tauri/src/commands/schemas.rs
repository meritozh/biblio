//! Runtime-defined resource schemas.
//!
//! A schema (novel / comic / galgame / user-created) is a row in the
//! `schemas` table plus its `schema_fields` and `schema_pipeline_steps`
//! children. This module owns the read model consumed by the frontend
//! (`schema_list`) and the DB-backed validators other command modules
//! use now that slugs are data rather than a Rust enum.

use serde::Serialize;
use sqlx::FromRow;
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct SchemaRow {
    /// The slug — primary key, referenced by `categories.schema_slug`
    /// and `prompts.schema_slug`.
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub description: Option<String>,
    /// JSON array of lowercased extensions (no dot). Parsed by the
    /// frontend; kept opaque here.
    pub accepted_extensions: String,
    /// Which built-in node composition (`novel` / `comic` / `galgame`)
    /// the import pipeline runs for files under this schema.
    pub pipeline_template: String,
    pub is_builtin: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct SchemaFieldRow {
    pub id: i64,
    pub schema_id: String,
    pub field_key: String,
    /// Code-behavior marker: `authors` / `cover` / `progress` / ... .
    /// NULL for user-defined fields, which are pure data stored via the
    /// metadata EAV table and never touched by pipeline code.
    pub semantic: Option<String>,
    /// Renderer hint. `builtin` = the frontend switches on `semantic`;
    /// the custom types (text/number/rating/date/enum/bool) arrive with
    /// the schema editor UI.
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

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct SchemaPipelineStepRow {
    pub id: i64,
    pub schema_id: String,
    pub step_key: String,
    pub label: String,
    pub enabled: bool,
    pub order_index: i64,
}

/// Wire shape for `schema_list`: the schema row flattened, plus its
/// ordered children.
#[derive(Debug, Serialize)]
pub struct SchemaDefinition {
    #[serde(flatten)]
    pub schema: SchemaRow,
    pub fields: Vec<SchemaFieldRow>,
    pub pipeline_steps: Vec<SchemaPipelineStepRow>,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

/// True if `slug` (case-insensitive) names a row in `schemas`. Replaces
/// the old `SchemaSlug::is_known` enum check at command boundaries —
/// user-supplied slugs are validated against data, not code.
pub async fn schema_exists(pool: &sqlx::SqlitePool, slug: &str) -> Result<bool, String> {
    let found: Option<(String,)> = sqlx::query_as("SELECT id FROM schemas WHERE id = lower(?)")
        .bind(slug.trim())
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(found.is_some())
}

/// The pipeline template for a schema slug. Unknown slugs fall back to
/// the novel template, mirroring the historical enum fallback so a
/// stale `categories.schema_slug` keeps working instead of erroring
/// the import.
pub async fn pipeline_template_for(
    pool: &sqlx::SqlitePool,
    slug: &str,
) -> Result<String, String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT pipeline_template FROM schemas WHERE id = lower(?)")
            .bind(slug.trim())
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(row
        .map(|(t,)| t)
        .unwrap_or_else(|| crate::schema::FALLBACK.to_string()))
}

/// True if `(slug, step)` is an enabled pipeline step, i.e. a prompt may
/// be stored under that pair. Replaces the hard-coded pair whitelist in
/// `prompts.rs`.
pub async fn schema_step_exists(
    pool: &sqlx::SqlitePool,
    slug: &str,
    step: &str,
) -> Result<bool, String> {
    let found: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM schema_pipeline_steps \
         WHERE schema_id = lower(?) AND step_key = ? AND enabled = 1",
    )
    .bind(slug.trim())
    .bind(step)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(found.is_some())
}

#[tauri::command]
pub async fn schema_list(app: tauri::AppHandle) -> Result<Vec<SchemaDefinition>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let schemas: Vec<SchemaRow> = sqlx::query_as(
        "SELECT id, name, icon, description, accepted_extensions, pipeline_template, \
                is_builtin, sort_order, created_at, updated_at \
         FROM schemas ORDER BY sort_order, id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(schemas.len());
    for schema in schemas {
        let fields: Vec<SchemaFieldRow> = sqlx::query_as(
            "SELECT id, schema_id, field_key, semantic, field_type, label, options, \
                    form_visible, card_visible, sortable, filterable, required, order_index \
             FROM schema_fields WHERE schema_id = ? ORDER BY order_index, id",
        )
        .bind(&schema.id)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

        let pipeline_steps: Vec<SchemaPipelineStepRow> = sqlx::query_as(
            "SELECT id, schema_id, step_key, label, enabled, order_index \
             FROM schema_pipeline_steps WHERE schema_id = ? ORDER BY order_index, id",
        )
        .bind(&schema.id)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

        out.push(SchemaDefinition {
            schema,
            fields,
            pipeline_steps,
        });
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// In-memory DB with the v6 tables + built-in seeds, mirroring the
    /// migration. Shared by the validator tests here and usable by
    /// other command modules' tests via `test_helpers` later.
    async fn seeded_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE schemas (\
                id TEXT PRIMARY KEY,\
                name TEXT NOT NULL,\
                accepted_extensions TEXT NOT NULL DEFAULT '[]',\
                pipeline_template TEXT NOT NULL DEFAULT 'novel',\
                is_builtin BOOLEAN NOT NULL DEFAULT 0,\
                sort_order INTEGER NOT NULL DEFAULT 0\
            );\
            CREATE TABLE schema_pipeline_steps (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                schema_id TEXT NOT NULL,\
                step_key TEXT NOT NULL,\
                label TEXT NOT NULL,\
                enabled BOOLEAN NOT NULL DEFAULT 1,\
                order_index INTEGER NOT NULL DEFAULT 0\
            );\
            INSERT INTO schemas (id, name, pipeline_template, is_builtin, sort_order) VALUES \
                ('novel', 'Novel', 'novel', 1, 0),\
                ('comic', 'Comic', 'comic', 1, 1),\
                ('galgame', 'Galgame', 'galgame', 1, 2);\
            INSERT INTO schema_pipeline_steps (schema_id, step_key, label, enabled, order_index) VALUES \
                ('novel', 'filename', 'Filename extraction', 1, 0),\
                ('novel', 'content', 'Content analysis', 1, 1),\
                ('comic', 'filename', 'Filename extraction', 1, 0),\
                ('comic', 'cover_pick', 'Cover detection', 1, 1),\
                ('comic', 'filename_folder', 'Folder filename extraction', 1, 2),\
                ('galgame', 'filename', 'Filename extraction', 1, 0);",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn schema_exists_is_case_insensitive() {
        let pool = seeded_pool().await;
        assert!(schema_exists(&pool, "novel").await.unwrap());
        assert!(schema_exists(&pool, "Comic").await.unwrap());
        assert!(!schema_exists(&pool, "manga").await.unwrap());
        assert!(!schema_exists(&pool, "").await.unwrap());
    }

    #[tokio::test]
    async fn pipeline_template_falls_back_for_unknown_slug() {
        let pool = seeded_pool().await;
        assert_eq!(
            pipeline_template_for(&pool, "galgame").await.unwrap(),
            "galgame"
        );
        assert_eq!(
            pipeline_template_for(&pool, "manga").await.unwrap(),
            crate::schema::FALLBACK
        );
    }

    #[tokio::test]
    async fn schema_step_matches_seeded_pairs() {
        let pool = seeded_pool().await;
        assert!(schema_step_exists(&pool, "novel", "filename").await.unwrap());
        assert!(schema_step_exists(&pool, "novel", "content").await.unwrap());
        assert!(schema_step_exists(&pool, "comic", "cover_pick").await.unwrap());
        assert!(
            schema_step_exists(&pool, "comic", "filename_folder")
                .await
                .unwrap()
        );
        assert!(!schema_step_exists(&pool, "novel", "cover_pick").await.unwrap());
        assert!(!schema_step_exists(&pool, "manga", "filename").await.unwrap());
    }
}
