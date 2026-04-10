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
    pub category: Option<String>,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
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

#[tauri::command]
pub async fn prompt_set_default(
    app: tauri::AppHandle,
    id: i64,
) -> Result<Prompt, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let prompt_category: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT category FROM prompts WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some((cat,)) = prompt_category {
        match cat {
            Some(c) => {
                sqlx::query("UPDATE prompts SET is_default = 0 WHERE category = ? AND is_default = 1")
                    .bind(&c)
                    .execute(&pool)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            None => {
                sqlx::query("UPDATE prompts SET is_default = 0 WHERE category IS NULL AND is_default = 1")
                    .execute(&pool)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    sqlx::query(
        "UPDATE prompts SET is_default = 1, updated_at = datetime('now') WHERE id = ?",
    )
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
