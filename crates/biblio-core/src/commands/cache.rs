//! Cache management for remote files pulled to disk.
//!
//! Remote rows carry a `local_cache_path` once the download worker has
//! pulled them; these commands let the user open or clear that cache
//! from the per-file context menu. Local rows go through the same
//! `cache_open` so the menu doesn't need to branch on `storage_kind` —
//! the backend resolves the right path from the DB.

use crate::Library;
use serde::Serialize;
use std::{
    io,
    path::{Path, PathBuf},
};
use tokio::sync::watch;

pub use crate::services::download_worker::CopyState;

/// Read the current value before waiting for a change. Dropping this subscription
/// leaves the shared transfer running for later opens and manual downloads.
pub async fn readable_copy(
    app: &Library,
    file_id: i64,
) -> Result<watch::Receiver<CopyState>, String> {
    // A cache publication cannot pass between our disk check and registration.
    // Transfers proceed independently while readers resolve their paths.
    let _read = app.cache_access.read().await;
    let (cache, path, kind): (Option<String>, String, String) = sqlx::query_as(
        "SELECT local_cache_path, path, COALESCE(storage_kind, 'local') FROM files WHERE id = ?",
    )
    .bind(file_id)
    .fetch_optional(&app.pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("FILE_NOT_FOUND")?;
    let roots = super::settings::load_path_roots(&app.pool).await?;
    let cache = cache.as_deref().filter(|_| kind == "remote");
    if let Some(target) = copy_path(cache, &path, &kind, &roots) {
        match check_readable(&target).await {
            Ok(()) => {
                let absolute = std::path::absolute(target).map_err(|e| e.to_string())?;
                return Ok(watch::channel(CopyState::Ready(absolute)).1);
            }
            Err(e) if kind == "remote" && e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("Cannot read {}: {e}", target.display())),
        }
    }
    app.queues.download.enqueue(file_id)
}

async fn check_readable(path: &Path) -> io::Result<()> {
    if !tokio::fs::metadata(path).await?.is_file() {
        return Err(io::Error::other("not a regular file"));
    }
    tokio::fs::File::open(path).await?;
    Ok(())
}

fn copy_path(
    cache: Option<&str>,
    path: &str,
    kind: &str,
    roots: &super::settings::PathRoots,
) -> Option<PathBuf> {
    if let Some(cache) = cache.filter(|s| !s.is_empty()) {
        Some(crate::path_resolve::cache_to_absolute(
            cache,
            &roots.storage_path,
        ))
    } else if kind == "local" {
        Some(crate::path_resolve::to_absolute(
            kind,
            path,
            &roots.storage_path,
            &roots.app_root,
        ))
    } else {
        None
    }
}

#[derive(Serialize)]
pub struct CacheActionResponse {
    pub success: bool,
}

/// Open the on-disk copy of a file with the configured app, or the
/// system default if none is set on the file's category.
///
/// Resolution order for the target path:
///   1. `local_cache_path` — present once the download worker has pulled
///      a remote row to `<storage_path>/.cache/`.
///   2. `path` when `storage_kind = 'local'` — the row's `path` is the
///      filesystem location.
///
/// Resolution order for the app:
///   1. `categories.view_config.open_app` — per-category override; the
///      string format is platform-dependent (macOS app name / bundle id,
///      Windows .exe path, Linux command), so the system opener receives it verbatim.
///   2. None → OS default for the extension.
///
/// A missing category, NULL `view_config`, malformed JSON, or empty
/// `open_app` all fall through to the OS default — never an error.
///
/// Returns `CACHE_NOT_FOUND` for remote rows that haven't been cached
/// yet so the UI can surface a clear "Download first" hint. The system
/// opener can still fail (no default app for the extension, or the
/// configured app isn't installed); that error is surfaced as-is.
pub async fn cache_open(app: Library, file_id: i64) -> Result<CacheActionResponse, String> {
    let _read = app.cache_access.read().await;
    let pool = app.pool.clone();

    let row: Option<(Option<String>, String, String, Option<String>)> = sqlx::query_as(
        "SELECT f.local_cache_path, f.path, COALESCE(f.storage_kind, 'local'), c.view_config
         FROM files f
         LEFT JOIN categories c ON c.id = f.category_id
         WHERE f.id = ?",
    )
    .bind(file_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((local_cache_path, path, storage_kind, view_config)) = row else {
        return Err("FILE_NOT_FOUND".to_string());
    };

    // Stored paths are relative to storage_path (cache + local rows) or
    // app_root (remote rows). Resolve to absolute before opening.
    let roots = super::settings::load_path_roots(&pool).await?;
    let target = copy_path(local_cache_path.as_deref(), &path, &storage_kind, &roots)
        .ok_or("CACHE_NOT_FOUND")?;

    let configured_app = extract_open_app(view_config.as_deref());

    match configured_app {
        Some(program) => open::with_detached(target, program),
        None => open::that_detached(target),
    }
    .map_err(|e| format!("Failed to open: {e}"))?;

    Ok(CacheActionResponse { success: true })
}

/// Pull `open_app` out of a `view_config` JSON blob, returning `None`
/// when the column is NULL, parse fails, the field is absent, or the
/// value is the empty string. Centralizes the "treat as no-override"
/// contract so `cache_open` doesn't sprout branches.
fn extract_open_app(view_config: Option<&str>) -> Option<String> {
    let raw = view_config?;
    let parsed: serde_json::Value = serde_json::from_str(raw).ok()?;
    let s = parsed.get("open_app")?.as_str()?.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Remove the local cache copy for a remote row.
///
/// Strict ordering: delete from disk first, NULL the column second.
/// Matches `file_delete` / `file_replace` policy so we never end up with
/// the DB pointing at a path that no longer exists, or worse, vice
/// versa. `NotFound` on the disk delete is treated as success (the
/// invariant "the file is gone" is already satisfied — the user just
/// wants the column cleaned up).
pub async fn cache_clear(app: Library, file_id: i64) -> Result<CacheActionResponse, String> {
    let _write = app.cache_access.write().await;
    let pool = app.pool.clone();

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

    // Resolve relative cache path → absolute for the disk delete.
    let roots = super::settings::load_path_roots(&pool).await?;
    let abs_cache = crate::path_resolve::cache_to_absolute(&cache_path, &roots.storage_path);
    if let Err(e) = std::fs::remove_file(&abs_cache) {
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
