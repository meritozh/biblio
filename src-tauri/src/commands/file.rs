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

/// Write every image file under `source_dir` (recursively, sorted) into
/// a `.zip` at `dest`. Stored (no compression) — comic images are already
/// JPEG/PNG/WebP, so deflate burns CPU for ~0% gain. Returns the final
/// destination path (after any unique-name disambiguation). Hidden files
/// and non-image files are skipped, matching the importer's collapse
/// rule.
fn zip_image_dir(source_dir: &std::path::Path, dest: &std::path::Path) -> Result<PathBuf, String> {
    use crate::pipeline::archive::is_image_filename;
    use std::io::Write;

    let final_dest = get_unique_destination(dest);
    let f = std::fs::File::create(&final_dest)
        .map_err(|e| format!("Failed to create zip: {e}"))?;
    let mut zw = zip::ZipWriter::new(f);
    let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    fn walk(
        root: &std::path::Path,
        dir: &std::path::Path,
        zw: &mut zip::ZipWriter<std::fs::File>,
        opts: &zip::write::SimpleFileOptions,
    ) -> Result<(), String> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read {}: {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                walk(root, &p, zw, opts)?;
            } else if p.is_file() && is_image_filename(&name_str) {
                let rel = p
                    .strip_prefix(root)
                    .map_err(|e| format!("strip_prefix: {e}"))?;
                // ZIP paths use forward slashes by spec.
                let zip_name = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join("/");
                zw.start_file(zip_name, *opts)
                    .map_err(|e| format!("zip start_file: {e}"))?;
                let bytes = std::fs::read(&p)
                    .map_err(|e| format!("read {}: {e}", p.display()))?;
                zw.write_all(&bytes)
                    .map_err(|e| format!("zip write: {e}"))?;
            }
        }
        Ok(())
    }

    walk(source_dir, source_dir, &mut zw, &opts)?;
    zw.finish().map_err(|e| format!("zip finish: {e}"))?;
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
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, created_at, updated_at FROM files{} ORDER BY created_at DESC LIMIT {} OFFSET {}",
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
            storage_kind: file.storage_kind,
            remote_provider: file.remote_provider,
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
            storage_kind: file.storage_kind,
            remote_provider: file.remote_provider,
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
                f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.created_at, f.updated_at
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
                f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.created_at, f.updated_at
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
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, created_at, updated_at FROM files WHERE id = ?"
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
        storage_kind: file.storage_kind,
        remote_provider: file.remote_provider,
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
    storage_kind: Option<String>,
) -> Result<FileCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Route remote imports through a separate path that uploads to Baidu
    // before persisting anything; the rest of this function is the
    // local-storage flow (move-or-copy + insert row with in_storage=1).
    if storage_kind.as_deref() == Some("remote") {
        let remote_upload_enabled: bool = sqlx::query_as::<_, (String,)>(
            "SELECT value FROM app_settings WHERE key = 'debug_remote_upload_enabled'",
        )
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?
        .map(|(v,)| v != "false")
        .unwrap_or(true);

        if remote_upload_enabled {
            return file_create_remote(
                &pool,
                path,
                display_name,
                category_id,
                tag_ids,
                author_ids,
                metadata,
                progress,
                cover_data,
                cover_mime_type,
            )
            .await;
        }
        // Debug flag disabled: fall through to local storage flow.
    }

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
    // Source kind drives the move/copy vs zip-on-commit branch below.
    let source_is_dir = source_path.is_dir();

    // Canonicalize paths for comparison
    let source_canonical = source_path.canonicalize()
        .map_err(|e| format!("Failed to resolve source path: {}", e))?;
    let storage_canonical = storage_path.canonicalize()
        .map_err(|e| format!("Failed to resolve storage path: {}", e))?;

    // Check source is not inside storage_path
    if source_canonical.starts_with(&storage_canonical) {
        return Err("SOURCE_ALREADY_IN_STORAGE".to_string());
    }

    // Determine destination folder. Imports must carry a category — the
    // legacy `_uncategorized` fallback was retired once the migration
    // helper moved every existing null row into the `novel` category.
    let cat_id = category_id.ok_or_else(|| "CATEGORY_REQUIRED".to_string())?;
    let cat_result: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT folder_name, name FROM categories WHERE id = ?",
    )
    .bind(cat_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let folder_name = match cat_result {
        Some((Some(folder), _)) => folder,
        Some((None, name)) => sanitize_folder_name(&name),
        None => return Err("CATEGORY_NOT_FOUND".to_string()),
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
    let dest_filename = if source_is_dir {
        // Image-folder import: package the directory into a `.zip` named
        // after the (sanitized) display_name. Source folder has no
        // extension, so we append `.zip` unconditionally.
        let stem = sanitize_filename(&validated_name);
        let stem = if stem.trim().is_empty() {
            sanitize_filename(source_filename)
        } else {
            stem
        };
        format!("{}.zip", stem)
    } else if should_clean_name {
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

    // Move or copy the file based on setting. Directory sources are
    // packaged as a `.zip` straight into the destination, then the source
    // folder is recursively removed (move-mode only).
    let final_path = if source_is_dir {
        let zipped = zip_image_dir(&source_canonical, &dest_path)?;
        if !use_copy {
            // Best-effort: orphan-on-failure is acceptable here; the DB
            // insert below is the source of truth, and the user can clean
            // up the source folder manually.
            let _ = fs::remove_dir_all(&source_canonical);
        }
        zipped
    } else if use_copy {
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

/// Remote (Baidu Netdisk) variant of `file_create`. Uploads the local
/// source to the user's configured `app_root` under a base64-url-encoded
/// filename, deletes the local source on success, and inserts the row
/// with `storage_kind='remote'` + `path=<remote_path>`. The `path` column
/// does double duty here — local rows hold a filesystem path, remote
/// rows hold the Baidu path, disambiguated via `storage_kind`.
async fn file_create_remote(
    pool: &sqlx::SqlitePool,
    local_path: String,
    display_name: String,
    category_id: Option<i64>,
    tag_ids: Option<Vec<i64>>,
    author_ids: Option<Vec<i64>>,
    metadata: Option<Vec<MetadataInput>>,
    progress: Option<String>,
    cover_data: Option<Vec<u8>>,
    cover_mime_type: Option<String>,
) -> Result<FileCreateResponse, String> {
    use base64::Engine;

    let validated_name = validate_display_name(&display_name)?;

    let source_path = PathBuf::from(&local_path);
    if !source_path.exists() {
        return Err("SOURCE_FILE_NOT_FOUND".to_string());
    }

    let source_filename = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(source_filename);
    let ext = source_path.extension().and_then(|e| e.to_str());

    // Base64-url-encoding the stem keeps the extension intact so Baidu's
    // mime detection still triggers (important for later browse/open via
    // the web UI) while obfuscating the original title in the remote
    // listing. No padding (=) since URL-safe base64 without padding is
    // still a valid filename and keeps the encoded form short.
    let encoded_stem = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(stem.as_bytes());
    let encoded_filename = match ext {
        Some(e) if !e.is_empty() => format!("{encoded_stem}.{e}"),
        _ => encoded_stem,
    };

    let remote_cfg = super::remote::load_config(pool).await;
    if !remote_cfg.enabled {
        return Err("REMOTE_STORAGE_NOT_CONFIGURED".to_string());
    }
    let app_root = remote_cfg.app_root.trim_end_matches('/');
    let remote_path = format!("{app_root}/{encoded_filename}");

    // Ensure the target path isn't already tracked — the user should see a
    // duplicate warning upstream, but guard here too in case a parallel
    // import got there first.
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM files WHERE path = ?")
        .bind(&remote_path)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    if existing.is_some() {
        return Err("FILE_ALREADY_EXISTS".to_string());
    }

    // Upload before any DB writes: if the upload fails, biblio's state is
    // unchanged and the user can retry without orphaned rows.
    let upload = super::remote::upload_to_remote(pool, &source_path, &remote_path).await?;

    // DB row first, then the local-source delete so a delete failure
    // still leaves the user's metadata captured. The source-delete error
    // gets swallowed (logged to stderr) — the file is already safely on
    // Baidu and a stray local copy is recoverable.
    let result = sqlx::query(
        "INSERT INTO files (\
            path, display_name, category_id, in_storage, original_path, file_status, progress, \
            storage_kind, remote_provider, remote_fs_id, remote_md5, remote_size\
         ) VALUES (?, ?, ?, 0, ?, 'available', ?, 'remote', 'baidu_netdisk', ?, ?, ?)",
    )
    .bind(&remote_path)
    .bind(&validated_name)
    .bind(category_id)
    .bind(&local_path)
    .bind(&progress)
    .bind(&upload.fs_id)
    .bind(&upload.md5)
    .bind(upload.size)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    let file_id = result.last_insert_rowid();

    if let Some(tags) = tag_ids {
        for tag_id in tags {
            sqlx::query("INSERT INTO file_tags (file_id, tag_id) VALUES (?, ?)")
                .bind(file_id)
                .bind(tag_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(authors) = author_ids {
        for author_id in authors {
            sqlx::query("INSERT INTO file_authors (file_id, author_id) VALUES (?, ?)")
                .bind(file_id)
                .bind(author_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(meta) = metadata {
        for m in meta {
            sqlx::query(
                "INSERT INTO metadata (file_id, key, value, data_type) VALUES (?, ?, ?, ?)",
            )
            .bind(file_id)
            .bind(&m.key)
            .bind(&m.value)
            .bind(m.data_type.unwrap_or_else(|| "text".to_string()))
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
    }

    if let Some(data) = cover_data {
        let mime = cover_mime_type.unwrap_or_else(|| "image/png".to_string());
        sqlx::query(
            "INSERT OR REPLACE INTO covers (file_id, data, mime_type) VALUES (?, ?, ?)",
        )
        .bind(file_id)
        .bind(&data)
        .bind(&mime)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to save cover: {}", e))?;
    }

    // Best-effort local delete. The remote upload + metadata row are
    // already persisted, so any failure here is a cleanup issue, not a
    // correctness one.
    if let Err(e) = fs::remove_file(&source_path) {
        eprintln!(
            "Remote import succeeded but local source delete failed ({}): {e}",
            source_path.display()
        );
    }

    Ok(FileCreateResponse { id: file_id })
}

/// Rename a file on disk to match current metadata, updating DB atomically.
/// Only renames .txt files. No-op for other extensions.
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
    if !matches!(ext_lower.as_deref(), Some("txt")) {
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

/// Physically move a file into the target category's folder and update
/// `files.path`. No category-equality or in_storage guards — the caller
/// decides whether the move should be attempted. Returns `Ok(true)` when
/// the file was actually moved, `Ok(false)` when it was already in the
/// destination folder.
async fn relocate_file_to_category_folder(
    pool: &sqlx::SqlitePool,
    file_id: i64,
    current_path: &str,
    target_category_id: i64,
) -> Result<bool, String> {
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

    let cat_row: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT folder_name, name FROM categories WHERE id = ?",
    )
    .bind(target_category_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let folder_name = match cat_row {
        Some((Some(folder), _)) => folder,
        Some((None, name)) => sanitize_folder_name(&name),
        None => return Err("CATEGORY_NOT_FOUND".to_string()),
    };

    let dest_folder = storage_canonical.join(&folder_name);
    if !dest_folder.exists() {
        fs::create_dir_all(&dest_folder)
            .map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    let current_pb = PathBuf::from(current_path);
    if let Some(parent) = current_pb.parent() {
        if parent == dest_folder {
            return Ok(false);
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

    Ok(true)
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

    let cat_id = new_category_id.ok_or_else(|| "CATEGORY_REQUIRED".to_string())?;
    relocate_file_to_category_folder(pool, file_id, &current_path, cat_id).await?;
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
/// the common case of a user picking a media folder.
///
/// Image-folder leaf collapse: a directory whose non-hidden direct
/// children are all image files (and which has no subdirectories) is
/// emitted as a single path. `file_prepare_import` routes such dir
/// paths through the comic pipeline; `file_create` zips them on commit.
/// The walker descends through every other directory, so a
/// `library/[author]/[work]/*.jpg` tree resolves to one comic per
/// `[work]` folder. Multi-level structures like
/// `vol/chapter-1/*.jpg, vol/chapter-2/*.jpg` are split into per-chapter
/// comics — there is no filesystem-only signal that distinguishes
/// sibling chapters of one comic from sibling comics of one author, so
/// this leaf-only rule errs on the side of finer-grained imports.
/// Result is sorted so repeated folder picks produce stable ordering.
#[tauri::command]
pub async fn list_files_in_folder(path: String) -> Result<Vec<String>, String> {
    use crate::pipeline::archive::is_image_filename;

    let root = std::path::Path::new(&path);
    if !root.exists() {
        return Err("PATH_NOT_FOUND".to_string());
    }
    if !root.is_dir() {
        return Err("NOT_A_DIRECTORY".to_string());
    }

    /// True iff every non-hidden direct child of `dir` is an image file
    /// AND `dir` contains no subdirectories. Hidden entries are ignored.
    /// Empty directories return false (no content to import).
    fn is_image_leaf(dir: &std::path::Path) -> std::io::Result<bool> {
        let mut saw_image = false;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                return Ok(false);
            }
            if p.is_file() {
                if !is_image_filename(&name_str) {
                    return Ok(false);
                }
                saw_image = true;
            }
        }
        Ok(saw_image)
    }

    fn walk(dir: &std::path::Path, out: &mut Vec<String>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                if is_image_leaf(&path)? {
                    if let Some(s) = path.to_str() {
                        out.push(s.to_string());
                    }
                } else {
                    walk(&path, out)?;
                }
            } else if path.is_file() {
                if let Some(s) = path.to_str() {
                    out.push(s.to_string());
                }
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    // Top-level: if the picked folder ITSELF is an image leaf, emit it
    // as a single comic instead of returning its images individually.
    if is_image_leaf(root).map_err(|e| format!("Failed to scan folder: {e}"))? {
        if let Some(s) = root.to_str() {
            files.push(s.to_string());
        }
    } else {
        walk(root, &mut files).map_err(|e| format!("Failed to walk folder: {e}"))?;
    }
    files.sort();
    Ok(files)
}

/// Post-commit cleanup for folder imports. Called by the frontend after
/// every per-file `file_create` in the batch succeeds.
///
/// Behavior:
/// - No-op when `had_folder_imports` is false (pure-archive folder picks
///   keep their picked root untouched, matching pre-feature behavior).
/// - No-op when `import_mode` is `'copy'` (copy semantics keep originals).
/// - Refuses to touch anything inside `storage_path` (defense in depth).
/// - Walks `folder_root` bottom-up and removes empty subdirectories.
///   If after the walk the root itself is empty, removes it. If
///   non-empty (the user had stray non-image files), leaves it alone
///   and logs to stderr — the import already succeeded; cleanup is
///   best-effort and never fails the call.
#[tauri::command]
pub async fn import_finalize(
    app: AppHandle,
    folder_root: String,
    had_folder_imports: bool,
) -> Result<(), String> {
    if !had_folder_imports {
        return Ok(());
    }

    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let import_mode: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'import_mode'",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;
    if import_mode.map(|(v,)| v == "copy").unwrap_or(false) {
        return Ok(());
    }

    let root = PathBuf::from(&folder_root);
    if !root.exists() {
        // file_create already removed every leaf; nothing to do.
        return Ok(());
    }
    if !root.is_dir() {
        return Err("FOLDER_ROOT_NOT_A_DIRECTORY".to_string());
    }

    // Defense in depth: never recurse into anything under storage_path.
    let storage_path: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;
    if let Some((sp,)) = storage_path {
        if !sp.is_empty() {
            let storage_canonical = std::path::Path::new(&sp)
                .canonicalize()
                .map_err(|e| format!("Failed to resolve storage path: {e}"))?;
            let root_canonical = root
                .canonicalize()
                .map_err(|e| format!("Failed to resolve folder root: {e}"))?;
            if root_canonical.starts_with(&storage_canonical) {
                return Err("FOLDER_ROOT_INSIDE_STORAGE".to_string());
            }
        }
    }

    /// True iff `dir` recursively contains no non-hidden files. Hidden
    /// entries (`.DS_Store`, `.localized`, etc.) are transparent —
    /// macOS Finder seeds them everywhere it's been opened, and they
    /// would otherwise block cleanup of folders that are otherwise empty
    /// after `file_create` removed the leaf source dirs. Mirrors the
    /// hidden-skip convention used by `list_files_in_folder` and
    /// `zip_image_dir`.
    fn has_only_hidden_content(dir: &std::path::Path) -> std::io::Result<bool> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                if !has_only_hidden_content(&p)? {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }
        Ok(true)
    }

    match has_only_hidden_content(&root) {
        Ok(true) => {
            // `remove_dir_all` nukes the dir tree including the hidden
            // metadata we treated as transparent above.
            if let Err(e) = std::fs::remove_dir_all(&root) {
                eprintln!(
                    "import_finalize: remove_dir_all failed for {}: {e}",
                    root.display()
                );
            }
            Ok(())
        }
        Ok(false) => {
            eprintln!(
                "import_finalize: {} not removed (real files remain after leaf cleanup)",
                root.display()
            );
            Ok(())
        }
        Err(e) => {
            eprintln!("import_finalize: cleanup failed for {}: {e}", root.display());
            Ok(())
        }
    }
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

    // Get file info before deleting. `storage_kind` tells us whether the
    // row's `path` is a local filesystem path or a Baidu Pan path.
    let file_info: Option<(String, bool, String)> = sqlx::query_as(
        "SELECT path, in_storage, storage_kind FROM files WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some((path, in_storage, storage_kind)) = file_info {
        if storage_kind == "remote" {
            // Best-effort: if Baidu delete fails (token expired, network
            // error, file already missing server-side), log and continue
            // with the DB delete. The local row is the source of truth
            // from the user's perspective, and a stray remote file can be
            // cleaned up manually later.
            if let Err(e) = super::remote::delete_on_remote(&pool, &path).await {
                eprintln!("Remote delete failed for {path}: {e}");
            }
        } else if in_storage {
            // Local file in biblio's managed storage — remove from disk.
            let _ = fs::remove_file(&path);
        }

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

    // Determine new folder. Files must carry a category — the legacy
    // `_uncategorized` fallback was retired with the Debug-section
    // migration helper.
    let cat_id = category_id.ok_or_else(|| "CATEGORY_REQUIRED".to_string())?;
    let cat_result: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT folder_name, name FROM categories WHERE id = ?",
    )
    .bind(cat_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;
    let folder_name = match cat_result {
        Some((Some(folder), _)) => folder,
        Some((None, name)) => sanitize_folder_name(&name),
        None => return Err("CATEGORY_NOT_FOUND".to_string()),
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

/// Minimum query length, in Unicode characters, for an FTS5 trigram lookup.
/// Trigram only indexes 3-character windows, so anything shorter has no
/// rows in the index and must use a `LIKE '%q%'` fallback instead.
const TRIGRAM_MIN_CHARS: usize = 3;

/// What kind of SQL filter to apply for a typed search query. Returned by
/// [`prepare_search_filter`] so `file_search` can pick the matching SQL.
enum SearchFilter {
    /// Use FTS5 `MATCH` with the given expression. Fast, ranked.
    Fts(String),
    /// Use `display_name LIKE %p% OR path LIKE %p%` with the given pattern
    /// (already wrapped with `%` and SQL wildcards escaped). Used for
    /// queries shorter than the trigram window.
    Like(String),
}

/// Translate a user-typed query into either an FTS5 MATCH expression or a
/// LIKE fallback for sub-trigram-length queries.
///
/// Strategy:
///   1. Replace every FTS5 operator character (`"'`:()-+*^`) with a space
///      so we never accidentally parse a user's punctuation as syntax.
///   2. Split on whitespace; if the longest remaining token is < 3 chars,
///      fall back to `LIKE '%q%'` against the trimmed raw query — trigram
///      indexes 3-character windows and returns nothing for shorter input.
///   3. Otherwise quote each token and AND them together. The trigram
///      tokenizer matches substrings inside indexed text, so we don't need
///      a `*` prefix marker.
///
/// Returns None if the query has no usable content (all whitespace, all
/// punctuation, etc.) — callers should treat that as "no filter" rather
/// than issuing an empty MATCH (which FTS5 rejects).
fn prepare_search_filter(raw: &str) -> Option<SearchFilter> {
    let sanitized: String = raw
        .chars()
        .map(|c| match c {
            '"' | '\'' | ':' | '(' | ')' | '+' | '-' | '*' | '^' => ' ',
            c => c,
        })
        .collect();

    let terms: Vec<&str> = sanitized
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .collect();

    if terms.is_empty() {
        return None;
    }

    let longest = terms.iter().map(|t| t.chars().count()).max().unwrap_or(0);
    if longest < TRIGRAM_MIN_CHARS {
        // Below the trigram window — fall back to a LIKE scan over the
        // raw trimmed query. Escape `%`, `_`, and `\` so the user can't
        // smuggle wildcards into the pattern.
        let pattern = raw
            .trim()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        if pattern.is_empty() {
            return None;
        }
        return Some(SearchFilter::Like(format!("%{}%", pattern)));
    }

    // Quote each term so spaces, slashes, and other token-internal
    // punctuation are searched literally rather than parsed as FTS5
    // syntax. Trigram needs the entire term as a contiguous substring.
    let quoted: Vec<String> = terms.iter().map(|t| format!("\"{}\"", t)).collect();
    Some(SearchFilter::Fts(quoted.join(" ")))
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
    let Some(filter) = prepare_search_filter(&query) else {
        return Ok(FileListResponse { files: Vec::new(), total: 0 });
    };

    // Same pattern as file_list: build the WHERE once and share it between
    // the row query and the count query so `total` matches what's loadable.
    let mut where_tail = String::new();
    if let Some(cat_id) = category_id {
        where_tail.push_str(&format!(" AND f.category_id = {}", cat_id));
    }

    let (row_query, count_query, bind_value) = match &filter {
        SearchFilter::Fts(expr) => (
            format!(
                "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.created_at, f.updated_at \
                 FROM files f \
                 JOIN files_fts ON files_fts.rowid = f.id \
                 WHERE files_fts MATCH ?{} \
                 ORDER BY f.created_at DESC LIMIT {} OFFSET {}",
                where_tail, limit, offset
            ),
            format!(
                "SELECT COUNT(*) FROM files f \
                 JOIN files_fts ON files_fts.rowid = f.id \
                 WHERE files_fts MATCH ?{}",
                where_tail
            ),
            expr.clone(),
        ),
        SearchFilter::Like(pattern) => (
            format!(
                "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.created_at, f.updated_at \
                 FROM files f \
                 WHERE (f.display_name LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'){} \
                 ORDER BY f.created_at DESC LIMIT {} OFFSET {}",
                where_tail, limit, offset
            ),
            format!(
                "SELECT COUNT(*) FROM files f \
                 WHERE (f.display_name LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'){}",
                where_tail
            ),
            pattern.clone(),
        ),
    };

    let mut row_stmt = sqlx::query_as::<_, FileEntry>(&row_query).bind(&bind_value);
    if matches!(filter, SearchFilter::Like(_)) {
        row_stmt = row_stmt.bind(&bind_value);
    }
    let files: Vec<FileEntry> = row_stmt.fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let mut count_stmt = sqlx::query_as::<_, (i64,)>(&count_query).bind(&bind_value);
    if matches!(filter, SearchFilter::Like(_)) {
        count_stmt = count_stmt.bind(&bind_value);
    }
    let total: (i64,) = count_stmt.fetch_one(&pool).await.map_err(|e| e.to_string())?;

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
                "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, created_at, updated_at FROM files WHERE id IN ({})",
                placeholders
            );
            sqlx::query_as(&query)
                .fetch_all(&pool)
                .await
                .map_err(|e| e.to_string())?
        }
        None => {
            sqlx::query_as("SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, created_at, updated_at FROM files")
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

    file_create(app, path, display_name, category_id, tag_ids, author_ids, metadata, progress, cover_data, cover_mime_type, None).await
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

    fn fts_expr(raw: &str) -> Option<String> {
        match prepare_search_filter(raw)? {
            SearchFilter::Fts(s) => Some(s),
            SearchFilter::Like(_) => None,
        }
    }

    fn like_pattern(raw: &str) -> Option<String> {
        match prepare_search_filter(raw)? {
            SearchFilter::Like(s) => Some(s),
            SearchFilter::Fts(_) => None,
        }
    }

    #[test]
    fn search_filter_single_token_quoted_for_fts() {
        // Trigram tokenizer matches substrings inside indexed text, so the
        // quoted whole-token form is enough — no `*` prefix marker needed.
        assert_eq!(fts_expr("三体老师").as_deref(), Some("\"三体老师\""));
    }

    #[test]
    fn search_filter_multiple_tokens_anded_with_quotes() {
        assert_eq!(
            fts_expr("三体老师 刘慈欣").as_deref(),
            Some("\"三体老师\" \"刘慈欣\"")
        );
    }

    #[test]
    fn search_filter_strips_fts5_operators() {
        assert_eq!(
            fts_expr("hello(world):today").as_deref(),
            Some("\"hello\" \"world\" \"today\"")
        );
    }

    #[test]
    fn search_filter_short_query_falls_back_to_like() {
        // Below the trigram window — must use LIKE with the raw pattern.
        assert_eq!(like_pattern("体").as_deref(), Some("%体%"));
        assert_eq!(like_pattern("三体").as_deref(), Some("%三体%"));
    }

    #[test]
    fn search_filter_short_query_escapes_like_wildcards() {
        // SQL wildcards in user input must be escaped so a literal `%` or
        // `_` can't expand the match. We escape with `\` and bind `ESCAPE '\\'`.
        assert_eq!(like_pattern("a%").as_deref(), Some("%a\\%%"));
        assert_eq!(like_pattern("a_").as_deref(), Some("%a\\_%"));
        assert_eq!(like_pattern("\\a").as_deref(), Some("%\\\\a%"));
    }

    #[test]
    fn search_filter_empty_or_punctuation_returns_none() {
        assert!(prepare_search_filter("").is_none());
        assert!(prepare_search_filter("   ").is_none());
        assert!(prepare_search_filter("\"\"()").is_none());
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