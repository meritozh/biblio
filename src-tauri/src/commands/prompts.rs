use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct Prompt {
    pub id: i64,
    pub name: String,
    pub content: String,
    pub category: Option<String>,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PromptCreate {
    pub name: String,
    pub content: String,
    pub category: String,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

/// Validate that a prompt category is one of the two supported values.
fn validate_category(cat: &str) -> Result<(), String> {
    match cat {
        "filename" | "content" => Ok(()),
        _ => Err("INVALID_PROMPT_CATEGORY".to_string()),
    }
}

/// Fetch the content of the currently-active prompt for a given category.
/// Used by `llm.rs` to build preambles from DB-managed prompts.
pub async fn prompt_get_active(
    pool: &sqlx::SqlitePool,
    category: &str,
) -> Result<String, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT content FROM prompts WHERE category = ? AND is_default = 1 LIMIT 1",
    )
    .bind(category)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    row.map(|(c,)| c)
        .ok_or_else(|| format!("NO_ACTIVE_PROMPT: {}", category))
}

#[tauri::command]
pub async fn prompt_list(app: tauri::AppHandle, category: Option<String>) -> Result<Vec<Prompt>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let prompts: Vec<Prompt> = if let Some(cat) = category {
        sqlx::query_as(
            "SELECT id, name, content, category, is_default, created_at, updated_at FROM prompts WHERE category = ? ORDER BY created_at DESC",
        )
        .bind(&cat)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as(
            "SELECT id, name, content, category, is_default, created_at, updated_at FROM prompts ORDER BY created_at DESC",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?
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

    validate_category(&payload.category)?;

    sqlx::query(
        "INSERT INTO prompts (name, content, category, is_default) VALUES (?, ?, ?, 0)",
    )
    .bind(&payload.name)
    .bind(&payload.content)
    .bind(&payload.category)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let prompt: Prompt = sqlx::query_as(
        "SELECT id, name, content, category, is_default, created_at, updated_at FROM prompts WHERE id = last_insert_rowid()",
    )
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

    validate_category(&payload.category)?;

    sqlx::query(
        "UPDATE prompts SET name = ?, content = ?, category = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&payload.name)
    .bind(&payload.content)
    .bind(&payload.category)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let prompt: Prompt = sqlx::query_as(
        "SELECT id, name, content, category, is_default, created_at, updated_at FROM prompts WHERE id = ?",
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

/// Per-category default switching: clears the active flag on any sibling in
/// the same category, then marks `id` active. Testable without an `AppHandle`.
pub async fn set_default_impl(
    pool: &sqlx::SqlitePool,
    id: i64,
) -> Result<Prompt, String> {
    // Fetch the target's category so we know which siblings to clear.
    let target: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT category FROM prompts WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let target_category = target
        .and_then(|(c,)| c)
        .ok_or_else(|| "PROMPT_NOT_FOUND".to_string())?;
    validate_category(&target_category)?;

    // Clear the active flag only within this category.
    sqlx::query("UPDATE prompts SET is_default = 0 WHERE category = ? AND is_default = 1")
        .bind(&target_category)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Set this one active.
    sqlx::query(
        "UPDATE prompts SET is_default = 1, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query_as(
        "SELECT id, name, content, category, is_default, created_at, updated_at FROM prompts WHERE id = ?",
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
    fn validate_category_accepts_known_values() {
        assert!(validate_category("filename").is_ok());
        assert!(validate_category("content").is_ok());
    }

    #[test]
    fn validate_category_rejects_unknown() {
        let err = validate_category("whatever").unwrap_err();
        assert_eq!(err, "INVALID_PROMPT_CATEGORY");
    }

    #[tokio::test]
    async fn prompt_get_active_returns_active_content_for_category() {
        let pool = setup_db().await;
        // schema.sql seeds both categories with is_default = 1
        let filename_prompt = prompt_get_active(&pool, "filename").await.unwrap();
        assert!(filename_prompt.contains("display_name"));
        let content_prompt = prompt_get_active(&pool, "content").await.unwrap();
        assert!(content_prompt.contains("tags"));
    }

    #[tokio::test]
    async fn prompt_get_active_errors_when_no_active_for_category() {
        let pool = setup_db().await;
        // Clear all active flags
        sqlx::query("UPDATE prompts SET is_default = 0").execute(&pool).await.unwrap();
        let err = prompt_get_active(&pool, "content").await.unwrap_err();
        assert_eq!(err, "NO_ACTIVE_PROMPT: content");
    }

    #[tokio::test]
    async fn prompt_set_default_scopes_to_category() {
        let pool = setup_db().await;

        // Seeds give us: filename (id 1, active), content (id 2, active).
        // Add a second content prompt that is NOT active.
        sqlx::query(
            "INSERT INTO prompts (name, content, category, is_default) VALUES \
             ('Content Alt', 'alternate rules', 'content', 0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Call: switch the active content prompt to the new one (id 3).
        let new_active = set_default_impl(&pool, 3).await.unwrap();
        assert_eq!(new_active.id, 3);
        assert!(new_active.is_default);

        // Verify: id 2 (previously-active content) is no longer active.
        let (prev_active_flag,): (bool,) =
            sqlx::query_as("SELECT is_default FROM prompts WHERE id = 2")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!prev_active_flag, "previously-active content prompt should be cleared");

        // Verify: id 1 (filename) is still active (cross-category isolation).
        let (filename_active,): (bool,) =
            sqlx::query_as("SELECT is_default FROM prompts WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(filename_active, "filename active flag must not be touched when setting a content default");
    }
}
