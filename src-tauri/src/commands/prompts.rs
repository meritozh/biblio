use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

use crate::schema::SchemaSlug;

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Prompt {
    pub id: i64,
    pub name: String,
    pub content: String,
    /// Legacy free-text label, kept for back-compat with rows written
    /// before the schema-slug refactor. The active discriminator is
    /// `(schema_slug, step)`.
    pub category: Option<String>,
    /// Legacy mime_group column. Kept readable for one release while
    /// callers migrate to `schema_slug`; will be dropped in a follow-up
    /// migration.
    pub mime_group: String,
    /// Built-in schema slug (`'novel'` / `'comic'`). Mirrors
    /// `Category.schema_slug` and is the active key for prompt lookup.
    pub schema_slug: Option<String>,
    /// Pipeline step the prompt feeds. Novel: `'filename'`, `'content'`.
    /// Comic: `'filename'`, `'cover_pick'`, `'filename_folder'`.
    pub step: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptCreate {
    pub name: String,
    pub content: String,
    pub schema_slug: String,
    pub step: String,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

const PROMPT_SELECT: &str =
    "SELECT id, name, content, category, mime_group, schema_slug, step, is_default, created_at, updated_at FROM prompts";

/// Validate that a (schema_slug, step) pair is one we know about.
/// Pairs:
///   `(novel, filename)`, `(novel, content)`,
///   `(comic, filename)`, `(comic, cover_pick)`, `(comic, filename_folder)`.
/// `filename_folder` exists because the comic pipeline picks between
/// archive and image-folder ingestion at runtime, and the two need
/// different filename-extraction rules (folder names already encode the
/// author, archive names don't).
fn validate_slug_step(slug: &str, step: &str) -> Result<(), String> {
    if !SchemaSlug::is_known(slug) {
        return Err("INVALID_PROMPT_SCHEMA_STEP".to_string());
    }
    let canonical = SchemaSlug::from_str(slug);
    match (canonical, step) {
        (SchemaSlug::Novel, "filename")
        | (SchemaSlug::Novel, "content")
        | (SchemaSlug::Comic, "filename")
        | (SchemaSlug::Comic, "cover_pick")
        | (SchemaSlug::Comic, "filename_folder")
        | (SchemaSlug::Galgame, "filename") => Ok(()),
        _ => Err("INVALID_PROMPT_SCHEMA_STEP".to_string()),
    }
}

/// Map a schema slug back to a legacy mime_group value. Used during
/// INSERT/UPDATE so the legacy column we keep around for one release
/// stays consistent with the new row.
fn legacy_mime_group(slug: SchemaSlug, step: &str) -> &'static str {
    match (slug, step) {
        (SchemaSlug::Novel, _) => "text",
        (SchemaSlug::Comic, "filename_folder") => "image_folder",
        (SchemaSlug::Comic, _) => "archive",
        (SchemaSlug::Galgame, _) => "game",
    }
}

fn legacy_category_label(slug: SchemaSlug, step: &str) -> String {
    // Pre-v3 callers stored 'filename' / 'content' in `category` for
    // novel/text steps; preserve that exact token there. Comic and
    // image_folder rows use `<group>_<step>` to avoid collision.
    match (slug, step) {
        (SchemaSlug::Novel, s) => s.to_string(),
        (slug, s) => format!("{}_{}", legacy_mime_group(slug, s), s),
    }
}

/// Fetch the content of the currently-active prompt for a given
/// (schema_slug, step) pair. Used by `llm.rs` to build preambles.
pub async fn prompt_get_active(
    pool: &sqlx::SqlitePool,
    schema_slug: SchemaSlug,
    step: &str,
) -> Result<String, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM prompts WHERE schema_slug = ? AND step = ? AND is_default = 1 LIMIT 1",
    )
    .bind(schema_slug.as_str())
    .bind(step)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    row.map(|(c,)| c)
        .ok_or_else(|| format!("NO_ACTIVE_PROMPT: {}/{}", schema_slug.as_str(), step))
}

#[tauri::command]
pub async fn prompt_list(
    app: tauri::AppHandle,
    schema_slug: Option<String>,
    step: Option<String>,
) -> Result<Vec<Prompt>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let prompts: Vec<Prompt> = match (schema_slug.as_deref(), step.as_deref()) {
        (Some(slug), Some(s)) => sqlx::query_as(
            "SELECT id, name, content, category, mime_group, schema_slug, step, is_default, created_at, updated_at FROM prompts WHERE schema_slug = ? AND step = ? ORDER BY created_at DESC",
        )
        .bind(slug)
        .bind(s)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
        (Some(slug), None) => sqlx::query_as(
            "SELECT id, name, content, category, mime_group, schema_slug, step, is_default, created_at, updated_at FROM prompts WHERE schema_slug = ? ORDER BY step, created_at DESC",
        )
        .bind(slug)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
        _ => sqlx::query_as(
            "SELECT id, name, content, category, mime_group, schema_slug, step, is_default, created_at, updated_at FROM prompts ORDER BY schema_slug, step, created_at DESC",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
    };

    Ok(prompts)
}

