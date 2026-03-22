use crate::commands::*;
use crate::commands::validation::{validate_display_name, sanitize_folder_name};
use serde::Serialize;
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_sql::{DbPool, DbInstances};
use std::path::PathBuf;
use std::fs;

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

const UNCATEGORIZED_FOLDER: &str = "_uncategorized";

/// Generate a unique filename if file already exists
fn get_unique_destination(dest: &std::path::Path) -> PathBuf {
    if !dest.exists() {
        return dest.to_path_buf();
    }

    let parent = dest.parent().unwrap_or(std::path::Path::new("."));
    let stem = dest.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = dest.extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let mut counter = 1;
    loop {
        let new_name = if ext.is_empty() {
            format!("{} ({})", stem, counter)
        } else {
            format!("{} ({}){}", stem, counter, ext)
        };
        let new_path = parent.join(&new_name);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}

/// Move a file, handling cross-drive moves
/// Returns the final destination path
fn move_file(source: &std::path::Path, dest: &std::path::Path) -> Result<PathBuf, String> {
    let final_dest = get_unique_destination(dest);

    // Try rename first (fast, same filesystem)
    if fs::rename(source, &final_dest).is_ok() {
        return Ok(final_dest);
    }

    // Fall back to copy + delete (cross-drive)
    fs::copy(source, &final_dest)
        .map_err(|e| {
            // Map common filesystem errors to user-friendly codes
            let err_str = e.to_string().to_lowercase();
            if err_str.contains("permission denied") {
                "PERMISSION_DENIED".to_string()
            } else if err_str.contains("disk full") || err_str.contains("no space") {
                "DISK_FULL".to_string()
            } else if err_str.contains("being used") || err_str.contains("locked") {
                "FILE_LOCKED".to_string()
            } else {
                format!("Failed to copy file: {}", e)
            }
        })?;

    fs::remove_file(source)
        .map_err(|e| format!("Failed to remove original: {}", e))?;

    Ok(final_dest)
}

#[tauri::command]
pub async fn file_list(
    app: AppHandle,
    category_id: Option<i64>,
    _tag_ids: Option<Vec<i64>>,
    status: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<FileListResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let mut query = String::from(
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, created_at, updated_at FROM files WHERE 1=1"
    );

    if let Some(cat_id) = category_id {
        query.push_str(&format!(" AND category_id = {}", cat_id));
    }

    if let Some(s) = &status {
        query.push_str(&format!(" AND file_status = '{}'", s));
    }

    query.push_str(&format!(" ORDER BY created_at DESC LIMIT {} OFFSET {}", limit, offset));

    let files: Vec<FileEntry> = sqlx::query_as(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files")
        .fetch_one(&pool)
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
    app: AppHandle,
    id: i64,
) -> Result<FileWithDetails, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let file: FileEntry = sqlx::query_as(
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, created_at, updated_at FROM files WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("File not found")?;

    let category: Option<Category> = if let Some(cat_id) = file.category_id {
        sqlx::query_as("SELECT id, name, icon, is_default, folder_name, created_at FROM categories WHERE id = ?")
            .bind(cat_id)
            .fetch_optional(&pool)
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
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let authors: Vec<Author> = sqlx::query_as(
        "SELECT a.id, a.name, a.created_at FROM authors a
         INNER JOIN file_authors fa ON a.id = fa.author_id WHERE fa.file_id = ?"
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let metadata: Vec<Metadata> = sqlx::query_as(
        "SELECT id, file_id, key, value, data_type FROM metadata WHERE file_id = ?"
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(FileWithDetails {
        id: file.id,
        path: file.path,
        display_name: file.display_name,
        category_id: file.category_id,
        file_status: file.file_status,
        in_storage: file.in_storage,
        original_path: file.original_path,
        created_at: file.created_at,
        updated_at: file.updated_at,
        category,
        tags,
        authors,
        metadata,
    })
}

#[tauri::command]
pub async fn file_create(
    app: AppHandle,
    path: String,
    display_name: String,
    category_id: Option<i64>,
    tag_ids: Option<Vec<i64>>,
    author_ids: Option<Vec<i64>>,
    metadata: Option<Vec<MetadataInput>>,
) -> Result<FileCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let validated_name = validate_display_name(&display_name)?;

    // Check for existing file with same path
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM files WHERE path = ?")
        .bind(&path)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("FILE_ALREADY_EXISTS".to_string());
    }

    // Get storage_path from settings
    let storage_path_result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let storage_path = match storage_path_result {
        Some((p,)) if !p.is_empty() => PathBuf::from(&p),
        _ => return Err("STORAGE_PATH_NOT_CONFIGURED".to_string()),
    };

    // Verify storage path exists
    if !storage_path.exists() {
        return Err("STORAGE_PATH_NOT_FOUND".to_string());
    }

    // Check source file exists
    let source_path = PathBuf::from(&path);
    if !source_path.exists() {
        return Err("SOURCE_FILE_NOT_FOUND".to_string());
    }

    // Canonicalize paths for comparison
    let source_canonical = source_path.canonicalize()
        .map_err(|e| format!("Failed to resolve source path: {}", e))?;
    let storage_canonical = storage_path.canonicalize()
        .map_err(|e| format!("Failed to resolve storage path: {}", e))?;

    // Check source is not inside storage_path
    if source_canonical.starts_with(&storage_canonical) {
        return Err("SOURCE_ALREADY_IN_STORAGE".to_string());
    }

    // Determine destination folder
    let folder_name = if let Some(cat_id) = category_id {
        // Get category's folder_name, computing if NULL
        let cat_result: Option<(Option<String>, String)> = sqlx::query_as(
            "SELECT folder_name, name FROM categories WHERE id = ?"
        )
        .bind(cat_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

        match cat_result {
            Some((Some(folder), _)) => folder,
            Some((None, name)) => sanitize_folder_name(&name),
            None => return Err("CATEGORY_NOT_FOUND".to_string()),
        }
    } else {
        UNCATEGORIZED_FOLDER.to_string()
    };

    // Create destination folder if needed
    let dest_folder = storage_canonical.join(&folder_name);
    if !dest_folder.exists() {
        fs::create_dir_all(&dest_folder)
            .map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    // Get filename and build destination path
    let filename = source_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;
    let dest_path = dest_folder.join(filename);

    // Move the file
    let final_path = move_file(&source_canonical, &dest_path)?;
    let final_path_str = final_path.to_string_lossy().to_string();

    // Insert into database with in_storage=true
    let result = sqlx::query(
        "INSERT INTO files (path, display_name, category_id, in_storage, original_path, file_status) VALUES (?, ?, ?, 1, ?, 'available')"
    )
    .bind(&final_path_str)
    .bind(&validated_name)
    .bind(category_id)
    .bind(&path)  // original_path is the source path
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let file_id = result.last_insert_rowid();

    // Insert tags
    if let Some(tags) = tag_ids {
        for tag_id in tags {
            sqlx::query("INSERT INTO file_tags (file_id, tag_id) VALUES (?, ?)")
                .bind(file_id)
                .bind(tag_id)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    // Insert authors
    if let Some(authors) = author_ids {
        for author_id in authors {
            sqlx::query("INSERT INTO file_authors (file_id, author_id) VALUES (?, ?)")
                .bind(file_id)
                .bind(author_id)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    // Insert metadata
    if let Some(meta) = metadata {
        for m in meta {
            sqlx::query("INSERT INTO metadata (file_id, key, value, data_type) VALUES (?, ?, ?, ?)")
                .bind(file_id)
                .bind(&m.key)
                .bind(&m.value)
                .bind(m.data_type.unwrap_or_else(|| "text".to_string()))
                .execute(&pool)
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
    app: AppHandle,
    id: i64,
    display_name: Option<String>,
) -> Result<FileUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    if let Some(name) = display_name {
        let validated_name = validate_display_name(&name)?;
        sqlx::query("UPDATE files SET display_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(&validated_name)
            .bind(id)
            .execute(&pool)
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
    app: AppHandle,
    id: i64,
) -> Result<FileDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Get file info before deleting
    let file_info: Option<(String, bool)> = sqlx::query_as(
        "SELECT path, in_storage FROM files WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some((path, in_storage)) = file_info {
        // If file is in storage, delete from filesystem
        if in_storage {
            let _ = fs::remove_file(&path); // Ignore errors if file doesn't exist
        }

        // Delete from database
        sqlx::query("DELETE FROM files WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(FileDeleteResponse { success: true })
}

#[derive(Serialize)]
pub struct FileDeleteResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn file_move_category(
    app: AppHandle,
    id: i64,
    category_id: Option<i64>,
) -> Result<FileMoveCategoryResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Get current file info
    let file_info: Option<(String, bool, Option<i64>)> = sqlx::query_as(
        "SELECT path, in_storage, category_id FROM files WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let (current_path, in_storage, _current_category) = file_info
        .ok_or("FILE_NOT_FOUND".to_string())?;

    // Verify file is in storage
    if !in_storage {
        return Err("FILE_NOT_IN_STORAGE".to_string());
    }

    // Get storage_path
    let storage_path_result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let storage_path = match storage_path_result {
        Some((p,)) if !p.is_empty() => PathBuf::from(&p),
        _ => return Err("STORAGE_PATH_NOT_CONFIGURED".to_string()),
    };

    // Determine new folder
    let folder_name = if let Some(cat_id) = category_id {
        let cat_result: Option<(Option<String>, String)> = sqlx::query_as(
            "SELECT folder_name, name FROM categories WHERE id = ?"
        )
        .bind(cat_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

        match cat_result {
            Some((Some(folder), _)) => folder,
            Some((None, name)) => sanitize_folder_name(&name),
            None => return Err("CATEGORY_NOT_FOUND".to_string()),
        }
    } else {
        UNCATEGORIZED_FOLDER.to_string()
    };

    // Create destination folder if needed
    let dest_folder = storage_path.join(&folder_name);
    if !dest_folder.exists() {
        fs::create_dir_all(&dest_folder)
            .map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    // Get current file path and filename
    let current_path_buf = PathBuf::from(&current_path);
    let filename = current_path_buf.file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;

    // Check if already in the correct folder
    if let Some(parent) = current_path_buf.parent()
        && parent == dest_folder
    {
        // Already in correct folder, just update database
        sqlx::query("UPDATE files SET category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(category_id)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;

        return Ok(FileMoveCategoryResponse { success: true, new_path: current_path });
    }

    // Move the file
    let dest_path = dest_folder.join(filename);
    let final_path = move_file(&current_path_buf, &dest_path)?;
    let final_path_str = final_path.to_string_lossy().to_string();

    // Update database
    sqlx::query("UPDATE files SET path = ?, category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(&final_path_str)
        .bind(category_id)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(FileMoveCategoryResponse { success: true, new_path: final_path_str })
}

#[derive(Serialize)]
pub struct FileMoveCategoryResponse {
    pub success: bool,
    pub new_path: String,
}

#[tauri::command]
pub async fn file_search(
    app: AppHandle,
    query: String,
    category_id: Option<i64>,
    _tag_ids: Option<Vec<i64>>,
    _metadata_filters: Option<Vec<MetadataFilter>>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<FileListResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let sql = if query.is_empty() {
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, created_at, updated_at FROM files WHERE 1=1"
    } else {
        "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.created_at, f.updated_at
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
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as(&final_query)
            .bind(&query)
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?
    };

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files")
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(FileListResponse {
        files,
        total: total.0,
    })
}

#[tauri::command]
pub async fn file_check_status(
    app: AppHandle,
    file_ids: Option<Vec<i64>>,
) -> Result<FileCheckStatusResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let files: Vec<FileEntry> = match file_ids {
        Some(ids) => {
            let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, created_at, updated_at FROM files WHERE id IN ({})",
                placeholders
            );
            sqlx::query_as(&query)
                .fetch_all(&pool)
                .await
                .map_err(|e| e.to_string())?
        }
        None => {
            sqlx::query_as("SELECT id, path, display_name, category_id, file_status, in_storage, original_path, created_at, updated_at FROM files")
                .fetch_all(&pool)
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
                .execute(&pool)
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