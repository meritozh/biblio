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

/// Build a clean filename from metadata: "<display_name> <progress> <authors>.ext"
pub fn build_novel_filename(
    display_name: &str,
    progress: Option<&str>,
    author_names: &[String],
    ext_with_dot: &str,
) -> String {
    let mut parts: Vec<String> = vec![display_name.to_string()];
    if let Some(p) = progress {
        if !p.is_empty() {
            parts.push(p.to_string());
        }
    }
    if !author_names.is_empty() {
        parts.push(author_names.join(", "));
    }
    format!("{}{}", parts.join(" "), ext_with_dot)
}

/// Remove filesystem-invalid characters from a filename
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect()
}

/// Copy a file to destination
/// Returns the final destination path
fn copy_file(source: &std::path::Path, dest: &std::path::Path) -> Result<PathBuf, String> {
    let final_dest = get_unique_destination(dest);

    fs::copy(source, &final_dest)
        .map_err(|e| {
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

    Ok(final_dest)
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

    // Build the WHERE clause once so both the row query and the count query
    // see the same filter — otherwise `total` misreports what's loadable,
    // which breaks pagination ("N remaining" stays non-zero forever).
    let mut where_clause = String::from(" WHERE 1=1");
    if let Some(cat_id) = category_id {
        where_clause.push_str(&format!(" AND category_id = {}", cat_id));
    }
    if let Some(s) = &status {
        where_clause.push_str(&format!(" AND file_status = '{}'", s));
    }

    let row_query = format!(
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, created_at, updated_at FROM files{} ORDER BY created_at DESC LIMIT {} OFFSET {}",
        where_clause, limit, offset
    );
    let files: Vec<FileEntry> = sqlx::query_as(&row_query)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let count_query = format!("SELECT COUNT(*) FROM files{}", where_clause);
    let total: (i64,) = sqlx::query_as(&count_query)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut file_items = Vec::with_capacity(files.len());
    for file in files {
        let tags: Vec<Tag> = sqlx::query_as(
            "SELECT t.id, t.name, t.color, t.created_at FROM tags t
             INNER JOIN file_tags ft ON t.id = ft.tag_id WHERE ft.file_id = ?"
        )
        .bind(file.id)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

        let authors: Vec<Author> = sqlx::query_as(
            "SELECT a.id, a.name, a.created_at FROM authors a
             INNER JOIN file_authors fa ON a.id = fa.author_id WHERE fa.file_id = ?"
        )
        .bind(file.id)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

        let description: Option<String> = sqlx::query_scalar(
            "SELECT value FROM metadata WHERE file_id = ? AND key = 'description'"
        )
        .bind(file.id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

        file_items.push(FileListItem {
            id: file.id,
            path: file.path,
            display_name: file.display_name,
            category_id: file.category_id,
            file_status: file.file_status,
            in_storage: file.in_storage,
            original_path: file.original_path,
            progress: file.progress,
            description,
            created_at: file.created_at,
            updated_at: file.updated_at,
            tags,
            authors,
        });
    }

    Ok(FileListResponse {
        files: file_items,
        total: total.0,
    })
}

#[derive(Serialize)]
pub struct FileListResponse {
    pub files: Vec<FileListItem>,
    pub total: i64,
}

/// Hydrate a list of `FileEntry` rows into `FileListItem`s by fetching their
/// associated tags and authors. Matches the per-file loop used inside `file_list`.
async fn hydrate_file_items(
    pool: &sqlx::SqlitePool,
    files: Vec<FileEntry>,
) -> Result<Vec<FileListItem>, String> {
    let mut items = Vec::with_capacity(files.len());
    for file in files {
        let tags: Vec<Tag> = sqlx::query_as(
            "SELECT t.id, t.name, t.color, t.created_at FROM tags t
             INNER JOIN file_tags ft ON t.id = ft.tag_id WHERE ft.file_id = ?",
        )
        .bind(file.id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let authors: Vec<Author> = sqlx::query_as(
            "SELECT a.id, a.name, a.created_at FROM authors a
             INNER JOIN file_authors fa ON a.id = fa.author_id WHERE fa.file_id = ?",
        )
        .bind(file.id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let description: Option<String> = sqlx::query_scalar(
            "SELECT value FROM metadata WHERE file_id = ? AND key = 'description'",
        )
        .bind(file.id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        items.push(FileListItem {
            id: file.id,
            path: file.path,
            display_name: file.display_name,
            category_id: file.category_id,
            file_status: file.file_status,
            in_storage: file.in_storage,
            original_path: file.original_path,
            progress: file.progress,
            description,
            created_at: file.created_at,
            updated_at: file.updated_at,
            tags,
            authors,
        });
    }
    Ok(items)
}

/// Core query for `file_list_by_tag` — testable without a Tauri `AppHandle`.
pub(crate) async fn list_files_by_tag_impl(
    pool: &sqlx::SqlitePool,
    tag_id: i64,
) -> Result<Vec<FileListItem>, String> {
    let files: Vec<FileEntry> = sqlx::query_as(
        "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status,
                f.in_storage, f.original_path, f.progress, f.created_at, f.updated_at
         FROM files f
         INNER JOIN file_tags ft ON ft.file_id = f.id
         WHERE ft.tag_id = ?
         ORDER BY f.created_at DESC",
    )
    .bind(tag_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    hydrate_file_items(pool, files).await
}

#[tauri::command]
pub async fn file_list_by_tag(
    app: AppHandle,
    tag_id: i64,
) -> Result<Vec<FileListItem>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    list_files_by_tag_impl(&pool, tag_id).await
}

/// Core query for `file_list_by_author` — testable without a Tauri `AppHandle`.
pub(crate) async fn list_files_by_author_impl(
    pool: &sqlx::SqlitePool,
    author_id: i64,
) -> Result<Vec<FileListItem>, String> {
    let files: Vec<FileEntry> = sqlx::query_as(
        "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status,
                f.in_storage, f.original_path, f.progress, f.created_at, f.updated_at
         FROM files f
         INNER JOIN file_authors fa ON fa.file_id = f.id
         WHERE fa.author_id = ?
         ORDER BY f.created_at DESC",
    )
    .bind(author_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    hydrate_file_items(pool, files).await
}

#[tauri::command]
pub async fn file_list_by_author(
    app: AppHandle,
    author_id: i64,
) -> Result<Vec<FileListItem>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    list_files_by_author_impl(&pool, author_id).await
}

#[tauri::command]
pub async fn file_get(
    app: AppHandle,
    id: i64,
) -> Result<FileWithDetails, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let file: FileEntry = sqlx::query_as(
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, created_at, updated_at FROM files WHERE id = ?"
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
        progress: file.progress,
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
    progress: Option<String>,
    cover_data: Option<Vec<u8>>,
    cover_mime_type: Option<String>,
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

    // Get source filename and extension
    let source_filename = source_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;
    let ext_lower = source_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    let should_clean_name = matches!(ext_lower.as_deref(), Some("txt") | Some("epub"));

    // Resolve author names up-front (needed for clean filename)
    let resolved_author_names: Vec<String> = match author_ids.as_ref() {
        Some(ids) if !ids.is_empty() => {
            let placeholders = (0..ids.len()).map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT name FROM authors WHERE id IN ({})",
                placeholders
            );
            let mut q = sqlx::query_scalar::<_, String>(&sql);
            for id in ids {
                q = q.bind(id);
            }
            q.fetch_all(&pool).await.map_err(|e| e.to_string())?
        }
        _ => Vec::new(),
    };

    // Compute destination filename
    let dest_filename = if should_clean_name {
        let ext_with_dot = ext_lower
            .as_ref()
            .map(|e| format!(".{}", e))
            .unwrap_or_default();
        let clean = build_novel_filename(
            &validated_name,
            progress.as_deref(),
            &resolved_author_names,
            &ext_with_dot,
        );
        sanitize_filename(&clean)
    } else {
        source_filename.to_string()
    };

    let dest_path = get_unique_destination(&dest_folder.join(&dest_filename));

    // Check import mode setting
    let import_mode: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'import_mode'"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let use_copy = import_mode
        .map(|(v,)| v == "copy")
        .unwrap_or(false);

    // Move or copy the file based on setting
    let final_path = if use_copy {
        copy_file(&source_canonical, &dest_path)?
    } else {
        move_file(&source_canonical, &dest_path)?
    };
    let final_path_str = final_path.to_string_lossy().to_string();

    // Insert into database with in_storage=true
    let result = sqlx::query(
        "INSERT INTO files (path, display_name, category_id, in_storage, original_path, file_status, progress) VALUES (?, ?, ?, 1, ?, 'available', ?)"
    )
    .bind(&final_path_str)
    .bind(&validated_name)
    .bind(category_id)
    .bind(&path)  // original_path is the source path
    .bind(&progress)
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

    if let Some(data) = cover_data {
        let mime = cover_mime_type.unwrap_or_else(|| "image/png".to_string());
        sqlx::query(
            "INSERT OR REPLACE INTO covers (file_id, data, mime_type) VALUES (?, ?, ?)"
        )
        .bind(file_id)
        .bind(&data)
        .bind(&mime)
        .execute(&pool)
        .await
        .map_err(|e| format!("Failed to save cover: {}", e))?;
    }

    Ok(FileCreateResponse { id: file_id })
}

/// Rename a file on disk to match current metadata, updating DB atomically.
/// Only renames .txt and .epub files. No-op for other extensions.
/// Uses a transaction: DB update first, then fs rename; rollback on rename failure.
pub async fn rename_file_to_match_metadata(
    pool: &sqlx::SqlitePool,
    file_id: i64,
) -> Result<(), String> {
    let file_info: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT path, display_name, progress FROM files WHERE id = ?"
    )
    .bind(file_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((current_path_str, display_name, progress)) = file_info else {
        return Ok(());
    };
    let current_path = PathBuf::from(&current_path_str);

    let ext_lower = current_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    // Only rename text files
    if !matches!(ext_lower.as_deref(), Some("txt") | Some("epub")) {
        return Ok(());
    }
    let ext_with_dot = ext_lower
        .as_ref()
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let author_rows: Vec<(String,)> = sqlx::query_as(
        "SELECT a.name FROM authors a JOIN file_authors fa ON a.id = fa.author_id WHERE fa.file_id = ?"
    )
    .bind(file_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let author_names_vec: Vec<String> = author_rows.into_iter().map(|(n,)| n).collect();

    let clean = build_novel_filename(
        &display_name,
        progress.as_deref(),
        &author_names_vec,
        &ext_with_dot,
    );
    let sanitized = sanitize_filename(&clean);

    let Some(parent) = current_path.parent() else {
        return Ok(());
    };

    let new_path = get_unique_destination(&parent.join(&sanitized));
    if new_path == current_path {
        return Ok(());
    }
    let new_path_str = new_path.to_string_lossy().to_string();

    // Transaction: update DB first, then rename file
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE files SET path = ? WHERE id = ?")
        .bind(&new_path_str)
        .bind(file_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    match fs::rename(&current_path, &new_path) {
        Ok(_) => {
            tx.commit().await.map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            let _ = tx.rollback().await;
            Err(format!("Failed to rename file: {}", e))
        }
    }
}

#[derive(Serialize)]
pub struct FileCreateResponse {
    pub id: i64,
}

/// Move a file to the folder of a different category when category_id changes.
/// Updates files.path in the DB to match. No-op when the file isn't in storage,
/// the category hasn't actually changed, or the file is already in the target
/// folder. Caller is responsible for updating files.category_id.
async fn move_file_to_category_folder(
    pool: &sqlx::SqlitePool,
    file_id: i64,
    new_category_id: Option<i64>,
) -> Result<(), String> {
    let file_info: Option<(String, bool, Option<i64>)> = sqlx::query_as(
        "SELECT path, in_storage, category_id FROM files WHERE id = ?",
    )
    .bind(file_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((current_path, in_storage, current_category)) = file_info else {
        return Ok(());
    };

    if !in_storage || current_category == new_category_id {
        return Ok(());
    }

    let storage_row: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let storage_path = match storage_row {
        Some((p,)) if !p.is_empty() => PathBuf::from(&p),
        _ => return Err("STORAGE_PATH_NOT_CONFIGURED".to_string()),
    };
    let storage_canonical = storage_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve storage path: {}", e))?;

    let folder_name = if let Some(cat_id) = new_category_id {
        let cat_row: Option<(Option<String>, String)> = sqlx::query_as(
            "SELECT folder_name, name FROM categories WHERE id = ?",
        )
        .bind(cat_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
        match cat_row {
            Some((Some(folder), _)) => folder,
            Some((None, name)) => sanitize_folder_name(&name),
            None => return Err("CATEGORY_NOT_FOUND".to_string()),
        }
    } else {
        UNCATEGORIZED_FOLDER.to_string()
    };

    let dest_folder = storage_canonical.join(&folder_name);
    if !dest_folder.exists() {
        fs::create_dir_all(&dest_folder)
            .map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    let current_pb = PathBuf::from(&current_path);
    if let Some(parent) = current_pb.parent() {
        if parent == dest_folder {
            return Ok(()); // already in correct folder
        }
    }

    let filename = current_pb
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;

    let dest_path = dest_folder.join(filename);
    let final_path = move_file(&current_pb, &dest_path)?;
    let final_path_str = final_path.to_string_lossy().to_string();

    sqlx::query("UPDATE files SET path = ? WHERE id = ?")
        .bind(&final_path_str)
        .bind(file_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn file_update(
    app: AppHandle,
    id: i64,
    display_name: Option<String>,
    category_id: Option<i64>,
    progress: Option<String>,
) -> Result<FileUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // If the category changed, physically move the file first so that the
    // subsequent rename_file_to_match_metadata operates on the new location.
    if let Some(new_cat) = category_id {
        move_file_to_category_folder(&pool, id, Some(new_cat)).await?;
    }

    match (display_name, category_id, progress) {
        (Some(name), Some(cat_id), Some(prog)) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, category_id = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(cat_id)
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (Some(name), Some(cat_id), None) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(cat_id)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (Some(name), None, Some(prog)) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (Some(name), None, None) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, Some(cat_id), Some(prog)) => {
            sqlx::query(
                "UPDATE files SET category_id = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(cat_id)
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, Some(cat_id), None) => {
            sqlx::query(
                "UPDATE files SET category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(cat_id)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, None, Some(prog)) => {
            sqlx::query(
                "UPDATE files SET progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, None, None) => {}
    }

    // Rename file on disk to match updated metadata (atomic with DB)
    let _ = rename_file_to_match_metadata(&pool, id).await;

    Ok(FileUpdateResponse { success: true })
}

#[derive(Serialize)]
pub struct FileUpdateResponse {
    pub success: bool,
}

/// Recursively enumerate every non-hidden file under `path`. Used by the
/// import flow's "Choose folder…" option to expand a directory into the
/// flat list of paths that `file_prepare_import` expects.
///
/// Hidden files/dirs (dotfiles) are skipped. Symlinks are followed by the
/// default `std::fs::read_dir` + `is_dir`/`is_file` calls — acceptable for
/// the common case of a user picking a media folder. Result is sorted so
/// repeated folder picks produce stable ordering.
#[tauri::command]
pub async fn list_files_in_folder(path: String) -> Result<Vec<String>, String> {
    let root = std::path::Path::new(&path);
    if !root.exists() {
        return Err("PATH_NOT_FOUND".to_string());
    }
    if !root.is_dir() {
        return Err("NOT_A_DIRECTORY".to_string());
    }

    fn walk(dir: &std::path::Path, out: &mut Vec<String>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Skip dotfiles / dotdirs (.DS_Store, .git, etc.)
            if name_str.starts_with('.') {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out)?;
            } else if path.is_file() {
                if let Some(s) = path.to_str() {
                    out.push(s.to_string());
                }
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    walk(root, &mut files).map_err(|e| format!("Failed to walk folder: {e}"))?;
    files.sort();
    Ok(files)
}

/// Delete a file at an arbitrary path on disk — used for the "Delete" choice
/// on the import duplicate dialog, where the file is NOT yet in the DB
/// (so `file_delete`, which keys off id, doesn't apply).
#[tauri::command]
pub async fn file_delete_source(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        // Nothing to remove — treat as success so the UI flow doesn't error.
        return Ok(());
    }
    fs::remove_file(p).map_err(|e| {
        let err_str = e.to_string().to_lowercase();
        if err_str.contains("permission denied") {
            "PERMISSION_DENIED".to_string()
        } else if err_str.contains("being used") || err_str.contains("locked") {
            "FILE_LOCKED".to_string()
        } else {
            format!("Failed to delete source file: {e}")
        }
    })
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

    // Verify storage path exists
    if !storage_path.exists() {
        return Err("STORAGE_PATH_NOT_FOUND".to_string());
    }

    // Canonicalize storage path
    let storage_canonical = storage_path.canonicalize()
        .map_err(|e| format!("Failed to resolve storage path: {}", e))?;

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
    let dest_folder = storage_canonical.join(&folder_name);
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

/// Translate a user-typed query into a safe FTS5 MATCH expression.
///
/// Strategy:
///   1. Replace every FTS5 operator character (`"'`:()-+*^`) with a space
///      so we never accidentally parse a user's punctuation as syntax.
///   2. Split on whitespace, trim empty tokens.
///   3. Append `*` to every token so FTS5 does a prefix match — lets a user
///      typing "三" find a file named "三体" without learning the syntax.
///
/// Returns None if the query has no usable tokens after sanitization (all
/// whitespace, all punctuation, etc.) — callers should treat that as
/// "no filter" rather than issuing an empty MATCH (which FTS5 rejects).
fn build_fts_query(raw: &str) -> Option<String> {
    let sanitized: String = raw
        .chars()
        .map(|c| match c {
            '"' | '\'' | ':' | '(' | ')' | '+' | '-' | '*' | '^' => ' ',
            c => c,
        })
        .collect();

    let terms: Vec<String> = sanitized
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| format!("{}*", t))
        .collect();

    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
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

    // If the user's query sanitizes to nothing, return an empty result set
    // with total = 0. The frontend routes empty queries to `file_list`, so
    // this path really only covers the all-punctuation / all-whitespace
    // edge case.
    let Some(fts_expr) = build_fts_query(&query) else {
        return Ok(FileListResponse { files: Vec::new(), total: 0 });
    };

    // Same pattern as file_list: build the WHERE once and share it between
    // the row query and the count query so `total` matches what's loadable.
    let mut where_tail = String::new();
    if let Some(cat_id) = category_id {
        where_tail.push_str(&format!(" AND f.category_id = {}", cat_id));
    }

    let row_query = format!(
        "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.progress, f.created_at, f.updated_at \
         FROM files f \
         JOIN files_fts ON files_fts.rowid = f.id \
         WHERE files_fts MATCH ?{} \
         ORDER BY f.created_at DESC LIMIT {} OFFSET {}",
        where_tail, limit, offset
    );
    let files: Vec<FileEntry> = sqlx::query_as(&row_query)
        .bind(&fts_expr)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let count_query = format!(
        "SELECT COUNT(*) FROM files f \
         JOIN files_fts ON files_fts.rowid = f.id \
         WHERE files_fts MATCH ?{}",
        where_tail
    );
    let total: (i64,) = sqlx::query_as(&count_query)
        .bind(&fts_expr)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let items = hydrate_file_items(&pool, files).await?;

    Ok(FileListResponse {
        files: items,
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
                "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, created_at, updated_at FROM files WHERE id IN ({})",
                placeholders
            );
            sqlx::query_as(&query)
                .fetch_all(&pool)
                .await
                .map_err(|e| e.to_string())?
        }
        None => {
            sqlx::query_as("SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, created_at, updated_at FROM files")
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

#[tauri::command]
pub async fn file_replace(
    app: AppHandle,
    existing_file_id: i64,
    path: String,
    display_name: String,
    category_id: Option<i64>,
    tag_ids: Option<Vec<i64>>,
    author_ids: Option<Vec<i64>>,
    metadata: Option<Vec<MetadataInput>>,
    progress: Option<String>,
    cover_data: Option<Vec<u8>>,
    cover_mime_type: Option<String>,
) -> Result<FileCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let existing: Option<(String, bool)> = sqlx::query_as(
        "SELECT path, in_storage FROM files WHERE id = ?",
    )
    .bind(existing_file_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some((existing_path, in_storage)) = existing {
        if in_storage {
            let old_path = PathBuf::from(&existing_path);
            if old_path.exists() {
                let _ = fs::remove_file(&old_path);
            }
        }

        sqlx::query("DELETE FROM files WHERE id = ?")
            .bind(existing_file_id)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to delete existing file: {e}"))?;
    }

    file_create(app, path, display_name, category_id, tag_ids, author_ids, metadata, progress, cover_data, cover_mime_type).await
}

#[cfg(test)]
mod filename_tests {
    use super::*;

    #[test]
    fn test_build_novel_filename_full() {
        let result = build_novel_filename(
            "三体",
            Some("完结"),
            &["刘慈欣".to_string()],
            ".txt",
        );
        assert_eq!(result, "三体 完结 刘慈欣.txt");
    }

    #[test]
    fn test_build_novel_filename_no_progress() {
        let result = build_novel_filename(
            "三体",
            None,
            &["刘慈欣".to_string()],
            ".txt",
        );
        assert_eq!(result, "三体 刘慈欣.txt");
    }

    #[test]
    fn test_build_novel_filename_no_authors() {
        let result = build_novel_filename(
            "三体",
            Some("完结"),
            &[],
            ".txt",
        );
        assert_eq!(result, "三体 完结.txt");
    }

    #[test]
    fn test_build_novel_filename_multiple_authors() {
        let result = build_novel_filename(
            "三体",
            None,
            &["A".to_string(), "B".to_string()],
            ".txt",
        );
        assert_eq!(result, "三体 A, B.txt");
    }

    #[test]
    fn test_build_novel_filename_empty_progress() {
        let result = build_novel_filename(
            "三体",
            Some(""),
            &["刘慈欣".to_string()],
            ".txt",
        );
        assert_eq!(result, "三体 刘慈欣.txt");
    }

    #[test]
    fn test_sanitize_filename_invalid_chars() {
        assert_eq!(
            sanitize_filename("a/b\\c:d*e?f\"g<h>i|j.txt"),
            "abcdefghij.txt"
        );
    }

    #[test]
    fn test_sanitize_filename_preserves_valid() {
        assert_eq!(
            sanitize_filename("三体 完结 刘慈欣.txt"),
            "三体 完结 刘慈欣.txt"
        );
    }

    #[test]
    fn test_build_fts_query_single_token_prefix_matched() {
        assert_eq!(build_fts_query("三体").as_deref(), Some("三体*"));
    }

    #[test]
    fn test_build_fts_query_multiple_tokens_joined_by_space() {
        assert_eq!(
            build_fts_query("三体 刘慈欣").as_deref(),
            Some("三体* 刘慈欣*")
        );
    }

    #[test]
    fn test_build_fts_query_strips_fts5_operators() {
        // Quotes, colon, parens, +, -, *, ^ become spaces so they can never
        // be parsed as FTS5 syntax.
        assert_eq!(
            build_fts_query("foo(bar):baz").as_deref(),
            Some("foo* bar* baz*")
        );
    }

    #[test]
    fn test_build_fts_query_empty_or_whitespace_returns_none() {
        assert!(build_fts_query("").is_none());
        assert!(build_fts_query("   ").is_none());
        assert!(build_fts_query("\"\"()").is_none());
    }
}

#[cfg(test)]
mod reverse_index_tests {
    use super::*;
    use crate::commands::test_helpers::setup_db;

    #[tokio::test]
    async fn setup_db_smoke_test() {
        let pool = setup_db().await;
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn list_files_by_tag_returns_joined_files_sorted_desc() {
        let pool = setup_db().await;

        // Seed 3 files with distinct created_at values (oldest → newest = A → B → C).
        sqlx::query(
            "INSERT INTO files (path, display_name, created_at) VALUES \
             ('/a.txt', 'File A', '2026-01-01 10:00:00'), \
             ('/b.txt', 'File B', '2026-01-02 10:00:00'), \
             ('/c.txt', 'File C', '2026-01-03 10:00:00')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO tags (name) VALUES ('sci-fi'), ('unused')")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("INSERT INTO authors (name) VALUES ('Liu Cixin')")
            .execute(&pool)
            .await
            .unwrap();

        // Tag 'sci-fi' (id 1) applied to File A (id 1) and File B (id 2), not File C.
        sqlx::query(
            "INSERT INTO file_tags (file_id, tag_id) VALUES (1, 1), (2, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO file_authors (file_id, author_id) VALUES (2, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = list_files_by_tag_impl(&pool, 1).await.unwrap();

        // Two files matched the tag.
        assert_eq!(result.len(), 2);
        // Sorted by created_at DESC: File B first (newer), File A second.
        assert_eq!(result[0].display_name, "File B");
        assert_eq!(result[1].display_name, "File A");
        // Tags hydrated on each row.
        assert_eq!(result[0].tags.len(), 1);
        assert_eq!(result[0].tags[0].name, "sci-fi");
        assert_eq!(result[1].tags[0].name, "sci-fi");
        // Authors hydrated (only File B has an author).
        assert_eq!(result[0].authors.len(), 1);
        assert_eq!(result[0].authors[0].name, "Liu Cixin");
        assert_eq!(result[1].authors.len(), 0);
    }

    #[tokio::test]
    async fn list_files_by_tag_returns_empty_when_tag_has_no_files() {
        let pool = setup_db().await;
        sqlx::query("INSERT INTO tags (name) VALUES ('unused')")
            .execute(&pool).await.unwrap();

        let result = list_files_by_tag_impl(&pool, 1).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_files_by_author_returns_joined_files_sorted_desc() {
        let pool = setup_db().await;

        sqlx::query(
            "INSERT INTO files (path, display_name, created_at) VALUES \
             ('/a.txt', 'File A', '2026-01-01 10:00:00'), \
             ('/b.txt', 'File B', '2026-01-02 10:00:00'), \
             ('/c.txt', 'File C', '2026-01-03 10:00:00')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO authors (name) VALUES ('Liu Cixin'), ('Unused')")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query("INSERT INTO tags (name) VALUES ('sci-fi')")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO file_authors (file_id, author_id) VALUES (1, 1), (3, 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO file_tags (file_id, tag_id) VALUES (3, 1)")
            .execute(&pool)
            .await
            .unwrap();

        let result = list_files_by_author_impl(&pool, 1).await.unwrap();

        assert_eq!(result.len(), 2);
        // Sorted by created_at DESC: File C (newer) first, File A second.
        assert_eq!(result[0].display_name, "File C");
        assert_eq!(result[1].display_name, "File A");
        // Authors hydrated.
        assert_eq!(result[0].authors.len(), 1);
        assert_eq!(result[0].authors[0].name, "Liu Cixin");
        // Tags hydrated (File C has the sci-fi tag, File A has none).
        assert_eq!(result[0].tags.len(), 1);
        assert_eq!(result[0].tags[0].name, "sci-fi");
        assert_eq!(result[1].tags.len(), 0);
    }

    #[tokio::test]
    async fn list_files_by_author_returns_empty_when_author_has_no_files() {
        let pool = setup_db().await;
        sqlx::query("INSERT INTO authors (name) VALUES ('Unused')")
            .execute(&pool).await.unwrap();

        let result = list_files_by_author_impl(&pool, 1).await.unwrap();
        assert!(result.is_empty());
    }
}