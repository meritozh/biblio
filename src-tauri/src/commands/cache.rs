//! Cache management for remote files pulled to disk.
//!
//! Remote rows carry a `local_cache_path` once the download worker has
//! pulled them; these commands let the user open or clear that cache
//! from the per-file context menu. Local rows go through the same
//! `cache_open` so the menu doesn't need to branch on `storage_kind` —
//! the backend resolves the right path from the DB.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_sql::{DbInstances, DbPool};

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

#[derive(Serialize)]
pub struct CacheActionResponse {
    pub success: bool,
}

/// Open the on-disk copy of a file with the system default app.
///
/// Resolution order:
///   1. `local_cache_path` — present once the download worker has pulled
///      a remote row to `<storage_path>/.cache/`.
///   2. `path` when `storage_kind = 'local'` — the row's `path` is the
///      filesystem location.
///
/// Returns `CACHE_NOT_FOUND` for remote rows that haven't been cached
/// yet so the UI can surface a clear "Download first" hint. The opener
/// plugin can still fail (no default app for the extension); that error
/// is surfaced as-is.
#[tauri::command]
pub async fn cache_open(app: AppHandle, file_id: i64) -> Result<CacheActionResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let row: Option<(Option<String>, String, String)> = sqlx::query_as(
        "SELECT local_cache_path, path, COALESCE(storage_kind, 'local') FROM files WHERE id = ?",
    )
    .bind(file_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((local_cache_path, path, storage_kind)) = row else {
        return Err("FILE_NOT_FOUND".to_string());
    };

    let target = match (local_cache_path.as_deref(), storage_kind.as_str()) {
        (Some(cache), _) if !cache.is_empty() => cache.to_string(),
        (_, "local") => path,
        _ => return Err("CACHE_NOT_FOUND".to_string()),
    };

    app.opener()
        .open_path(target, None::<&str>)
        .map_err(|e| format!("Failed to open: {e}"))?;

    Ok(CacheActionResponse { success: true })
}

/// Remove the local cache copy for a remote row.
///
/// Strict ordering: delete from disk first, NULL the column second.
/// Matches `file_delete` / `file_replace` policy so we never end up with
/// the DB pointing at a path that no longer exists, or worse, vice
/// versa. `NotFound` on the disk delete is treated as success (the
/// invariant "the file is gone" is already satisfied — the user just
/// wants the column cleaned up).
#[tauri::command]
pub async fn cache_clear(app: AppHandle, file_id: i64) -> Result<CacheActionResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT local_cache_path FROM files WHERE id = ?")
            .bind(file_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    let Some((local_cache_path,)) = row else {
        return Err("FILE_NOT_FOUND".to_string());
    };

    let Some(cache_path) = local_cache_path.filter(|s| !s.is_empty()) else {
        // Already cleared — column is null. Treat as success so a
        // double-click doesn't error.
        return Ok(CacheActionResponse { success: true });
    };

    if let Err(e) = std::fs::remove_file(PathBuf::from(&cache_path)) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(format!("Failed to remove cache file: {e}"));
        }
    }

    sqlx::query("UPDATE files SET local_cache_path = NULL WHERE id = ?")
        .bind(file_id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CacheActionResponse { success: true })
}
