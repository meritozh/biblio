//! Two-stage pipeline that re-encrypts files already sitting on Baidu in
//! raw, hash-matchable form (rows with `remote_container IS NULL`, uploaded
//! before the container feature existed).
//!
//! Per file: download the raw object → wrap it in the encrypted container →
//! upload the opaque copy → delete the old raw object → flip the row to
//! 'bbx1'. Baidu can't transform bytes server-side, so this is a full
//! round-trip.
//!
//! Pipeline: a bounded `mpsc(PIPELINE_DEPTH)` decouples the download leg
//! from the encrypt+upload leg so the two run concurrently — on an
//! asymmetric home link the downloads hide under the uploads. The bound
//! also caps how many raw plaintext temps sit on disk at once: the
//! downloader's `.send().await` blocks once the buffer is full
//! (backpressure).
//!
//! Ordering invariant — **upload-new → delete-old → update-DB**. The raw
//! object is never deleted until its encrypted replacement is confirmed up,
//! so no failure can destroy the only copy. A failed delete leaves the row
//! legacy and re-runnable (the raw object is retried), with at most an
//! *encrypted* orphan on Baidu — never a raw one.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::commands::remote::{delete_on_remote, ensure_access_token, load_config};
use crate::providers::baidu_netdisk::{download_to, get_download_dlink};
use crate::services::container;
use crate::services::upload_worker::wrap_and_upload;

/// Max raw plaintext temps buffered between the download and upload legs.
const PIPELINE_DEPTH: usize = 5;

#[derive(Debug, Clone)]
pub struct ReencryptJob {
    pub file_id: i64,
}

pub struct ReencryptQueueSender(pub UnboundedSender<ReencryptJob>);

#[derive(Debug, Clone, Serialize)]
pub struct RemoteReencryptProgressEvent {
    pub file_id: i64,
    pub file_name: String,
    pub status: String,
    pub error: Option<String>,
}

/// Item handed from the download leg to the encrypt+upload leg. Owning the
/// `raw_temp` path transfers cleanup responsibility to the consumer.
struct Downloaded {
    file_id: i64,
    display_name: String,
    raw_temp: PathBuf,
    /// Absolute Baidu path of the raw object to delete after re-upload.
    old_remote_path: String,
    app_root: String,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

/// Spawn both pipeline legs joined by a bounded channel and return the entry
/// sender (unbounded, so enqueueing a large backfill never blocks a command).
pub fn spawn(app: AppHandle) -> UnboundedSender<ReencryptJob> {
    let (entry_tx, entry_rx) = unbounded_channel::<ReencryptJob>();
    let (mid_tx, mid_rx) = tokio::sync::mpsc::channel::<Downloaded>(PIPELINE_DEPTH);
    tauri::async_runtime::spawn(download_leg(app.clone(), entry_rx, mid_tx));
    tauri::async_runtime::spawn(upload_leg(app, mid_rx));
    entry_tx
}

async fn download_leg(
    app: AppHandle,
    mut entry_rx: UnboundedReceiver<ReencryptJob>,
    mid_tx: tokio::sync::mpsc::Sender<Downloaded>,
) {
    while let Some(job) = entry_rx.recv().await {
        match download_one(&app, job.file_id).await {
            // `.send().await` blocks here while PIPELINE_DEPTH items are
            // already buffered — this is the disk/throughput backpressure.
            Ok(Some(item)) => {
                if mid_tx.send(item).await.is_err() {
                    break; // upload leg gone; nothing left to do
                }
            }
            // Ineligible (already encrypted / no longer remote). Emit a
            // terminal `skipped` event so the frontend progress counter —
            // which tallies terminal events against the queued total — can
            // still reach completion instead of spinning forever.
            Ok(None) => emit(&app, job.file_id, "", "skipped", None),
            Err((name, err)) => emit(&app, job.file_id, &name, "error", Some(err)),
        }
    }
}

/// Download leg: validate eligibility and pull the raw bytes to a temp.
/// Returns `Ok(None)` for rows that aren't legacy-remote (keeps a bulk
/// "encrypt all" enqueue idempotent). Errors carry the display name so the
/// caller can emit a useful event.
async fn download_one(
    app: &AppHandle,
    file_id: i64,
) -> Result<Option<Downloaded>, (String, String)> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db").map_err(|e| (String::new(), e))?;

    let row: Option<(String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT path, display_name, remote_fs_id, COALESCE(storage_kind, 'local'), \
                remote_container \
         FROM files WHERE id = ?",
    )
    .bind(file_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| (String::new(), e.to_string()))?;

    let Some((path, display_name, remote_fs_id, storage_kind, remote_container)) = row else {
        return Err((String::new(), "File not found".into()));
    };

    // Only legacy remote rows are eligible; everything else is skipped.
    if storage_kind != "remote" || remote_container.is_some() {
        return Ok(None);
    }
    let Some(fs_id) = remote_fs_id.filter(|s| !s.is_empty()) else {
        return Err((display_name, "Row missing remote_fs_id".into()));
    };

