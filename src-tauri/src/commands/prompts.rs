use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Prompt {
    pub id: i64,
    pub name: String,
    pub content: String,
    /// Legacy free-text label kept for backward compatibility with existing
    /// rows. The active discriminator is `(mime_group, step)`.
    pub category: Option<String>,
    /// Mime-type group the prompt applies to. Currently `'text'` (novels:
    /// .txt) or `'archive'` (comics: .zip / .cbz / .rar / .cbr).
    pub mime_group: String,
    /// Pipeline step the prompt feeds. `'filename'` and `'content'` for
    /// text; `'filename'` and `'cover_pick'` for archives.
    pub step: String,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptCreate {
    pub name: String,
    pub content: String,
    pub mime_group: String,
    pub step: String,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

/// Validate that a (mime_group, step) pair is one we know about.
/// Pairs: `('text', 'filename')`, `('text', 'content')`,
/// `('archive', 'filename')`, `('archive', 'cover_pick')`,
/// `('image_folder', 'filename')`. The image_folder group reuses
/// `(archive, cover_pick)` for cover detection (filename-pattern
/// reasoning is identical), so no `(image_folder, cover_pick)` pair.
fn validate_group_step(mime_group: &str, step: &str) -> Result<(), String> {
    match (mime_group, step) {
        ("text", "filename")
        | ("text", "content")
        | ("archive", "filename")
        | ("archive", "cover_pick")
        | ("image_folder", "filename") => Ok(()),
        _ => Err("INVALID_PROMPT_GROUP_STEP".to_string()),
    }
}

/// Fetch the content of the currently-active prompt for a given
/// (mime_group, step) pair. Used by `llm.rs` to build preambles.
pub async fn prompt_get_active(
    pool: &sqlx::SqlitePool,
    mime_group: &str,
    step: &str,
) -> Result<String, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM prompts WHERE mime_group = ? AND step = ? AND is_default = 1 LIMIT 1",
    )
    .bind(mime_group)
    .bind(step)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    row.map(|(c,)| c)
        .ok_or_else(|| format!("NO_ACTIVE_PROMPT: {}/{}", mime_group, step))
}

