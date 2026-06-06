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
    /// Present only on `error` events. `skip_serializing_if` keeps the JSON
    /// shape honest: success/downloading events omit the field entirely
    /// instead of sending `null`, so the TS type's `error?: string` (no
    /// `null`) holds, and store readers don't silently overwrite to `null`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Absolute path of the newly-written cache file. Present only on the
    /// terminal `success` event — every other status omits it from the JSON.
    /// The frontend patches `files.local_cache_path` with this value so
    /// "Show in Finder" / Open / etc. can resolve to a real fs path without
    /// a refetch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_cache_path: Option<String>,
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
            emit(app, file_id, "", "error", Some(e), None);
            return;
        }
    };

    // Pull the row state we need to compute the cache filename and confirm
    // the file is actually remote. `path` for a remote row is the Baidu
    // path (e.g. `/apps/biblio/<base64>.cbz`); `original_path` is the
    // pre-upload local path, used to recover a friendly basename.
    let row: Option<(String, String, Option<String>, Option<String>, String, Option<String>)> =
        match sqlx::query_as(
            "SELECT path, display_name, original_path, remote_fs_id, \
                    COALESCE(storage_kind, 'local'), remote_container \
             FROM files WHERE id = ?",
        )
        .bind(file_id)
        .fetch_optional(&pool)
        .await
        {
            Ok(r) => r,
            Err(e) => {
                emit(app, file_id, "", "error", Some(e.to_string()), None);
                return;
            }
        };

    let Some((remote_path, display_name, original_path, remote_fs_id, storage_kind, remote_container)) =
        row
    else {
        emit(app, file_id, "", "error", Some("File not found".into()), None);
        return;
    };

    if storage_kind != "remote" {
        emit(
            app,
            file_id,
            &display_name,
            "error",
            Some("File is not in remote storage".into()),
            None,
        );
        return;
    }

    // Multi-part files ('bbx1-split') have no single object/fs_id — their bytes
    // live across the `remote_parts` objects. Dispatch on the stored marker,
    // never on size, so an existing single 'bbx1' object (even one larger than
    // PART_SIZE) keeps the single-object path below.
    if is_multipart(remote_container.as_deref()) {
        download_split(
            app,
            &pool,
            file_id,
            &display_name,
            &remote_path,
            original_path.as_deref(),
        )
        .await;
        return;
    }

    let Some(fs_id) = remote_fs_id.filter(|s| !s.is_empty()) else {
        emit(
            app,
            file_id,
            &display_name,
            "error",
            Some("Row missing remote_fs_id; cannot resolve dlink".into()),
            None,
        );
        return;
    };

    // Storage path lives in app_settings. The cache directory is a
    // dedicated subfolder so `storage_path` itself stays clean for
    // category-folder layout the rest of the app expects.
    let storage_root = match read_storage_root(&pool).await {
        Ok(p) => p,
        Err(e) => {
            emit(app, file_id, &display_name, "error", Some(e), None);
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

    emit(app, file_id, &display_name, "downloading", None, None);

    let access_token = match ensure_access_token(&pool).await {
        Ok(t) => t,
        Err(e) => {
            emit(app, file_id, &display_name, "error", Some(e), None);
            return;
        }
    };

    let dlink = match get_download_dlink(&access_token, &fs_id).await {
        Ok(d) => d,
        Err(e) => {
            emit(app, file_id, &display_name, "error", Some(e.0), None);
            return;
        }
    };

    // Wrapped objects ('bbx1') download to a temp, then unwrap into the real
    // cache path. Legacy rows (remote_container IS NULL) download straight
    // through unchanged.
    let is_wrapped = remote_container.as_deref() == Some("bbx1");
    let download_target = if is_wrapped {
        cache_dir.join(format!(".{file_id}.bbxdl"))
    } else {
        cache_path.clone()
    };

    if let Err(e) = download_to(&access_token, &dlink, &download_target).await {
        emit(app, file_id, &display_name, "error", Some(e.0), None);
        return;
    }

    if is_wrapped {
        let key = match crate::services::container::get_or_create_key(&pool).await {
            Ok(k) => k,
            Err(e) => {
                let _ = std::fs::remove_file(&download_target);
                emit(app, file_id, &display_name, "error", Some(e), None);
                return;
            }
        };
        if let Err(e) = crate::services::container::unwrap(&download_target, &cache_path, &key) {
            let _ = std::fs::remove_file(&download_target);
            emit(
                app,
                file_id,
                &display_name,
                "error",
                Some(format!("container unwrap failed: {e}")),
                None,
            );
            return;
        }
        let _ = std::fs::remove_file(&download_target);
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
            None,
        );
        return;
    }

    // Single source of truth for the event shape: route success through the
    // same emit() helper, with the absolute cache path moved (not cloned)
    // into the Option. A future field on RemoteDownloadProgressEvent only
    // needs the helper updated, not a second inline struct literal.
    emit(
        app,
        file_id,
        &display_name,
        "success",
        None,
        Some(cache_path_str),
    );
}