    let cfg = load_config(&pool).await;
    // Build the raw object's absolute Baidu path through the same resolver
    // the delete worker uses, so delete-old targets exactly the right path
    // (handles leading slashes and absolute-passthrough rows). A hand-built
    // `{app_root}/{path}` would mis-target a row stored with an absolute
    // path, making that file impossible to re-encrypt. storage_path is
    // unused for the remote branch.
    let old_remote_path = crate::path_resolve::to_absolute("remote", &path, "", &cfg.app_root)
        .to_string_lossy()
        .to_string();

    emit(app, file_id, &display_name, "downloading", None);

    let access_token = ensure_access_token(&pool)
        .await
        .map_err(|e| (display_name.clone(), e))?;
    let dlink = get_download_dlink(&access_token, &fs_id)
        .await
        .map_err(|e| (display_name.clone(), e.0))?;

    let raw_temp =
        std::env::temp_dir().join(format!("biblio-reenc-{}.raw", container::random_token()));
    download_to(&access_token, &dlink, &raw_temp)
        .await
        .map_err(|e| (display_name.clone(), e.0))?;

    Ok(Some(Downloaded {
        file_id,
        display_name,
        raw_temp,
        old_remote_path,
        app_root: cfg.app_root,
    }))
}

async fn upload_leg(app: AppHandle, mut mid_rx: tokio::sync::mpsc::Receiver<Downloaded>) {
    while let Some(item) = mid_rx.recv().await {
        reencrypt_one(&app, item).await;
    }
}

/// Encrypt+upload leg: wrap the raw temp, upload it, delete the old raw
/// object, then flip the row. Owns and always cleans up `raw_temp`.
async fn reencrypt_one(app: &AppHandle, item: Downloaded) {
    let Downloaded {
        file_id,
        display_name,
        raw_temp,
        old_remote_path,
        app_root,
    } = item;

    let instances = app.state::<DbInstances>();
    let pool = match get_sqlite_pool(&instances, "sqlite:biblio.db") {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::remove_file(&raw_temp);
            emit(app, file_id, &display_name, "error", Some(e));
            return;
        }
    };

    emit(app, file_id, &display_name, "uploading", None);

    // 1. Upload the encrypted replacement FIRST. If this fails the raw copy
    //    on Baidu is untouched and the row stays legacy (re-runnable).
    // Re-encryption is a background backfill — no per-slice progress UI, so a
    // no-op progress sink.
    let (relative_path, upload) =
        match wrap_and_upload(&pool, &app_root, &raw_temp, file_id, |_, _, _| {}).await
    {
        Ok(v) => v,
        Err(e) => {
            let _ = std::fs::remove_file(&raw_temp);
            emit(app, file_id, &display_name, "error", Some(e));
            return;
        }
    };
    let _ = std::fs::remove_file(&raw_temp);

    // 2. Flip the row onto the NEW encrypted object BEFORE deleting the old
    //    raw one. If this UPDATE fails, the row still points at the intact raw
    //    object (downloadable, remote_container still NULL → re-runnable) and
    //    the new encrypted copy is a harmless orphan — never a row pointing at
    //    a deleted object. This ordering keeps a live reference at every step.
    //    original_path / display_name are untouched, so the real filename is
    //    still recovered on download.
    if let Err(e) = sqlx::query(
        "UPDATE files SET \
         path = ?, remote_fs_id = ?, remote_md5 = ?, remote_size = ?, remote_container = 'bbx1' \
         WHERE id = ?",
    )
    .bind(&relative_path)
    .bind(&upload.fs_id)
    .bind(&upload.md5)
    .bind(upload.size)
    .bind(file_id)
    .execute(&pool)
    .await
    {
        emit(
            app,
            file_id,
            &display_name,
            "error",
            Some(format!("DB update failed (raw original kept; will retry): {e}")),
        );
        return;
    }

    // 3. Delete the old raw object, best-effort. The row already points at the
    //    encrypted copy, so a failed delete only leaves a harmless raw orphan
    //    on Baidu (the row is no longer eligible for re-encryption) — never
    //    data loss. Log it so the orphan can be reclaimed.
    if let Err(e) = delete_on_remote(&pool, &old_remote_path).await {
        eprintln!(
            "Re-encrypt: row {file_id} flipped to encrypted, but deleting the raw \
             original at {old_remote_path} failed (orphan left on remote): {e}"
        );
    }

    emit(app, file_id, &display_name, "success", None);
}

fn emit(app: &AppHandle, file_id: i64, file_name: &str, status: &str, error: Option<String>) {
    let _ = app.emit(
        "remote-reencrypt-progress",
        &RemoteReencryptProgressEvent {
            file_id,
            file_name: file_name.to_string(),
            status: status.to_string(),
            error,
        },
    );
}
