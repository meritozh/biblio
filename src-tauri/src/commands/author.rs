use crate::commands::validation::validate_author_name;
use serde::Serialize;
use tauri::AppHandle;
use tauri::{Emitter, Manager};
use tauri_plugin_sql::{DbPool, DbInstances};

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

#[derive(serde::Serialize, Clone)]
struct AuthorChangeEvent {
    id: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AuthorWithUsage {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    /// Wire-format alias: TS reads `usageCount`. See `TagWithUsage` for
    /// the same pattern; we avoid `rename_all = "camelCase"` so other
    /// snake_case fields (created_at) stay aligned with the base
    /// `Author` type.
    #[serde(rename = "usageCount")]
    pub usage_count: i64,
}

#[tauri::command]
pub async fn author_list(
    app: AppHandle,
    include_usage: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
    name_query: Option<String>,
) -> Result<AuthorListResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // `LIMIT -1 OFFSET 0` returns every row in SQLite — lets the unpaginated
    // callers (edit dialog's author picker etc.) reuse the same SQL.
    let limit_val: i64 = limit.unwrap_or(-1);
    let offset_val: i64 = offset.unwrap_or(0);
    let like_pattern = name_query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s.to_lowercase()));

    let authors: Vec<AuthorWithUsage> = match (include_usage.unwrap_or(false), like_pattern.as_deref()) {
        (true, Some(pat)) => sqlx::query_as(
            "SELECT a.id, a.name, a.created_at, COUNT(fa.file_id) as usage_count
             FROM authors a
             LEFT JOIN file_authors fa ON a.id = fa.author_id
             WHERE LOWER(a.name) LIKE ?
             GROUP BY a.id
             ORDER BY a.name
             LIMIT ? OFFSET ?",
        )
        .bind(pat)
        .bind(limit_val)
        .bind(offset_val)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
        (true, None) => sqlx::query_as(
            "SELECT a.id, a.name, a.created_at, COUNT(fa.file_id) as usage_count
             FROM authors a
             LEFT JOIN file_authors fa ON a.id = fa.author_id
             GROUP BY a.id
             ORDER BY a.name
             LIMIT ? OFFSET ?",
        )
        .bind(limit_val)
        .bind(offset_val)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
        (false, Some(pat)) => sqlx::query_as(
            "SELECT a.id, a.name, a.created_at, 0 as usage_count
             FROM authors a
             WHERE LOWER(a.name) LIKE ?
             ORDER BY a.name
             LIMIT ? OFFSET ?",
        )
        .bind(pat)
        .bind(limit_val)
        .bind(offset_val)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
        (false, None) => sqlx::query_as(
            "SELECT a.id, a.name, a.created_at, 0 as usage_count
             FROM authors a
             ORDER BY a.name
             LIMIT ? OFFSET ?",
        )
        .bind(limit_val)
        .bind(offset_val)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
    };

    Ok(AuthorListResponse { authors })
}

#[derive(Serialize)]
pub struct AuthorListResponse {
    pub authors: Vec<AuthorWithUsage>,
}

#[tauri::command]
pub async fn author_count(app: AppHandle, name_query: Option<String>) -> Result<i64, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let like_pattern = name_query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s.to_lowercase()));
    let (total,): (i64,) = if let Some(pat) = like_pattern {
        sqlx::query_as("SELECT COUNT(*) FROM authors WHERE LOWER(name) LIKE ?")
            .bind(pat)
            .fetch_one(&pool)
            .await
            .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM authors")
            .fetch_one(&pool)
            .await
            .map_err(|e| e.to_string())?
    };
    Ok(total)
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

    let _ = app.emit("author-updated", AuthorChangeEvent { id });

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

    let _ = app.emit("author-deleted", AuthorChangeEvent { id });

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

/// Bulk-delete authors with no `file_authors` row referencing them. Used
/// by the `/cleanup` page. Emits one `author-deleted` event with `id: 0`
/// as a bulk sentinel; see `tag_delete_unused` for the rationale.
#[tauri::command]
pub async fn author_delete_unused(
    app: AppHandle,
) -> Result<AuthorDeleteUnusedResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let result = sqlx::query(
        "DELETE FROM authors WHERE NOT EXISTS (SELECT 1 FROM file_authors WHERE author_id = authors.id)",
    )
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let deleted = result.rows_affected() as i64;
    if deleted > 0 {
        let _ = app.emit("author-deleted", AuthorChangeEvent { id: 0 });
    }

    Ok(AuthorDeleteUnusedResponse { deleted })
}

#[derive(Serialize)]
pub struct AuthorDeleteUnusedResponse {
    pub deleted: i64,
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

    let _ = super::file::rename_file_to_match_metadata(&pool, file_id).await;

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

    let _ = super::file::rename_file_to_match_metadata(&pool, file_id).await;

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

    let _ = super::file::rename_file_to_match_metadata(&pool, file_id).await;

    Ok(AuthorSetResponse { success: true })
}

#[derive(Serialize)]
pub struct AuthorSetResponse {
    pub success: bool,
}