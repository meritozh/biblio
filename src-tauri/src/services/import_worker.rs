//! Background worker that drains the import-analysis queue one path at a time.
//!
//! Each `ImportJob` carries a single path plus the folder-derived author
//! candidates already resolved by the enqueue command. The worker hands the
//! job to `commands::processing::process_import_path`, which builds the env,
//! runs the per-kind pipeline, and emits `processing-progress` /
//! `file-prepared` events along the way.
//!
//! `cancel_processing` flips the shared `ProcessingCancelled` flag; the
//! worker checks it before each job and skips remaining work, draining the
//! channel without touching the DB.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::ProcessingCancelled;
use crate::commands::processing::process_import_path;

/// One queued import. Authored by the enqueue command, which derives the
/// folder-root author candidates up-front so the worker doesn't need to
/// reconstruct cross-path batch context.
#[derive(Debug, Clone)]
pub struct ImportJob {
    pub path: PathBuf,
    pub parent_authors: Vec<String>,
    /// Cancellation generation claimed by the `enqueue_import` that created
    /// this job. The worker skips the job iff this generation was cancelled,
    /// so a cancel of an earlier batch can't drop a later batch's jobs.
    pub generation: u64,
}

pub struct ImportQueueSender(pub UnboundedSender<ImportJob>);

pub fn spawn(app: AppHandle) -> UnboundedSender<ImportJob> {
    let (tx, rx) = unbounded_channel::<ImportJob>();
    tauri::async_runtime::spawn(run(app, rx));
    tx
}

async fn run(app: AppHandle, mut rx: UnboundedReceiver<ImportJob>) {
    while let Some(job) = rx.recv().await {
        // Honor cancel by draining without processing — but only for the
        // generation that was actually cancelled. A batch enqueued after a
        // cancel claims a higher generation and runs normally.
        let cancelled = app
            .state::<ProcessingCancelled>()
            .0
            .is_cancelled(job.generation);
        if cancelled {
            continue;
        }

        if let Err(e) =
            process_import_path(&app, job.path, job.parent_authors, job.generation).await
        {
            eprintln!("import worker: process_import_path failed: {e}");
        }
    }
}
