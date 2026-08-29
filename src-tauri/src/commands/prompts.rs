use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

use crate::commands::schemas::schema_step_exists;

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

/// Validate that a (schema_slug, step) pair exists as an enabled
/// pipeline step in the DB (seeded pairs: `(novel, filename)`,
/// `(novel, content)`, `(comic, filename)`, `(comic, cover_pick)`,
/// `(comic, filename_folder)`, `(galgame, filename)`).
/// `filename_folder` exists because the comic pipeline picks between
/// archive and image-folder ingestion at runtime, and the two need
/// different filename-extraction rules (folder names already encode the
/// author, archive names don't).
async fn validate_slug_step(
    pool: &sqlx::SqlitePool,
    slug: &str,
    step: &str,
) -> Result<(), String> {
    if schema_step_exists(pool, slug, step).await? {
        Ok(())
    } else {
        Err("INVALID_PROMPT_SCHEMA_STEP".to_string())
    }
}

/// Map a schema slug back to a legacy mime_group value. Used during
/// INSERT/UPDATE so the legacy column we keep around for one release
/// stays consistent with the new row. Custom schemas store their own
/// slug — the column is read by nothing on this build, it just needs
/// to be non-NULL and stable.
fn legacy_mime_group(slug: &str, step: &str) -> String {
    match (slug, step) {
        ("novel", _) => "text".to_string(),
        ("comic", "filename_folder") => "image_folder".to_string(),
        ("comic", _) => "archive".to_string(),
        ("galgame", _) => "game".to_string(),
        (other, _) => other.to_string(),
    }
}

fn legacy_category_label(slug: &str, step: &str) -> String {
    // Pre-v3 callers stored 'filename' / 'content' in `category` for
    // novel/text steps; preserve that exact token there. Comic and
    // image_folder rows use `<group>_<step>` to avoid collision.
    match slug {
        "novel" => step.to_string(),
        other => format!("{}_{}", legacy_mime_group(other, step), step),
    }
}

/// Fetch the content of the currently-active prompt for a given
/// (schema_slug, step) pair. Used by `llm.rs` to build preambles.
pub async fn prompt_get_active(
    pool: &sqlx::SqlitePool,
    schema_slug: &str,
    step: &str,
) -> Result<String, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM prompts WHERE schema_slug = ? AND step = ? AND is_default = 1 LIMIT 1",
    )
    .bind(schema_slug)
    .bind(step)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    row.map(|(c,)| c)
        .ok_or_else(|| format!("NO_ACTIVE_PROMPT: {}/{}", schema_slug, step))
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

    validate_slug_step(&pool, &payload.schema_slug, &payload.step).await?;
    let slug = payload.schema_slug.trim().to_ascii_lowercase();
    let legacy_group = legacy_mime_group(&slug, &payload.step);
    let legacy_category = legacy_category_label(&slug, &payload.step);

    let id = sqlx::query(
        "INSERT INTO prompts (name, content, category, mime_group, schema_slug, step, is_default) VALUES (?, ?, ?, ?, ?, ?, 0)",
    )
    .bind(&payload.name)
    .bind(&payload.content)
    .bind(&legacy_category)
    .bind(&legacy_group)
    .bind(&slug)
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

    validate_slug_step(&pool, &payload.schema_slug, &payload.step).await?;
    let slug = payload.schema_slug.trim().to_ascii_lowercase();
    let legacy_group = legacy_mime_group(&slug, &payload.step);
    let legacy_category = legacy_category_label(&slug, &payload.step);

    sqlx::query(
        "UPDATE prompts SET name = ?, content = ?, category = ?, mime_group = ?, schema_slug = ?, step = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&payload.name)
    .bind(&payload.content)
    .bind(&legacy_category)
    .bind(&legacy_group)
    .bind(&slug)
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
    validate_slug_step(pool, &slug, &step).await?;

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

    /// In-memory DB with the v6 schema tables + built-in step seeds.
    async fn seeded_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE schema_pipeline_steps (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                schema_id TEXT NOT NULL,\
                step_key TEXT NOT NULL,\
                label TEXT NOT NULL,\
                enabled BOOLEAN NOT NULL DEFAULT 1,\
                order_index INTEGER NOT NULL DEFAULT 0\
            );\
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
    async fn validate_slug_step_accepts_known_pairs() {
        let pool = seeded_pool().await;
        assert!(validate_slug_step(&pool, "novel", "filename").await.is_ok());
        assert!(validate_slug_step(&pool, "novel", "content").await.is_ok());
        assert!(validate_slug_step(&pool, "comic", "filename").await.is_ok());
        assert!(validate_slug_step(&pool, "comic", "cover_pick").await.is_ok());
        assert!(
            validate_slug_step(&pool, "comic", "filename_folder")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn validate_slug_step_rejects_unknown() {
        let pool = seeded_pool().await;
        assert_eq!(
            validate_slug_step(&pool, "novel", "cover_pick")
                .await
                .unwrap_err(),
            "INVALID_PROMPT_SCHEMA_STEP"
        );
        assert_eq!(
            validate_slug_step(&pool, "manga", "filename")
                .await
                .unwrap_err(),
            "INVALID_PROMPT_SCHEMA_STEP"
        );
        // Retired with the category reclassify feature.
        assert_eq!(
            validate_slug_step(&pool, "novel", "category_reanalyze")
                .await
                .unwrap_err(),
            "INVALID_PROMPT_SCHEMA_STEP"
        );
    }

    #[test]
    fn legacy_mime_group_routes_filename_folder_to_image_folder() {
        assert_eq!(legacy_mime_group("novel", "filename"), "text");
        assert_eq!(legacy_mime_group("comic", "filename"), "archive");
        assert_eq!(legacy_mime_group("comic", "cover_pick"), "archive");
        assert_eq!(
            legacy_mime_group("comic", "filename_folder"),
            "image_folder"
        );
        // Custom schemas keep their own slug in the legacy column.
        assert_eq!(legacy_mime_group("podcast", "filename"), "podcast");
    }

    #[test]
    fn legacy_category_label_preserves_text_step_for_back_compat() {
        assert_eq!(legacy_category_label("novel", "filename"), "filename");
        assert_eq!(legacy_category_label("novel", "content"), "content");
        assert_eq!(legacy_category_label("comic", "filename"), "archive_filename");
        assert_eq!(
            legacy_category_label("comic", "filename_folder"),
            "image_folder_filename_folder"
        );
    }
}
