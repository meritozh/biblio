use crate::commands::validation::validate_author_name;
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
pub struct AuthorWithUsage {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub usage_count: i64,
}

#[tauri::command]
pub async fn author_list(
    app: AppHandle,
    include_usage: Option<bool>,
) -> Result<AuthorListResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let authors: Vec<AuthorWithUsage> = if include_usage.unwrap_or(false) {
        sqlx::query_as(
            "SELECT a.id, a.name, a.created_at, COUNT(fa.file_id) as usage_count
             FROM authors a
             LEFT JOIN file_authors fa ON a.id = fa.author_id
             GROUP BY a.id
             ORDER BY a.name"
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as(
            "SELECT a.id, a.name, a.created_at, 0 as usage_count FROM authors a ORDER BY a.name"
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?
    };

    Ok(AuthorListResponse { authors })
}

#[derive(Serialize)]
pub struct AuthorListResponse {
    pub authors: Vec<AuthorWithUsage>,
}

#[tauri::command]
pub async fn author_create(
    app: AppHandle,
    name: String,
) -> Result<AuthorCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let validated_name = validate_author_name(&name)?;

    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM authors WHERE name = ?")
        .bind(&validated_name)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("AUTHOR_EXISTS".to_string());
    }

    let result = sqlx::query("INSERT INTO authors (name) VALUES (?)")
        .bind(&validated_name)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(AuthorCreateResponse {
        id: result.last_insert_rowid(),
    })
}

#[derive(Serialize)]
pub struct AuthorCreateResponse {
    pub id: i64,
}

#[tauri::command]
pub async fn author_update(
    app: AppHandle,
    id: i64,
    name: String,
) -> Result<AuthorUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let validated_name = validate_author_name(&name)?;

    // Check if another author with the same name exists
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM authors WHERE name = ? AND id != ?")
        .bind(&validated_name)
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("AUTHOR_EXISTS".to_string());
    }

    sqlx::query("UPDATE authors SET name = ? WHERE id = ?")
        .bind(&validated_name)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(AuthorUpdateResponse { success: true })
}

#[derive(Serialize)]
pub struct AuthorUpdateResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn author_delete(
    app: AppHandle,
    id: i64,
) -> Result<AuthorDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let affected: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM file_authors WHERE author_id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM file_authors WHERE author_id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM authors WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(AuthorDeleteResponse {
        success: true,
        affected_files: affected.0,
    })
}

#[derive(Serialize)]
pub struct AuthorDeleteResponse {
    pub success: bool,
    pub affected_files: i64,
}

#[tauri::command]
pub async fn author_assign(
    app: AppHandle,
    file_id: i64,
    author_ids: Vec<i64>,
) -> Result<AuthorAssignResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    for author_id in &author_ids {
        sqlx::query("INSERT OR IGNORE INTO file_authors (file_id, author_id) VALUES (?, ?)")
            .bind(file_id)
            .bind(author_id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(AuthorAssignResponse { success: true })
}

#[derive(Serialize)]
pub struct AuthorAssignResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn author_unassign(
    app: AppHandle,
    file_id: i64,
    author_ids: Vec<i64>,
) -> Result<AuthorUnassignResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    for author_id in &author_ids {
        sqlx::query("DELETE FROM file_authors WHERE file_id = ? AND author_id = ?")
            .bind(file_id)
            .bind(author_id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(AuthorUnassignResponse { success: true })
}

#[derive(Serialize)]
pub struct AuthorUnassignResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn author_set(
    app: AppHandle,
    file_id: i64,
    author_ids: Vec<i64>,
) -> Result<AuthorSetResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // First remove all existing author assignments for this file
    sqlx::query("DELETE FROM file_authors WHERE file_id = ?")
        .bind(file_id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    // Then add the new author assignments
    for author_id in &author_ids {
        sqlx::query("INSERT INTO file_authors (file_id, author_id) VALUES (?, ?)")
            .bind(file_id)
            .bind(author_id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(AuthorSetResponse { success: true })
}

#[derive(Serialize)]
pub struct AuthorSetResponse {
    pub success: bool,
}