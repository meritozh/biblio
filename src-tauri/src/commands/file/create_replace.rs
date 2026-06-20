use super::*;
#[tauri::command]
pub async fn file_get(app: AppHandle, id: i64) -> Result<FileWithDetails, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let file: FileEntry = sqlx::query_as(
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, is_favorite, created_at, updated_at FROM files WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("File not found")?;

    let category: Option<Category> = if let Some(cat_id) = file.category_id {
        sqlx::query_as("SELECT id, name, description, icon, is_default, folder_name, schema_slug, view_config, created_at FROM categories WHERE id = ?")
            .bind(cat_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?
    } else {
        None
    };

    let tags: Vec<Tag> = sqlx::query_as(
        "SELECT t.id, t.name, t.color, t.created_at FROM tags t
         INNER JOIN file_tags ft ON t.id = ft.tag_id WHERE ft.file_id = ?",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let authors: Vec<Author> = sqlx::query_as(
        "SELECT a.id, a.name, a.created_at FROM authors a
         INNER JOIN file_authors fa ON a.id = fa.author_id WHERE fa.file_id = ?",
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let metadata: Vec<Metadata> =
        sqlx::query_as("SELECT id, file_id, key, value, data_type FROM metadata WHERE file_id = ?")
            .bind(id)
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?;

    // Resolve stored relative paths → absolute at the IPC boundary.
    let roots = crate::commands::settings::load_path_roots(&pool).await?;
    let storage_kind_ref = file.storage_kind.as_deref().unwrap_or("local");
    let abs_path = crate::path_resolve::to_absolute(
        storage_kind_ref,
        &file.path,
        &roots.storage_path,
        &roots.app_root,
    )
    .to_string_lossy()
    .to_string();
    let abs_cache = file.local_cache_path.as_ref().map(|p| {
        crate::path_resolve::cache_to_absolute(p, &roots.storage_path)
            .to_string_lossy()
            .to_string()
    });

    Ok(FileWithDetails {
        id: file.id,
        path: abs_path,
        display_name: file.display_name,
        category_id: file.category_id,
        file_status: file.file_status,
        in_storage: file.in_storage,
        original_path: file.original_path,
        progress: file.progress,
        storage_kind: file.storage_kind,
        remote_provider: file.remote_provider,
        local_cache_path: abs_cache,
        is_favorite: file.is_favorite,
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
    staged_cover_path: Option<String>,
) -> Result<FileCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Inline `cover_data` (user uploaded one in the form) wins. When absent,
    // fall back to whichever cover the pipeline staged for this import path
    // — those bytes never crossed IPC, so this `take` is the only consumer.
    let (cover_data, cover_mime_type) = if cover_data.is_some() {
        (cover_data, cover_mime_type)
    } else if let Some(staged_path) = staged_cover_path.as_deref() {
        let cache = app.state::<crate::commands::processing::PreparedCoverCache>();
        match cache.take(staged_path) {
            Some((bytes, mime)) => (Some(bytes), Some(mime)),
            None => (None, cover_mime_type),
        }
    } else {
        (None, cover_mime_type)
    };

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
    let storage_path_result: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = 'storage_path'")
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
    let source_canonical = source_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve source path: {}", e))?;
    let storage_canonical = storage_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve storage path: {}", e))?;

    // Check source is not inside storage_path
    if source_canonical.starts_with(&storage_canonical) {
        return Err("SOURCE_ALREADY_IN_STORAGE".to_string());
    }

    // Determine destination folder. Imports must carry a category — the
    // legacy `_uncategorized` fallback was retired once the migration
    // helper moved every existing null row into the `novel` category.
    let cat_id = category_id.ok_or_else(|| "CATEGORY_REQUIRED".to_string())?;
    let cat_result: Option<(Option<String>, String, String)> =
        sqlx::query_as("SELECT folder_name, name, schema_slug FROM categories WHERE id = ?")
            .bind(cat_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let (folder_name, schema_slug) = match cat_result {
        Some((Some(folder), _, slug)) => (folder, crate::schema::SchemaSlug::from_str(&slug)),
        Some((None, name, slug)) => (
            sanitize_folder_name(&name),
            crate::schema::SchemaSlug::from_str(&slug),
        ),
        None => return Err("CATEGORY_NOT_FOUND".to_string()),
    };

    // Create destination folder if needed
    let dest_folder = storage_canonical.join(&folder_name);
    if !dest_folder.exists() {
        fs::create_dir_all(&dest_folder).map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    // Get source filename and extension
    let source_filename = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;
    let ext_lower = source_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    let should_clean_name = matches!(ext_lower.as_deref(), Some("txt") | Some("epub"));

    // Resolve author names up-front (needed for clean filename)
    let resolved_author_names: Vec<String> = match author_ids.as_ref() {
        Some(ids) if !ids.is_empty() => {
            let placeholders = (0..ids.len()).map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!("SELECT name FROM authors WHERE id IN ({})", placeholders);
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
    let import_mode: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = 'import_mode'")
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let use_copy = import_mode.map(|(v,)| v == "copy").unwrap_or(false);

    // Stage the destination WITHOUT destroying the source yet. The row writes
    // below run in one transaction; if any fail we roll back and delete the
    // staged file, and because the source is still intact (the move-mode
    // source delete is deferred until after commit) that cleanup can never
    // lose data. Previously the source was moved/removed before the INSERT, so
    // a DB failure orphaned the bytes (move mode) or left a row with partial
    // children (a mid-loop child-insert failure).
    let final_path = if source_is_dir {
        // Both comic and galgame folder imports zip on commit. Comics keep
        // only image files (`zip_image_dir`); galgames archive the whole
        // directory tree verbatim (`zip_dir`) since a game is more than its
        // images. Other schemas don't accept directory inputs (validated at
        // import time), so this binary split is exhaustive in practice.
        if schema_slug == crate::schema::SchemaSlug::Galgame {
            zip_dir(&source_canonical, &dest_path)?
        } else {
            zip_image_dir(&source_canonical, &dest_path)?
        }
    } else {
        // Copy (not move) so the source survives a rolled-back transaction;
        // the original is removed below only after the commit succeeds.
        copy_file(&source_canonical, &dest_path)?
    };
    let final_path_str = final_path.to_string_lossy().to_string();
    // Store the destination path RELATIVE to storage_path so the user can
    // rename / move the storage folder without rewriting every row. The
    // resolver in `path_resolve::to_absolute` rebuilds the full path at
    // read time using whatever `storage_path` is currently set to.
    let storage_path_str = storage_path.to_string_lossy();
    let stored_path = crate::path_resolve::to_relative_local(&final_path_str, &storage_path_str);

    // Compress the cover (if any) BEFORE opening the transaction so a
    // compression failure aborts before any DB write and the transaction
    // stays short. Route through the shared compressor: user-uploaded
    // replacements arrive uncompressed, and even pipeline-staged bytes
    // (already JPEG'd by `CoverCompressNode`) round-trip cheaply through it,
    // avoiding the 1 MB+ blobs the helper exists to prevent.
    let _ = cover_mime_type;
    let compressed_cover = match cover_data {
        Some(data) => Some(
            crate::commands::cover::compress_cover_bytes(&data).map_err(|e| {
                // Clean up the staged destination file on compression failure
                let _ = std::fs::remove_file(&final_path);
                format!("Failed to compress cover: {e}")
            })?,
        ),
        None => None,
    };

    // All row writes (parent + tags + authors + metadata + cover) go through
    // one transaction so a mid-sequence failure rolls back to nothing rather
    // than leaving a row with partial children.
    let insert_result: Result<i64, String> = async move {
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

        let result = sqlx::query(
            "INSERT INTO files (path, display_name, category_id, in_storage, original_path, file_status, progress) VALUES (?, ?, ?, 1, ?, 'available', ?)"
        )
        .bind(&stored_path)
        .bind(&validated_name)
        .bind(category_id)
        .bind(&path)  // original_path is the source path
        .bind(&progress)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        let file_id = result.last_insert_rowid();

        if let Some(tags) = tag_ids {
            for tag_id in tags {
                sqlx::query("INSERT INTO file_tags (file_id, tag_id) VALUES (?, ?)")
                    .bind(file_id)
                    .bind(tag_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }

        if let Some(authors) = author_ids {
            for author_id in authors {
                sqlx::query("INSERT INTO file_authors (file_id, author_id) VALUES (?, ?)")
                    .bind(file_id)
                    .bind(author_id)
                    .execute(&mut *tx)
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
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }

        if let Some(compressed) = &compressed_cover {
            sqlx::query(
                "INSERT OR REPLACE INTO covers (file_id, data, mime_type) VALUES (?, ?, ?)"
            )
            .bind(file_id)
            .bind(compressed)
            .bind("image/jpeg")
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to save cover: {e}"))?;
        }

        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(file_id)
    }
    .await;

    let file_id = match insert_result {
        Ok(id) => id,
        Err(e) => {
            // DB rolled back — remove the staged destination so it doesn't
            // leak as an orphan. The source is untouched (move-mode delete is
            // deferred below), so retrying from the original is always safe.
            let _ = fs::remove_file(&final_path);
            return Err(e);
        }
    };

    // DB committed. In move mode, remove the original source now that the
    // import is durably recorded. Best-effort: a failed cleanup leaves a
    // harmless duplicate at the source, never data loss.
    if !use_copy {
        if source_is_dir {
            let _ = fs::remove_dir_all(&source_canonical);
        } else {
            let _ = fs::remove_file(&source_canonical);
        }
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
    let file_info: Option<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT path, display_name, progress, COALESCE(storage_kind, 'local') FROM files WHERE id = ?"
    )
    .bind(file_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((stored_path, display_name, progress, storage_kind)) = file_info else {
        return Ok(());
    };

    // The stored path is relative to storage_path (post-v11). Resolve to
    // an absolute path for the on-disk rename, then strip the prefix
    // again on the way back into the DB.
    let roots = crate::commands::settings::load_path_roots(pool).await?;
    let current_path = crate::path_resolve::to_absolute(
        &storage_kind,
        &stored_path,
        &roots.storage_path,
        &roots.app_root,
    );

    let ext_lower = current_path
        .extension()
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
    let new_stored = crate::path_resolve::to_relative_local(&new_path_str, &roots.storage_path);

    // Transaction: update DB first, then rename file
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE files SET path = ? WHERE id = ?")
        .bind(&new_stored)
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
    stored_path: &str,
    target_category_id: i64,
) -> Result<bool, String> {
    let storage_row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = 'storage_path'")
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
    let storage_canonical_str = storage_canonical.to_string_lossy().to_string();

    let cat_row: Option<(Option<String>, String)> =
        sqlx::query_as("SELECT folder_name, name FROM categories WHERE id = ?")
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
        fs::create_dir_all(&dest_folder).map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    // Caller passes the stored (relative) path. Resolve to an absolute
    // filesystem path for the on-disk operations; the relativizer at the
    // bottom strips the prefix again before UPDATE.
    let current_pb =
        crate::path_resolve::to_absolute("local", stored_path, &storage_canonical_str, "");
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
    let new_stored =
        crate::path_resolve::to_relative_local(&final_path_str, &storage_canonical_str);

    sqlx::query("UPDATE files SET path = ? WHERE id = ?")
        .bind(&new_stored)
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
pub(super) async fn move_file_to_category_folder(
    pool: &sqlx::SqlitePool,
    file_id: i64,
    new_category_id: Option<i64>,
) -> Result<(), String> {
    let file_info: Option<(String, bool, Option<i64>)> =
        sqlx::query_as("SELECT path, in_storage, category_id FROM files WHERE id = ?")
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
