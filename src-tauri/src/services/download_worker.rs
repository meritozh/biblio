//! Background worker that drains the remote-download queue one file at a time.
//!
//! Mirror of `upload_worker`: producer-consumer queue, sequential draining,
//! per-file `remote-download-progress` events. Pulls a remote file's bytes
//! to a local cache directory and records the cache path on the row so the
//! UI can show a "cached locally" badge without a per-render `fs::exists`.
//!
//! The remote copy is left in place — this is "copy back, keep cloud", not
//! "move back" (the inverse of upload would have been move). Users who want
//! the remote copy gone use the Delete action.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::commands::remote::ensure_access_token;
use crate::providers::baidu_netdisk::{download_to, get_download_dlink};

#[derive(Debug, Clone)]
pub struct DownloadJob {
    pub file_id: i64,
}

pub struct DownloadQueueSender(pub UnboundedSender<DownloadJob>);

#[derive(Debug, Clone, Serialize)]
pub struct RemoteDownloadProgressEvent {
    pub file_id: i64,
    pub file_name: String,
    pub status: String,
    pub error: Option<String>,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

pub fn spawn(app: AppHandle) -> UnboundedSender<DownloadJob> {
    let (tx, rx) = unbounded_channel::<DownloadJob>();
    tauri::async_runtime::spawn(run(app, rx));
    tx
}

async fn run(app: AppHandle, mut rx: UnboundedReceiver<DownloadJob>) {
    while let Some(job) = rx.recv().await {
        process_one(&app, job).await;
    }
}

async fn process_one(app: &AppHandle, job: DownloadJob) {
    let DownloadJob { file_id } = job;

    let instances = app.state::<DbInstances>();
    let pool = match get_sqlite_pool(&instances, "sqlite:biblio.db") {
        Ok(p) => p,
        Err(e) => {
            emit(app, file_id, "", "error", Some(e));
            return;
        }
    };

    // Pull the row state we need to compute the cache filename and confirm
    // the file is actually remote. `path` for a remote row is the Baidu
    // path (e.g. `/apps/biblio/<base64>.cbz`); `original_path` is the
    // pre-upload local path, used to recover a friendly basename.
    let row: Option<(String, String, Option<String>, Option<String>, String)> =
        match sqlx::query_as(
            "SELECT path, display_name, original_path, remote_fs_id, \
                    COALESCE(storage_kind, 'local') \
             FROM files WHERE id = ?",
        )
        .bind(file_id)
        .fetch_optional(&pool)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                emit(app, file_id, "", "error", Some(e.to_string()));
                return;
            }
        };

    let Some((remote_path, display_name, original_path, remote_fs_id, storage_kind)) = row else {
        emit(app, file_id, "", "error", Some("File not found".into()));
        return;
    };

    if storage_kind != "remote" {
        emit(
            app,
            file_id,
            &display_name,
            "error",
            Some("File is not in remote storage".into()),
        );
        return;
    }

    let Some(fs_id) = remote_fs_id.filter(|s| !s.is_empty()) else {
        emit(
            app,
            file_id,
            &display_name,
            "error",
            Some("Row missing remote_fs_id; cannot resolve dlink".into()),
        );
        return;
    };

    // Storage path lives in app_settings. The cache directory is a
    // dedicated subfolder so `storage_path` itself stays clean for
    // category-folder layout the rest of the app expects.
    let storage_root = match read_storage_root(&pool).await {
        Ok(p) => p,
        Err(e) => {
            emit(app, file_id, &display_name, "error", Some(e));
            return;
        }
    };
    let cache_dir = storage_root.join(".cache");

    // Cache filename: `<file_id>_<basename>`. The id prefix avoids
    // collisions when two rows share the same original filename, and the
    // visible basename helps users who browse the cache directory in
    // Finder. Falls back to display_name + extension stripped from the
    // remote path if the row never had an original_path.
    let basename = derive_basename(&remote_path, original_path.as_deref(), &display_name);
    let cache_path = cache_dir.join(format!("{file_id}_{basename}"));

    emit(app, file_id, &display_name, "downloading", None);

    let access_token = match ensure_access_token(&pool).await {
        Ok(t) => t,
        Err(e) => {
            emit(app, file_id, &display_name, "error", Some(e));
            return;
        }
    };

    let dlink = match get_download_dlink(&access_token, &fs_id).await {
        Ok(d) => d,
        Err(e) => {
            emit(app, file_id, &display_name, "error", Some(e.0));
            return;
        }
    };

    if let Err(e) = download_to(&access_token, &dlink, &cache_path).await {
        emit(app, file_id, &display_name, "error", Some(e.0));
        return;
    }

    // Store the cache path RELATIVE to storage_root so the user can
    // move the storage folder later without rewriting every row's
    // cache pointer. The resolver in path_resolve::cache_to_absolute
    // rebuilds the full path at read time.
    let cache_path_str = cache_path.to_string_lossy().to_string();
    let storage_root_str = storage_root.to_string_lossy().to_string();
    let stored_cache =
        crate::path_resolve::to_relative_cache(&cache_path_str, &storage_root_str);
    if let Err(e) = sqlx::query("UPDATE files SET local_cache_path = ? WHERE id = ?")
        .bind(&stored_cache)
        .bind(file_id)
        .execute(&pool)
        .await
    {
        emit(
            app,
            file_id,
            &display_name,
            "error",
            Some(format!("DB update failed: {e}")),
        );
        return;
    }

    emit(app, file_id, &display_name, "success", None);
}

async fn read_storage_root(pool: &sqlx::SqlitePool) -> Result<PathBuf, String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT value FROM app_settings WHERE key = 'storage_path'")
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    let s = row
        .map(|r| r.0)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Storage path is not configured".to_string())?;
    Ok(PathBuf::from(s))
}

/// Pick a friendly basename for the cache file. Prefer the basename of the
/// original local path (what the user uploaded), then the visible display
/// name plus the remote path's extension, then a final fallback so we
/// never produce an empty filename.
fn derive_basename(remote_path: &str, original_path: Option<&str>, display_name: &str) -> String {
    if let Some(orig) = original_path {
        if let Some(name) = std::path::Path::new(orig)
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
        {
            return name.to_string();
        }
    }

    let ext = std::path::Path::new(remote_path)
        .extension()
        .and_then(|s| s.to_str());
    let cleaned = display_name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '\0') { '_' } else { c })
        .collect::<String>();
    let cleaned = if cleaned.trim().is_empty() {
        format!("file_{}", chrono::Utc::now().timestamp())
    } else {
        cleaned
    };
    match ext {
        Some(e) if !e.is_empty() => format!("{cleaned}.{e}"),
        _ => cleaned,
    }
}

fn emit(app: &AppHandle, file_id: i64, file_name: &str, status: &str, error: Option<String>) {
    let _ = app.emit(
        "remote-download-progress",
        &RemoteDownloadProgressEvent {
            file_id,
            file_name: file_name.to_string(),
            status: status.to_string(),
            error,
        },
    );
}
