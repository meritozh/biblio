//! Background worker that drains the remote-upload queue one file at a time.
//!
//! Producer-consumer model: `enqueue_remote_upload` pushes file IDs to the
//! channel and returns immediately; this worker processes them serially in
//! the order received and emits `remote-upload-progress` events that the
//! frontend's RemoteUploadProgress panel listens to.
//!
//! Sequential by design — uploading more than one file in parallel would
//! saturate user bandwidth and risk Baidu Pan rate limits. If parallelism
//! is ever wanted, swap the `while let` recv loop for a bounded JoinSet.

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::commands::remote::{load_config, upload_to_remote};

/// One queued upload. Carries only what the worker needs to identify the
/// file; everything else is fetched from the DB at processing time so the
/// queue doesn't pin stale row data.
#[derive(Debug, Clone)]
pub struct UploadJob {
    pub file_id: i64,
}

/// Wrapper stored in Tauri state. Cloning the sender is cheap and lets
/// every Tauri command push to the same channel.
pub struct UploadQueueSender(pub UnboundedSender<UploadJob>);

/// Per-event payload mirrored from the previous batch-mode command. Frontend
/// listeners (`onRemoteUploadProgress`) depend on this exact shape.
#[derive(Debug, Clone, Serialize)]
pub struct RemoteUploadProgressEvent {
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

/// Spawn the worker on Tauri's async runtime and return the sender end of
/// the channel so callers (`lib.rs::run`) can stash it in app state.
pub fn spawn(app: AppHandle) -> UnboundedSender<UploadJob> {
    let (tx, rx) = unbounded_channel::<UploadJob>();
    tauri::async_runtime::spawn(run(app, rx));
    tx
}

async fn run(app: AppHandle, mut rx: UnboundedReceiver<UploadJob>) {
    while let Some(job) = rx.recv().await {
        process_one(&app, job).await;
    }
}

async fn process_one(app: &AppHandle, job: UploadJob) {
    let UploadJob { file_id } = job;

    let instances = app.state::<DbInstances>();
    let pool = match get_sqlite_pool(&instances, "sqlite:biblio.db") {
        Ok(p) => p,
        Err(e) => {
            emit(app, file_id, "", "error", Some(e));
            return;
        }
    };

    let remote_cfg = load_config(&pool).await;
    if !remote_cfg.enabled {
        emit(
            app,
            file_id,
            "",
            "error",
            Some("REMOTE_STORAGE_NOT_CONFIGURED".to_string()),
        );
        return;
    }

    let row: Option<(String, String, Option<String>, String)> = match sqlx::query_as(
        "SELECT path, display_name, progress, COALESCE(storage_kind, 'local') FROM files WHERE id = ?",
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

    let Some((local_path, display_name, _progress, storage_kind)) = row else {
        emit(app, file_id, "", "error", Some("File not found".into()));
        return;
    };

    if storage_kind != "local" {
        emit(
            app,
            file_id,
            &display_name,
            "error",
            Some("File is not local storage".into()),
        );
        return;
    }

    // `files.path` is stored relative to `storage_path`, so resolve to
    // absolute before any filesystem op — otherwise every row fails the
    // existence check below. `storage_kind` is always "local" here
    // (gated above), but pass it through for correctness.
    let roots = match crate::commands::settings::load_path_roots(&pool).await {
        Ok(r) => r,
        Err(e) => {
            emit(app, file_id, &display_name, "error", Some(e));
            return;
        }
    };
    let source_path = crate::path_resolve::to_absolute(
        &storage_kind,
        &local_path,
        &roots.storage_path,
        &roots.app_root,
    );
    if !source_path.exists() {
        emit(
            app,
            file_id,
            &display_name,
            "error",
            Some("Local file not found".into()),
        );
        return;
    }

    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = source_path.extension().and_then(|e| e.to_str());
    let encoded_stem = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(stem.as_bytes());
    let encoded_filename = match ext {
        Some(e) if !e.is_empty() => format!("{encoded_stem}.{e}"),
        _ => encoded_stem.clone(),
    };

    let app_root = remote_cfg.app_root.trim_end_matches('/');
    // Two views of the remote path: the absolute form Baidu's API needs,
    // and the relative form we store in the DB (so `app_root` can be
    // changed later without rewriting every row). `relative_path` is
    // everything after the `app_root/` prefix.
    let mut relative_path = encoded_filename.clone();
    let mut remote_path = format!("{app_root}/{relative_path}");

    let mut counter = 1u32;
    loop {
        // De-dup check against stored RELATIVE paths — the column holds
        // paths relative to the storage root.
        let existing: Result<Option<(i64,)>, _> =
            sqlx::query_as("SELECT id FROM files WHERE path = ? AND id != ?")
                .bind(&relative_path)
                .bind(file_id)
                .fetch_optional(&pool)
                .await;

        match existing {
            Ok(None) => break,
            Ok(Some(_)) => {
                relative_path = match ext {
                    Some(e) if !e.is_empty() => {
                        format!("{encoded_stem}_{counter}.{e}")
                    }
                    _ => format!("{encoded_stem}_{counter}"),
                };
                remote_path = format!("{app_root}/{relative_path}");
                counter += 1;
            }
            Err(e) => {
                emit(
                    app,
                    file_id,
                    &display_name,
                    "error",
                    Some(e.to_string()),
                );
                return;
            }
        }
    }

    emit(app, file_id, &display_name, "uploading", None);

    match upload_to_remote(&pool, &source_path, &remote_path).await {
        Ok(upload) => {
            if let Err(e) = sqlx::query(
                "UPDATE files SET \
                 path = ?, storage_kind = 'remote', remote_provider = 'baidu_netdisk', \
                 remote_fs_id = ?, remote_md5 = ?, remote_size = ?, \
                 in_storage = 0, original_path = ?, file_status = 'available' \
                 WHERE id = ?",
            )
            .bind(&relative_path)
            .bind(&upload.fs_id)
            .bind(&upload.md5)
            .bind(upload.size)
            .bind(&local_path)
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

            if let Err(e) = std::fs::remove_file(&source_path) {
                eprintln!(
                    "Remote upload succeeded but local delete failed ({}): {e}",
                    source_path.display()
                );
            }

            emit(app, file_id, &display_name, "success", None);
        }
        Err(e) => {
            emit(app, file_id, &display_name, "error", Some(e));
        }
    }
}

fn emit(app: &AppHandle, file_id: i64, file_name: &str, status: &str, error: Option<String>) {
    let _ = app.emit(
        "remote-upload-progress",
        &RemoteUploadProgressEvent {
            file_id,
            file_name: file_name.to_string(),
            status: status.to_string(),
            error,
        },
    );
}
