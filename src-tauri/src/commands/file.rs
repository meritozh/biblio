use crate::commands::*;
use crate::commands::validation::validate_display_name;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

#[tauri::command]
pub async fn file_list(
    pool: State<'_, SqlitePool>,
    category_id: Option<i64>,
    _tag_ids: Option<Vec<i64>>,
    status: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<FileListResponse, String> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let mut query = String::from(
        "SELECT id, path, display_name, category_id, file_status, created_at, updated_at FROM files WHERE 1=1"
    );

    if let Some(cat_id) = category_id {
        query.push_str(&format!(" AND category_id = {}", cat_id));
    }

    if let Some(s) = &status {
        query.push_str(&format!(" AND file_status = '{}'", s));
    }

    query.push_str(&format!(" ORDER BY created_at DESC LIMIT {} OFFSET {}", limit, offset));

    let files: Vec<FileEntry> = sqlx::query_as(&query)
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files")
        .fetch_one(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(FileListResponse {
        files,
        total: total.0,
    })
}

#[derive(Serialize)]
pub struct FileListResponse {
    pub files: Vec<FileEntry>,
    pub total: i64,
}

#[tauri::command]
pub async fn file_get(
    pool: State<'_, SqlitePool>,
    id: i64,
) -> Result<FileWithDetails, String> {
    let file: FileEntry = sqlx::query_as(
        "SELECT id, path, display_name, category_id, file_status, created_at, updated_at FROM files WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&*pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("File not found")?;

    let category: Option<Category> = if let Some(cat_id) = file.category_id {
        sqlx::query_as("SELECT id, name, icon, is_default, created_at FROM categories WHERE id = ?")
            .bind(cat_id)
            .fetch_optional(&*pool)
            .await
            .map_err(|e| e.to_string())?
    } else {
        None
    };

    let tags: Vec<Tag> = sqlx::query_as(
        "SELECT t.id, t.name, t.color, t.created_at FROM tags t 
         INNER JOIN file_tags ft ON t.id = ft.tag_id WHERE ft.file_id = ?"
    )
    .bind(id)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    let metadata: Vec<Metadata> = sqlx::query_as(
        "SELECT id, file_id, key, value, data_type FROM metadata WHERE file_id = ?"
    )
    .bind(id)
    .fetch_all(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(FileWithDetails {
        id: file.id,
        path: file.path,
        display_name: file.display_name,
        category_id: file.category_id,
        file_status: file.file_status,
        created_at: file.created_at,
        updated_at: file.updated_at,
        category,
        tags,
        metadata,
    })
}

#[tauri::command]
pub async fn file_create(
    pool: State<'_, SqlitePool>,
    path: String,
    display_name: String,
    category_id: Option<i64>,
    tag_ids: Option<Vec<i64>>,
    metadata: Option<Vec<MetadataInput>>,
) -> Result<FileCreateResponse, String> {
    let validated_name = validate_display_name(&display_name)?;
    
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM files WHERE path = ?")
        .bind(&path)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("FILE_ALREADY_EXISTS".to_string());
    }

    let result = sqlx::query(
        "INSERT INTO files (path, display_name, category_id) VALUES (?, ?, ?)"
    )
    .bind(&path)
    .bind(&validated_name)
    .bind(category_id)
    .execute(&*pool)
    .await
    .map_err(|e| e.to_string())?;

    let file_id = result.last_insert_rowid();

    if let Some(tags) = tag_ids {
        for tag_id in tags {
            sqlx::query("INSERT INTO file_tags (file_id, tag_id) VALUES (?, ?)")
                .bind(file_id)
                .bind(tag_id)
                .execute(&*pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(meta) = metadata {
        for m in meta {
            sqlx::query("INSERT INTO metadata (file_id, key, value, data_type) VALUES (?, ?, ?, ?)")
                .bind(file_id)
                .bind(&m.key)
                .bind(&m.value)
                .bind(m.data_type.unwrap_or_else(|| "text".to_string()))
                .execute(&*pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(FileCreateResponse { id: file_id })
}

#[derive(Serialize)]
pub struct FileCreateResponse {
    pub id: i64,
}

#[tauri::command]
pub async fn file_update(
    pool: State<'_, SqlitePool>,
    id: i64,
    display_name: Option<String>,
    category_id: Option<Option<i64>>,
) -> Result<FileUpdateResponse, String> {
    if let Some(name) = display_name {
        let validated_name = validate_display_name(&name)?;
        sqlx::query("UPDATE files SET display_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(&validated_name)
            .bind(id)
            .execute(&*pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    if let Some(cat_id) = category_id {
        sqlx::query("UPDATE files SET category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(cat_id)
            .bind(id)
            .execute(&*pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(FileUpdateResponse { success: true })
}

#[derive(Serialize)]
pub struct FileUpdateResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn file_delete(
    pool: State<'_, SqlitePool>,
    id: i64,
) -> Result<FileDeleteResponse, String> {
    sqlx::query("DELETE FROM files WHERE id = ?")
        .bind(id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(FileDeleteResponse { success: true })
}

#[derive(Serialize)]
pub struct FileDeleteResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn file_search(
    pool: State<'_, SqlitePool>,
    query: String,
    category_id: Option<i64>,
    _tag_ids: Option<Vec<i64>>,
    _metadata_filters: Option<Vec<MetadataFilter>>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<FileListResponse, String> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let sql = if query.is_empty() {
        "SELECT id, path, display_name, category_id, file_status, created_at, updated_at FROM files WHERE 1=1"
    } else {
        "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.created_at, f.updated_at 
         FROM files f 
         JOIN files_fts ON files_fts.rowid = f.id 
         WHERE files_fts MATCH ?"
    };

    let mut final_query = sql.to_string();

    if let Some(cat_id) = category_id {
        final_query.push_str(&format!(" AND category_id = {}", cat_id));
    }

    final_query.push_str(&format!(" ORDER BY f.created_at DESC LIMIT {} OFFSET {}", limit, offset));

    let files: Vec<FileEntry> = if query.is_empty() {
        sqlx::query_as(&final_query)
            .fetch_all(&*pool)
            .await
            .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as(&final_query)
            .bind(&query)
            .fetch_all(&*pool)
            .await
            .map_err(|e| e.to_string())?
    };

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files")
        .fetch_one(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(FileListResponse {
        files,
        total: total.0,
    })
}

#[tauri::command]
pub async fn file_check_status(
    pool: State<'_, SqlitePool>,
    file_ids: Option<Vec<i64>>,
) -> Result<FileCheckStatusResponse, String> {
    let files: Vec<FileEntry> = match file_ids {
        Some(ids) => {
            let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "SELECT id, path, display_name, category_id, file_status, created_at, updated_at FROM files WHERE id IN ({})",
                placeholders
            );
            sqlx::query_as(&query)
                .fetch_all(&*pool)
                .await
                .map_err(|e| e.to_string())?
        }
        None => {
            sqlx::query_as("SELECT id, path, display_name, category_id, file_status, created_at, updated_at FROM files")
                .fetch_all(&*pool)
                .await
                .map_err(|e| e.to_string())?
        }
    };

    let mut updated = Vec::new();
    for file in files {
        let exists = std::path::Path::new(&file.path).exists();
        let new_status = if exists { "available" } else { "missing" };

        if file.file_status != new_status {
            sqlx::query("UPDATE files SET file_status = ? WHERE id = ?")
                .bind(new_status)
                .bind(file.id)
                .execute(&*pool)
                .await
                .map_err(|e| e.to_string())?;

            updated.push(FileStatusUpdate {
                id: file.id,
                status: new_status.to_string(),
            });
        }
    }

    Ok(FileCheckStatusResponse { updated })
}

#[derive(Serialize)]
pub struct FileStatusUpdate {
    pub id: i64,
    pub status: String,
}

#[derive(Serialize)]
pub struct FileCheckStatusResponse {
    pub updated: Vec<FileStatusUpdate>,
}