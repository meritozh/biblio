use super::*;
/// Move a file at an arbitrary path on disk to the system Trash — used
/// for the "Delete" choice on the import duplicate dialog, where the
/// file is NOT yet in the DB (so `file_delete`, which keys off id,
/// doesn't apply).
///
/// Uses the `trash` crate (which delegates to macOS NSWorkspace's
/// recycle / Linux XDG trash / Windows IFileOperation) instead of
/// `std::fs::remove_file`.
#[tauri::command]
pub async fn file_delete_source(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        // Nothing to remove — treat as success so the UI flow doesn't error.
        return Ok(());
    }
    trash::delete(p).map_err(|e| {
        let err_str = e.to_string().to_lowercase();
        if err_str.contains("permission") {
            "PERMISSION_DENIED".to_string()
        } else if err_str.contains("being used") || err_str.contains("locked") {
            "FILE_LOCKED".to_string()
        } else {
            format!("Failed to move source file to Trash: {e}")
        }
    })
}

#[tauri::command]
pub async fn file_delete(app: AppHandle, id: i64) -> Result<FileDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Get file info before deleting. `storage_kind` tells us whether the
    // row's `path` is a local filesystem path or a Baidu Pan path.
    let file_info: Option<(String, bool, String)> =
        sqlx::query_as("SELECT path, in_storage, storage_kind FROM files WHERE id = ?")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let Some((stored_path, in_storage, storage_kind)) = file_info else {
        // Already gone — treat as success so a double-click doesn't error.
        return Ok(FileDeleteResponse { success: true });
    };

    // Resolve stored relative path → absolute for the external op.
    // Baidu's API and fs::remove both want the full path.
    let roots = super::settings::load_path_roots(&pool).await?;
    let path = crate::path_resolve::to_absolute(
        &storage_kind,
        &stored_path,
        &roots.storage_path,
        &roots.app_root,
    )
    .to_string_lossy()
    .to_string();

    // Strict ordering: external resource first, DB row second. A failure
    // here aborts before the row is touched so the user can retry against
    // the same id. Matches the bulk-delete worker's policy and prevents
    // orphans on Baidu Pan / in the storage folder.
    if storage_kind == "remote" {
        super::remote::delete_on_remote_for_file(&pool, id, &path).await?;
    } else if in_storage {
        if let Err(e) = fs::remove_file(&path) {
            // "Already missing on disk" satisfies the invariant — the
            // external state is what we wanted. Anything else (permission
            // denied, file locked, IO error) propagates.
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(format!("Failed to remove local file: {e}"));
            }
        }
    }

    sqlx::query("DELETE FROM files WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

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
    let file_info: Option<(String, bool, Option<i64>)> =
        sqlx::query_as("SELECT path, in_storage, category_id FROM files WHERE id = ?")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let (current_path, in_storage, _current_category) =
        file_info.ok_or("FILE_NOT_FOUND".to_string())?;

    // Verify file is in storage
    if !in_storage {
        return Err("FILE_NOT_IN_STORAGE".to_string());
    }

    // Get storage_path
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

    // Canonicalize storage path
    let storage_canonical = storage_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve storage path: {}", e))?;

    // Determine new folder. Files must carry a category — the legacy
    // `_uncategorized` fallback was retired with the Debug-section
    // migration helper.
    let cat_id = category_id.ok_or_else(|| "CATEGORY_REQUIRED".to_string())?;
    let cat_result: Option<(Option<String>, String)> =
        sqlx::query_as("SELECT folder_name, name FROM categories WHERE id = ?")
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
        fs::create_dir_all(&dest_folder).map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    // Resolve the stored relative path to an absolute filesystem path
    // for the move operation; we strip the prefix again before UPDATE.
    let storage_canonical_str = storage_canonical.to_string_lossy().to_string();
    let current_path_buf =
        crate::path_resolve::to_absolute("local", &current_path, &storage_canonical_str, "");
    let filename = current_path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;

    // Check if already in the correct folder
    if let Some(parent) = current_path_buf.parent()
        && parent == dest_folder
    {
        // Already in correct folder, just update database
        sqlx::query(
            "UPDATE files SET category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(category_id)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

        return Ok(FileMoveCategoryResponse {
            success: true,
            new_path: current_path_buf.to_string_lossy().to_string(),
        });
    }

    // Move the file
    let dest_path = dest_folder.join(filename);
    let final_path = move_file(&current_path_buf, &dest_path)?;
    let final_path_str = final_path.to_string_lossy().to_string();
    let new_stored =
        crate::path_resolve::to_relative_local(&final_path_str, &storage_canonical_str);

    // Update database
    sqlx::query(
        "UPDATE files SET path = ?, category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(&new_stored)
    .bind(category_id)
    .bind(id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    // Return the absolute path — TS callers expect to use it directly
    // for follow-up disk ops (open, etc.). Internal storage stays
    // relative; this is just the IPC-boundary shape.
    Ok(FileMoveCategoryResponse {
        success: true,
        new_path: final_path_str,
    })
}

#[derive(Serialize)]
pub struct FileMoveCategoryResponse {
    pub success: bool,
    pub new_path: String,
}
