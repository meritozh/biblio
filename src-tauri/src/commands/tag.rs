use crate::commands::validation::{validate_tag_name, validate_color};
use serde::Serialize;
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_sql::{DbPool, DbInstances};

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TagWithUsage {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
    pub usage_count: i64,
}

#[tauri::command]
pub async fn tag_list(
    app: AppHandle,
    include_usage: Option<bool>,
) -> Result<TagListResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let tags: Vec<TagWithUsage> = if include_usage.unwrap_or(false) {
        sqlx::query_as(
            "SELECT t.id, t.name, t.color, t.created_at, COUNT(ft.file_id) as usage_count
             FROM tags t
             LEFT JOIN file_tags ft ON t.id = ft.tag_id
             GROUP BY t.id
             ORDER BY t.name"
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as(
            "SELECT t.id, t.name, t.color, t.created_at, 0 as usage_count FROM tags t ORDER BY t.name"
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?
    };

    Ok(TagListResponse { tags })
}

#[derive(Serialize)]
pub struct TagListResponse {
    pub tags: Vec<TagWithUsage>,
}

#[tauri::command]
pub async fn tag_create(
    app: AppHandle,
    name: String,
    color: Option<String>,
) -> Result<TagCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let validated_name = validate_tag_name(&name)?;

    let validated_color = if let Some(c) = color {
        Some(validate_color(&c)?)
    } else {
        None
    };

    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM tags WHERE name = ?")
        .bind(&validated_name)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("TAG_EXISTS".to_string());
    }

    let result = sqlx::query("INSERT INTO tags (name, color) VALUES (?, ?)")
        .bind(&validated_name)
        .bind(&validated_color)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(TagCreateResponse {
        id: result.last_insert_rowid(),
    })
}

#[derive(Serialize)]
pub struct TagCreateResponse {
    pub id: i64,
}

#[tauri::command]
pub async fn tag_update(
    app: AppHandle,
    id: i64,
    name: Option<String>,
    color: Option<String>,
) -> Result<TagUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    if let Some(n) = name {
        let validated_name = validate_tag_name(&n)?;
        sqlx::query("UPDATE tags SET name = ? WHERE id = ?")
            .bind(&validated_name)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    if let Some(c) = color {
        let validated_color = validate_color(&c)?;
        sqlx::query("UPDATE tags SET color = ? WHERE id = ?")
            .bind(&validated_color)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(TagUpdateResponse { success: true })
}

#[derive(Serialize)]
pub struct TagUpdateResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn tag_delete(
    app: AppHandle,
    id: i64,
) -> Result<TagDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let affected: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM file_tags WHERE tag_id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM file_tags WHERE tag_id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM tags WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(TagDeleteResponse {
        success: true,
        affected_files: affected.0,
    })
}

#[derive(Serialize)]
pub struct TagDeleteResponse {
    pub success: bool,
    pub affected_files: i64,
}

#[tauri::command]
pub async fn tag_assign(
    app: AppHandle,
    file_id: i64,
    tag_ids: Vec<i64>,
) -> Result<TagAssignResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    for tag_id in tag_ids {
        sqlx::query("INSERT OR IGNORE INTO file_tags (file_id, tag_id) VALUES (?, ?)")
            .bind(file_id)
            .bind(tag_id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(TagAssignResponse { success: true })
}

#[derive(Serialize)]
pub struct TagAssignResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn tag_unassign(
    app: AppHandle,
    file_id: i64,
    tag_ids: Vec<i64>,
) -> Result<TagUnassignResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    for tag_id in tag_ids {
        sqlx::query("DELETE FROM file_tags WHERE file_id = ? AND tag_id = ?")
            .bind(file_id)
            .bind(tag_id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(TagUnassignResponse { success: true })
}

#[derive(Serialize)]
pub struct TagUnassignResponse {
    pub success: bool,
}