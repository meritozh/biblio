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

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::commands::remote::{load_config, upload_to_remote};
use crate::providers::baidu_netdisk::UploadResult;
use crate::services::container;

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

    emit(app, file_id, &display_name, "uploading", None);

    // Wrap the file in the opaque encrypted container and upload it under an
    // extension-less random name, then record the 'bbx1' marker so the
    // download path knows to unwrap it. Plaintext never leaves the device;
    // only ciphertext is hashed for Baidu's block_list.
    let (relative_path, upload) =
        match wrap_and_upload(&pool, &remote_cfg.app_root, &source_path, file_id).await {
            Ok(v) => v,
            Err(e) => {
                emit(app, file_id, &display_name, "error", Some(e));
                return;
            }
        };

    if let Err(e) = sqlx::query(
        "UPDATE files SET \
         path = ?, storage_kind = 'remote', remote_provider = 'baidu_netdisk', \
         remote_fs_id = ?, remote_md5 = ?, remote_size = ?, remote_container = 'bbx1', \
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

/// Wrap `source_path` in the encrypted container and upload it under an
/// opaque, extension-less name. Shared by this worker and the re-encrypt
/// worker so both produce byte-identical container objects.
///
/// Returns the stored relative path (a random token) and Baidu's upload
/// result. Because only the ciphertext is hashed, 秒传 never matches and the
/// remote object carries no recognizable name, extension, or format.
pub(crate) async fn wrap_and_upload(
    pool: &sqlx::SqlitePool,
    app_root: &str,
    source_path: &std::path::Path,
    exclude_file_id: i64,
) -> Result<(String, UploadResult), String> {
    let key = container::get_or_create_key(pool).await?;
    let app_root = app_root.trim_end_matches('/');

    // Opaque, extension-less remote name. A 128-bit token collision is
    // astronomically unlikely, but the existing de-dup invariant is cheap.
    let mut relative_path = container::random_token();
    loop {
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM files WHERE path = ? AND id != ?")
                .bind(&relative_path)
                .bind(exclude_file_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;
        if existing.is_none() {
            break;
        }
        relative_path = container::random_token();
    }
    let remote_path = format!("{app_root}/{relative_path}");

    let tmp = std::env::temp_dir().join(format!("biblio-wrap-{}.bbx", container::random_token()));
    container::wrap(source_path, &tmp, &key).map_err(|e| format!("container wrap failed: {e}"))?;
    let result = upload_to_remote(pool, &tmp, &remote_path).await;
    let _ = std::fs::remove_file(&tmp);
    let upload = result?;
    Ok((relative_path, upload))
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