#[tauri::command]
pub async fn prompt_create(
    app: tauri::AppHandle,
    payload: PromptCreate,
) -> Result<Prompt, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    validate_slug_step(&payload.schema_slug, &payload.step)?;
    let slug = SchemaSlug::from_str(&payload.schema_slug);
    let legacy_group = legacy_mime_group(slug, &payload.step);
    let legacy_category = legacy_category_label(slug, &payload.step);

    let id = sqlx::query(
        "INSERT INTO prompts (name, content, category, mime_group, schema_slug, step, is_default) VALUES (?, ?, ?, ?, ?, ?, 0)",
    )
    .bind(&payload.name)
    .bind(&payload.content)
    .bind(&legacy_category)
    .bind(legacy_group)
    .bind(slug.as_str())
    .bind(&payload.step)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?
    .last_insert_rowid();

    let prompt: Prompt = sqlx::query_as(
        &format!("{PROMPT_SELECT} WHERE id = ?"),
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(prompt)
}

#[tauri::command]
pub async fn prompt_update(
    app: tauri::AppHandle,
    id: i64,
    payload: PromptCreate,
) -> Result<Prompt, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    validate_slug_step(&payload.schema_slug, &payload.step)?;
    let slug = SchemaSlug::from_str(&payload.schema_slug);
    let legacy_group = legacy_mime_group(slug, &payload.step);
    let legacy_category = legacy_category_label(slug, &payload.step);

    sqlx::query(
        "UPDATE prompts SET name = ?, content = ?, category = ?, mime_group = ?, schema_slug = ?, step = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&payload.name)
    .bind(&payload.content)
    .bind(&legacy_category)
    .bind(legacy_group)
    .bind(slug.as_str())
    .bind(&payload.step)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let prompt: Prompt = sqlx::query_as(
        &format!("{PROMPT_SELECT} WHERE id = ?"),
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(prompt)
}

#[tauri::command]
pub async fn prompt_delete(app: tauri::AppHandle, id: i64) -> Result<(), String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let is_default: bool = sqlx::query_scalar(
        "SELECT is_default FROM prompts WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if is_default {
        return Err("Cannot delete the default prompt".to_string());
    }

    sqlx::query("DELETE FROM prompts WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Per-(schema_slug, step) default switching: clears the active flag on
/// any sibling in the same slug + step, then marks `id` active.
pub async fn set_default_impl(
    pool: &sqlx::SqlitePool,
    id: i64,
) -> Result<Prompt, String> {
    let target: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT schema_slug, step FROM prompts WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let (slug_opt, step) = target.ok_or_else(|| "PROMPT_NOT_FOUND".to_string())?;
    // Legacy rows written before schema_slug existed may have it NULL. Refuse
    // to flip the active prompt for those — they need to be edited (the
    // update path will populate the column) before they can be activated.
    let slug = slug_opt.ok_or_else(|| "PROMPT_MISSING_SCHEMA_SLUG".to_string())?;
    validate_slug_step(&slug, &step)?;

    sqlx::query(
        "UPDATE prompts SET is_default = 0 WHERE schema_slug = ? AND step = ? AND is_default = 1",
    )
    .bind(&slug)
    .bind(&step)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "UPDATE prompts SET is_default = 1, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query_as(
        &format!("{PROMPT_SELECT} WHERE id = ?"),
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn prompt_set_default(
    app: tauri::AppHandle,
    id: i64,
) -> Result<Prompt, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    set_default_impl(&pool, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_slug_step_accepts_known_pairs() {
        assert!(validate_slug_step("novel", "filename").is_ok());
        assert!(validate_slug_step("novel", "content").is_ok());
        assert!(validate_slug_step("comic", "filename").is_ok());
        assert!(validate_slug_step("comic", "cover_pick").is_ok());
        assert!(validate_slug_step("comic", "filename_folder").is_ok());
    }

    #[test]
    fn validate_slug_step_rejects_unknown() {
        assert_eq!(
            validate_slug_step("novel", "cover_pick").unwrap_err(),
            "INVALID_PROMPT_SCHEMA_STEP"
        );
        assert_eq!(
            validate_slug_step("manga", "filename").unwrap_err(),
            "INVALID_PROMPT_SCHEMA_STEP"
        );
        // Retired with the category reclassify feature.
        assert_eq!(
            validate_slug_step("novel", "category_reanalyze").unwrap_err(),
            "INVALID_PROMPT_SCHEMA_STEP"
        );
    }

    #[test]
    fn legacy_mime_group_routes_filename_folder_to_image_folder() {
        assert_eq!(legacy_mime_group(SchemaSlug::Novel, "filename"), "text");
        assert_eq!(legacy_mime_group(SchemaSlug::Comic, "filename"), "archive");
        assert_eq!(legacy_mime_group(SchemaSlug::Comic, "cover_pick"), "archive");
        assert_eq!(
            legacy_mime_group(SchemaSlug::Comic, "filename_folder"),
            "image_folder"
        );
    }

    #[test]
    fn legacy_category_label_preserves_text_step_for_back_compat() {
        assert_eq!(legacy_category_label(SchemaSlug::Novel, "filename"), "filename");
        assert_eq!(legacy_category_label(SchemaSlug::Novel, "content"), "content");
        assert_eq!(
            legacy_category_label(SchemaSlug::Comic, "filename"),
            "archive_filename"
        );
        assert_eq!(
            legacy_category_label(SchemaSlug::Comic, "filename_folder"),
            "image_folder_filename_folder"
        );
    }
}
