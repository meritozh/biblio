//! Background worker that drains the file-delete queue one file at a time.
//!
//! Used by the bulk Delete action in the UI. Single-row deletes still go
//! through the existing `file_delete` command (best-effort cloud delete);
//! this worker exists for the multi-select case so the user gets per-file
//! progress and explicit error events instead of a swallowed log line.
//!
//! For remote files the cloud delete is *strict* — failure aborts the row's
//! deletion entirely so the user can retry. For local files we still
//! remove the on-disk copy first; if that fails we surface the error and
//! leave the DB row in place. Either way: only after every external side
//! effect succeeds do we actually drop the row.

use crate::Services;
use serde::Serialize;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::commands::remote::delete_on_remote_for_file;

#[derive(Debug, Clone)]
pub struct DeleteJob {
    pub file_id: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteDeleteProgressEvent {
    pub file_id: i64,
    pub file_name: String,
    pub status: String,
    pub error: Option<String>,
}

pub fn spawn(app: Services) -> UnboundedSender<DeleteJob> {
    let (tx, rx) = unbounded_channel::<DeleteJob>();
    tokio::spawn(run(app, rx));
    tx
}

async fn run(app: Services, mut rx: UnboundedReceiver<DeleteJob>) {
    while let Some(job) = rx.recv().await {
        process_one(&app, job).await;
    }
}

async fn process_one(app: &Services, job: DeleteJob) {
    let DeleteJob { file_id } = job;

    let pool = app.pool.clone();

    let row: Option<(String, String, bool, Option<String>, String)> = match sqlx::query_as(
        "SELECT path, display_name, in_storage, local_cache_path, \
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

    let Some((stored_path, display_name, in_storage, local_cache_path, storage_kind)) = row else {
        // Row already gone — treat as success so a duplicate-enqueue
        // doesn't surface a confusing error.
        emit(app, file_id, "", "success", None);
        return;
    };

    // Resolve stored relative paths → absolute for the external op
    // (Baidu API for remote, fs::remove_file for local).
    let roots = match crate::commands::settings::load_path_roots(&pool).await {
        Ok(r) => r,
        Err(e) => {
            emit(app, file_id, &display_name, "error", Some(e));
            return;
        }
    };
    let path = crate::path_resolve::to_absolute(
        &storage_kind, &stored_path, &roots.storage_path, &roots.app_root,
    )
    .to_string_lossy()
    .to_string();

    emit(app, file_id, &display_name, "deleting", None);

    if storage_kind == "remote" {
        if let Err(e) = delete_on_remote_for_file(&pool, file_id, &path).await {
            emit(app, file_id, &display_name, "error", Some(e));
            return;
        }
    } else if in_storage {
        if let Err(e) = std::fs::remove_file(&path) {
            // "Already missing on disk" satisfies the invariant — the
            // external state is what we wanted, so proceed to the row
            // DELETE. Anything else (permission denied, file locked, IO
            // error) surfaces and leaves the row so the user can retry.
            if e.kind() != std::io::ErrorKind::NotFound {
                emit(
                    app,
                    file_id,
                    &display_name,
                    "error",
                    Some(format!("Failed to remove local file: {e}")),
                );
                return;
            }
        }
    }

    if let Some(cache) = local_cache_path.filter(|s| !s.is_empty()) {
        // Best-effort: a stale cache file is worth a log line but
        // shouldn't block the DB delete the user just confirmed.
        let abs_cache = crate::path_resolve::cache_to_absolute(&cache, &roots.storage_path);
        if let Err(e) = std::fs::remove_file(&abs_cache) {
            eprintln!(
                "Cache cleanup failed for file {file_id} ({}): {e}",
                abs_cache.display()
            );
        }
    }

    if let Err(e) = sqlx::query("DELETE FROM files WHERE id = ?")
        .bind(file_id)
        .execute(&pool)
        .await
    {
        emit(
            app,
            file_id,
            &display_name,
            "error",
            Some(format!("DB delete failed: {e}")),
        );
        return;
    }

    emit(app, file_id, &display_name, "success", None);
}

fn emit(app: &Services, file_id: i64, file_name: &str, status: &str, error: Option<String>) {
    let _ = app.emit(
        "remote-delete-progress",
        &RemoteDeleteProgressEvent {
            file_id,
            file_name: file_name.to_string(),
            status: status.to_string(),
            error,
        },
    );
}