#[tauri::command]
pub async fn prompt_list(
    app: tauri::AppHandle,
    mime_group: Option<String>,
    step: Option<String>,
) -> Result<Vec<Prompt>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let prompts: Vec<Prompt> = match (mime_group.as_deref(), step.as_deref()) {
        (Some(mg), Some(s)) => sqlx::query_as(
            "SELECT id, name, content, category, mime_group, step, is_default, created_at, updated_at FROM prompts WHERE mime_group = ? AND step = ? ORDER BY created_at DESC",
        )
        .bind(mg)
        .bind(s)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
        (Some(mg), None) => sqlx::query_as(
            "SELECT id, name, content, category, mime_group, step, is_default, created_at, updated_at FROM prompts WHERE mime_group = ? ORDER BY step, created_at DESC",
        )
        .bind(mg)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
        _ => sqlx::query_as(
            "SELECT id, name, content, category, mime_group, step, is_default, created_at, updated_at FROM prompts ORDER BY mime_group, step, created_at DESC",
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

    validate_group_step(&payload.mime_group, &payload.step)?;

    // Keep `category` populated with `<group>_<step>` (or just `step` for
    // text/text-step pairs) so legacy queries that still read it return
    // a sensible value.
    let legacy_category = legacy_category_label(&payload.mime_group, &payload.step);

    sqlx::query(
        "INSERT INTO prompts (name, content, category, mime_group, step, is_default) VALUES (?, ?, ?, ?, ?, 0)",
    )
    .bind(&payload.name)
    .bind(&payload.content)
    .bind(&legacy_category)
    .bind(&payload.mime_group)
    .bind(&payload.step)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let prompt: Prompt = sqlx::query_as(
        "SELECT id, name, content, category, mime_group, step, is_default, created_at, updated_at FROM prompts WHERE id = last_insert_rowid()",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(prompt)
}

fn legacy_category_label(mime_group: &str, step: &str) -> String {
    match (mime_group, step) {
        ("text", s) => s.to_string(),
        (g, s) => format!("{g}_{s}"),
    }
}

#[tauri::command]
pub async fn prompt_update(
    app: tauri::AppHandle,
    id: i64,
    payload: PromptCreate,
) -> Result<Prompt, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    validate_group_step(&payload.mime_group, &payload.step)?;
    let legacy_category = legacy_category_label(&payload.mime_group, &payload.step);

    sqlx::query(
        "UPDATE prompts SET name = ?, content = ?, category = ?, mime_group = ?, step = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&payload.name)
    .bind(&payload.content)
    .bind(&legacy_category)
    .bind(&payload.mime_group)
    .bind(&payload.step)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let prompt: Prompt = sqlx::query_as(
        "SELECT id, name, content, category, mime_group, step, is_default, created_at, updated_at FROM prompts WHERE id = ?",
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

/// Per-(mime_group, step) default switching: clears the active flag on any
/// sibling in the same group + step, then marks `id` active. Testable
/// without an `AppHandle`.
pub async fn set_default_impl(
    pool: &sqlx::SqlitePool,
    id: i64,
) -> Result<Prompt, String> {
    let target: Option<(String, String)> = sqlx::query_as(
        "SELECT mime_group, step FROM prompts WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let (mime_group, step) = target.ok_or_else(|| "PROMPT_NOT_FOUND".to_string())?;
    validate_group_step(&mime_group, &step)?;

    sqlx::query(
        "UPDATE prompts SET is_default = 0 WHERE mime_group = ? AND step = ? AND is_default = 1",
    )
    .bind(&mime_group)
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
        "SELECT id, name, content, category, mime_group, step, is_default, created_at, updated_at FROM prompts WHERE id = ?",
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
    use crate::commands::test_helpers::setup_db;

    #[test]
    fn validate_group_step_accepts_known_pairs() {
        assert!(validate_group_step("text", "filename").is_ok());
        assert!(validate_group_step("text", "content").is_ok());
        assert!(validate_group_step("archive", "filename").is_ok());
        assert!(validate_group_step("archive", "cover_pick").is_ok());
        assert!(validate_group_step("image_folder", "filename").is_ok());
    }

    #[test]
    fn validate_group_step_rejects_unknown() {
        assert_eq!(
            validate_group_step("text", "cover_pick").unwrap_err(),
            "INVALID_PROMPT_GROUP_STEP"
        );
        assert_eq!(
            validate_group_step("video", "filename").unwrap_err(),
            "INVALID_PROMPT_GROUP_STEP"
        );
    }

    #[test]
    fn legacy_category_label_preserves_text_step_for_back_compat() {
        // Pre-v3 callers stored 'filename' / 'content' in `category`; the
        // text path still emits those exact tokens so any external
        // consumer that read the column keeps working.
        assert_eq!(legacy_category_label("text", "filename"), "filename");
        assert_eq!(legacy_category_label("text", "content"), "content");
        assert_eq!(legacy_category_label("archive", "filename"), "archive_filename");
        assert_eq!(legacy_category_label("archive", "cover_pick"), "archive_cover_pick");
    }

    #[tokio::test]
    async fn prompt_get_active_returns_text_seeds() {
        // Schema.sql is v1 only; v3 (which adds the columns + comic
        // seeds) hasn't run in the test pool. Verify that the legacy
        // text seeds are still resolvable via the back-compat fallback.
        let pool = setup_db().await;
        sqlx::query("ALTER TABLE prompts ADD COLUMN mime_group TEXT NOT NULL DEFAULT 'text'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE prompts ADD COLUMN step TEXT NOT NULL DEFAULT 'filename'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE prompts SET step = category WHERE category IN ('filename', 'content')")
            .execute(&pool)
            .await
            .unwrap();

        let filename_prompt = prompt_get_active(&pool, "text", "filename").await.unwrap();
        assert!(filename_prompt.contains("display_name"));
        let content_prompt = prompt_get_active(&pool, "text", "content").await.unwrap();
        assert!(content_prompt.contains("tags"));
    }

    #[tokio::test]
    async fn prompt_get_active_errors_when_no_active() {
        let pool = setup_db().await;
        sqlx::query("ALTER TABLE prompts ADD COLUMN mime_group TEXT NOT NULL DEFAULT 'text'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE prompts ADD COLUMN step TEXT NOT NULL DEFAULT 'filename'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE prompts SET step = category WHERE category IN ('filename', 'content')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE prompts SET is_default = 0").execute(&pool).await.unwrap();

        let err = prompt_get_active(&pool, "text", "content").await.unwrap_err();
        assert_eq!(err, "NO_ACTIVE_PROMPT: text/content");
    }

    #[tokio::test]
    async fn prompt_set_default_scopes_to_group_and_step() {
        let pool = setup_db().await;
        sqlx::query("ALTER TABLE prompts ADD COLUMN mime_group TEXT NOT NULL DEFAULT 'text'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("ALTER TABLE prompts ADD COLUMN step TEXT NOT NULL DEFAULT 'filename'")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE prompts SET step = category WHERE category IN ('filename', 'content')")
            .execute(&pool)
            .await
            .unwrap();

        // Add a second text/content prompt (not active).
        sqlx::query(
            "INSERT INTO prompts (name, content, category, mime_group, step, is_default) VALUES \
             ('Content Alt', 'alternate rules', 'content', 'text', 'content', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let new_active = set_default_impl(&pool, 3).await.unwrap();
        assert_eq!(new_active.id, 3);
        assert!(new_active.is_default);

        // Previously-active text/content (id 2) is cleared.
        let (prev_flag,): (bool,) =
            sqlx::query_as("SELECT is_default FROM prompts WHERE id = 2")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!prev_flag);

        // text/filename (id 1) is untouched — different (group, step) bucket.
        let (filename_flag,): (bool,) =
            sqlx::query_as("SELECT is_default FROM prompts WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(filename_flag);
    }
}