/// Whether a row's bytes are stored as multiple parts. The download layout is
/// chosen from the stored `remote_container` marker ONLY — never from file size.
/// This is the invariant that keeps an existing single 'bbx1' object readable
/// even when it is larger than `PART_SIZE` (it predates splitting): such a row
/// must take the single-object path, not be wrongly treated as multi-part.
fn is_multipart(remote_container: Option<&str>) -> bool {
    remote_container == Some("bbx1-split")
}

/// Download and reassemble a multi-part ('bbx1-split') file: fetch each part
/// object in order, unwrap-append its plaintext onto the cache file, then verify
/// the reassembled length matches the recorded original size. Any part failure
/// removes the partial cache file and aborts (local_cache_path is only set on
/// full success, so a partial reassembly is never recorded as valid).
async fn download_split(
    app: &AppHandle,
    pool: &sqlx::SqlitePool,
    file_id: i64,
    display_name: &str,
    remote_path: &str,
    original_path: Option<&str>,
) {
    let storage_root = match read_storage_root(pool).await {
        Ok(p) => p,
        Err(e) => {
            emit(app, file_id, display_name, "error", Some(e), None);
            return;
        }
    };
    let cache_dir = storage_root.join(".cache");
    if let Err(e) = std::fs::create_dir_all(&cache_dir) {
        emit(
            app,
            file_id,
            display_name,
            "error",
            Some(format!("Failed to create cache dir: {e}")),
            None,
        );
        return;
    }
    let basename = derive_basename(remote_path, original_path, display_name);
    let cache_path = cache_dir.join(format!("{file_id}_{basename}"));

    // Ordered parts + the expected reassembled size (files.remote_size).
    let parts: Vec<(i64, String, Option<String>)> = match sqlx::query_as(
        "SELECT part_index, object_name, fs_id FROM remote_parts \
         WHERE file_id = ? ORDER BY part_index",
    )
    .bind(file_id)
    .fetch_all(pool)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            emit(app, file_id, display_name, "error", Some(e.to_string()), None);
            return;
        }
    };
    if parts.is_empty() {
        emit(
            app,
            file_id,
            display_name,
            "error",
            Some("Split file has no parts recorded".into()),
            None,
        );
        return;
    }
    let expected_total: Option<i64> =
        match sqlx::query_as::<_, (Option<i64>,)>("SELECT remote_size FROM files WHERE id = ?")
            .bind(file_id)
            .fetch_optional(pool)
            .await
        {
            Ok(r) => r.and_then(|t| t.0),
            Err(e) => {
                emit(app, file_id, display_name, "error", Some(e.to_string()), None);
                return;
            }
        };

    emit(app, file_id, display_name, "downloading", None, None);

    let access_token = match ensure_access_token(pool).await {
        Ok(t) => t,
        Err(e) => {
            emit(app, file_id, display_name, "error", Some(e), None);
            return;
        }
    };
    let key = match crate::services::container::get_or_create_key(pool).await {
        Ok(k) => k,
        Err(e) => {
            emit(app, file_id, display_name, "error", Some(e), None);
            return;
        }
    };

    // Reassemble: each part's plaintext is appended onto the cache file.
    let mut out = match std::fs::File::create(&cache_path) {
        Ok(f) => f,
        Err(e) => {
            emit(
                app,
                file_id,
                display_name,
                "error",
                Some(format!("Failed to create cache file: {e}")),
                None,
            );
            return;
        }
    };

    for (idx, object_name, fs_id) in &parts {
        let Some(fs_id) = fs_id.as_deref().filter(|s| !s.is_empty()) else {
            drop(out);
            let _ = std::fs::remove_file(&cache_path);
            emit(
                app,
                file_id,
                display_name,
                "error",
                Some(format!("Part {idx} ({object_name}) missing fs_id")),
                None,
            );
            return;
        };
        let dlink = match get_download_dlink(&access_token, fs_id).await {
            Ok(d) => d,
            Err(e) => {
                drop(out);
                let _ = std::fs::remove_file(&cache_path);
                emit(app, file_id, display_name, "error", Some(e.0), None);
                return;
            }
        };
        let part_tmp = cache_dir.join(format!(".{file_id}.part{idx}.bbxdl"));
        if let Err(e) = download_to(&access_token, &dlink, &part_tmp).await {
            drop(out);
            let _ = std::fs::remove_file(&cache_path);
            emit(app, file_id, display_name, "error", Some(e.0), None);
            return;
        }
        if let Err(e) = crate::services::container::unwrap_append(&part_tmp, &mut out, &key) {
            let _ = std::fs::remove_file(&part_tmp);
            drop(out);
            let _ = std::fs::remove_file(&cache_path);
            emit(
                app,
                file_id,
                display_name,
                "error",
                Some(format!("Part {idx} unwrap failed: {e}")),
                None,
            );
            return;
        }
        let _ = std::fs::remove_file(&part_tmp);
    }
    drop(out);

    // Reassembled length must equal the recorded original size — guards against
    // a silently-truncated part or a missing/misordered chunk.
    if let Some(expected) = expected_total {
        match std::fs::metadata(&cache_path) {
            Ok(m) if m.len() == expected as u64 => {}
            Ok(m) => {
                let _ = std::fs::remove_file(&cache_path);
                emit(
                    app,
                    file_id,
                    display_name,
                    "error",
                    Some(format!(
                        "Reassembled size {} != expected {expected}",
                        m.len()
                    )),
                    None,
                );
                return;
            }
            Err(e) => {
                emit(app, file_id, display_name, "error", Some(e.to_string()), None);
                return;
            }
        }
    }

    let cache_path_str = cache_path.to_string_lossy().to_string();
    let storage_root_str = storage_root.to_string_lossy().to_string();
    let stored_cache = crate::path_resolve::to_relative_cache(&cache_path_str, &storage_root_str);
    if let Err(e) = sqlx::query("UPDATE files SET local_cache_path = ? WHERE id = ?")
        .bind(&stored_cache)
        .bind(file_id)
        .execute(pool)
        .await
    {
        emit(
            app,
            file_id,
            display_name,
            "error",
            Some(format!("DB update failed: {e}")),
            None,
        );
        return;
    }

    emit(
        app,
        file_id,
        display_name,
        "success",
        None,
        Some(cache_path_str),
    );
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

/// Build and emit a `remote-download-progress` event. The single struct
/// literal in this helper is the only place the event shape lives — every
/// caller (downloading / error / success) routes through it so a new field
/// can never drift between the terminal-success path and the rest.
///
/// `local_cache_path` is `Some(...)` only on the `success` event; the struct
/// uses `skip_serializing_if` so non-success events omit the field on the
/// wire (TS handler sees `undefined`, not `null`).
fn emit(
    app: &AppHandle,
    file_id: i64,
    file_name: &str,
    status: &str,
    error: Option<String>,
    local_cache_path: Option<String>,
) {
    let _ = app.emit(
        "remote-download-progress",
        &RemoteDownloadProgressEvent {
            file_id,
            file_name: file_name.to_string(),
            status: status.to_string(),
            error,
            local_cache_path,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_dispatch_is_by_marker_not_size() {
        // Only the explicit split marker takes the multi-part path.
        assert!(is_multipart(Some("bbx1-split")));
        // A single encrypted object stays single REGARDLESS of size — this is
        // the backward-compat guarantee for >10GB objects uploaded before
        // splitting existed.
        assert!(!is_multipart(Some("bbx1")));
        // Legacy raw rows pass through unchanged.
        assert!(!is_multipart(None));
        // Unknown future markers don't accidentally trigger reassembly.
        assert!(!is_multipart(Some("bbx2")));
    }
}
