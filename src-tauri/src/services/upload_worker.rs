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

/// Filename prefix for the per-upload encrypted temp container, shared between
/// temp creation and the startup sweep so the two can't drift.
const WRAP_TEMP_PREFIX: &str = "biblio-wrap-";

/// Plaintext bytes per part when an upload artifact is too large for a single
/// remote object. Baidu's per-object cap is 20 GB (SVIP); a file larger than
/// this is split into N parts, each encrypted and uploaded as its own object,
/// then reassembled on download. 10 GiB keeps every part well under the cap
/// (negligible AEAD overhead) and the temp footprint to one part at a time.
/// This is both the split threshold and the chunk size: files <= PART_SIZE are
/// a single 'bbx1' object (unchanged); larger ones become 'bbx1-split'.
const PART_SIZE: u64 = 10 * 1024 * 1024 * 1024;

/// One uploaded part of a split file, recorded into `remote_parts`.
pub(crate) struct UploadedPart {
    pub index: i64,
    pub object_name: String,
    pub fs_id: String,
    pub md5: String,
    pub ciphertext_size: i64,
    pub plaintext_size: i64,
}

/// Result of [`wrap_and_upload`]: a file small enough for one remote object, or
/// one split across N encrypted parts.
pub(crate) enum WrapUploadOutcome {
    Single {
        /// Opaque token stored as `files.path`; also the remote object name.
        relative_path: String,
        upload: UploadResult,
    },
    Split {
        /// Opaque token stored as `files.path` — a unique placeholder only; the
        /// real objects live in `remote_parts`. There is no single object here.
        placeholder_path: String,
        /// Original (plaintext) artifact size; stored as `files.remote_size`.
        total_plaintext: i64,
        parts: Vec<UploadedPart>,
    },
}

/// Delete leftover multi-GB remote-operation temps (`biblio-wrap-*.bbx` from the
/// upload pipeline, `biblio-reenc-*.raw` from reencrypt) in the temp dir. Each is
/// removed on the normal return path, but a hard kill (force-quit / crash) during
/// the multi-minute operation on a large file orphans a full-size container —
/// these accumulate and eat the disk. Called once at startup: no operation is in
/// flight then and the app is single-instance, so every match is provably stale.
pub(crate) fn sweep_stale_upload_temps() {
    let dir = std::env::temp_dir();
    let removed = sweep_dir(&dir);
    if removed > 0 {
        eprintln!(
            "[upload_worker] swept {removed} stale upload temp(s) from {}",
            dir.display()
        );
    }
}

/// (filename prefix, extension) for every multi-GB temp a remote operation
/// leaves in the shared temp dir. Each is removed on the normal return path but
/// orphaned by a hard kill mid-operation; the startup sweep reclaims them.
const STALE_TEMP_PATTERNS: &[(&str, &str)] = &[
    (WRAP_TEMP_PREFIX, ".bbx"), // upload: encrypted container (upload_worker)
    ("biblio-reenc-", ".raw"),  // reencrypt: downloaded raw original (reencrypt_worker)
];

/// Remove our temp files directly inside `dir`; returns the count deleted.
/// Matches strictly on a known prefix + extension so it can never touch an
/// unrelated file in the shared temp directory.
fn sweep_dir(dir: &std::path::Path) -> u32 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0u32;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let ours = STALE_TEMP_PATTERNS
            .iter()
            .any(|(prefix, ext)| name.starts_with(prefix) && name.ends_with(ext));
        if ours && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

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
    /// Bytes done so far / total, populated on progress ticks. `None` on
    /// status-only events (pending, error). The frontend derives percent +
    /// speed from successive ticks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uploaded_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<i64>,
    /// Which long phase the byte counts belong to: "encrypting", "hashing", or
    /// "uploading". `None` on status-only events. Drives the panel's row label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
}

async fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    // Park on writer contention via the async read guard instead of dropping
    // the job with `try_read`, which fails transiently while a writer holds it.
    let instances_lock = instances.0.read().await;
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
    let pool = match get_sqlite_pool(&instances, "sqlite:biblio.db").await {
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
    //
    // Progress callback throttled to ~4 events/sec within a phase: a 10 GB
    // file is ~2,600 slices/blocks per phase, and emitting each would flood the
    // webview event bus. Always emit on a phase change so the bar's reset to 0%
    // for the next phase (Encrypting → Hashing → Uploading) isn't throttled
    // away. The terminal `success`/`error` status `emit` follows separately.
    let progress_app = app.clone();
    let progress_name = display_name.clone();
    let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let mut last_phase: Option<Phase> = None;
    let on_progress = move |phase: Phase, done: i64, total: i64| {
        let now = std::time::Instant::now();
        let phase_changed = last_phase != Some(phase);
        if phase_changed || now.duration_since(last_emit) >= std::time::Duration::from_millis(250) {
            last_emit = now;
            last_phase = Some(phase);
            emit_progress(&progress_app, file_id, &progress_name, phase, done, total);
        }
    };
    let outcome =
        match wrap_and_upload(&pool, &remote_cfg.app_root, &source_path, file_id, on_progress)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                emit(app, file_id, &display_name, "error", Some(e));
                return;
            }
        };

    if let Err(e) = commit_upload_outcome(&pool, file_id, &local_path, &outcome).await {
        // The encrypted object(s) are already uploaded but the row never flipped
        // to 'remote', so they're now orphaned. Best-effort delete them so we
        // don't leak dangling remote objects.
        delete_outcome_objects(&pool, remote_cfg.app_root.trim_end_matches('/'), &outcome).await;
        eprintln!("Remote upload orphaned (status update failed) for file {file_id}");

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

/// Flip a file row onto its freshly-uploaded remote object(s). Single → one
/// 'bbx1' object; Split → a 'bbx1-split' row (no single fs_id/md5) plus the
/// `remote_parts` rows. `original_local_path` is recorded as `original_path`
/// so the real filename survives for download.
async fn commit_upload_outcome(
    pool: &sqlx::SqlitePool,
    file_id: i64,
    original_local_path: &str,
    outcome: &WrapUploadOutcome,
) -> Result<(), String> {
    match outcome {
        WrapUploadOutcome::Single {
            relative_path,
            upload,
        } => {
            sqlx::query(
                "UPDATE files SET \
                 path = ?, storage_kind = 'remote', remote_provider = 'baidu_netdisk', \
                 remote_fs_id = ?, remote_md5 = ?, remote_size = ?, remote_container = 'bbx1', \
                 in_storage = 0, original_path = ?, file_status = 'available' \
                 WHERE id = ?",
            )
            .bind(relative_path)
            .bind(&upload.fs_id)
            .bind(&upload.md5)
            .bind(upload.size)
            .bind(original_local_path)
            .bind(file_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        WrapUploadOutcome::Split {
            placeholder_path,
            total_plaintext,
            parts,
        } => {
            // No single object: remote_fs_id/remote_md5 are NULL; remote_size is
            // the original plaintext size; per-object data lives in remote_parts.
            sqlx::query(
                "UPDATE files SET \
                 path = ?, storage_kind = 'remote', remote_provider = 'baidu_netdisk', \
                 remote_fs_id = NULL, remote_md5 = NULL, remote_size = ?, \
                 remote_container = 'bbx1-split', in_storage = 0, original_path = ?, \
                 file_status = 'available' \
                 WHERE id = ?",
            )
            .bind(placeholder_path)
            .bind(total_plaintext)
            .bind(original_local_path)
            .bind(file_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
            persist_remote_parts(pool, file_id, parts).await?;
        }
    }
    Ok(())
}

/// Best-effort delete of an upload's remote object(s) — used when the row never
/// flipped to 'remote' so nothing is left dangling. One object for Single, all
/// part objects for Split.
async fn delete_outcome_objects(
    pool: &sqlx::SqlitePool,
    app_root: &str,
    outcome: &WrapUploadOutcome,
) {
    match outcome {
        WrapUploadOutcome::Single { relative_path, .. } => {
            let remote_path = format!("{app_root}/{relative_path}");
            let _ = crate::commands::remote::delete_on_remote(pool, &remote_path).await;
        }
        WrapUploadOutcome::Split { parts, .. } => {
            cleanup_part_objects(pool, app_root, parts).await;
        }
    }
}

/// Wrap `source_path` in the encrypted container and upload it under an
/// opaque, extension-less name. Shared by this worker and the re-encrypt
/// worker so both produce byte-identical container objects.
///
/// Returns the stored relative path (a random token) and Baidu's upload
/// result. Because only the ciphertext is hashed, 秒传 never matches and the
/// remote object carries no recognizable name, extension, or format.
/// The long-running phases of a remote upload, reported to the progress
/// callback so the UI can label the bar. `Encrypting` and `Hashing` each read
/// the whole multi-GB file and cost minutes; `Uploading` is the slice POSTs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Encrypting,
    Hashing,
    Uploading,
}

pub(crate) async fn wrap_and_upload(
    pool: &sqlx::SqlitePool,
    app_root: &str,
    source_path: &std::path::Path,
    exclude_file_id: i64,
    mut on_progress: impl FnMut(Phase, i64, i64),
) -> Result<WrapUploadOutcome, String> {
    use crate::providers::baidu_netdisk::UploadPhase;

    let key = container::get_or_create_key(pool).await?;
    let app_root = app_root.trim_end_matches('/');

    let total = std::fs::metadata(source_path)
        .map_err(|e| format!("I/O error reading upload size: {e}"))?
        .len();

    // Oversized artifact → split into encrypted parts under the per-object cap.
    if total > PART_SIZE {
        return wrap_and_upload_split(
            pool,
            app_root,
            source_path,
            exclude_file_id,
            total,
            &key,
            on_progress,
        )
        .await;
    }

    // ── Single-object path (unchanged) ──────────────────────────────
    let relative_path = unique_token(pool, exclude_file_id).await?;
    let remote_path = format!("{app_root}/{relative_path}");

    let tmp = std::env::temp_dir()
        .join(format!("{WRAP_TEMP_PREFIX}{}.bbx", container::random_token()));
    // A failed wrap can still leave a partial container behind; clean up the
    // temp before propagating so it isn't leaked on this early-return path.
    // `wrap_with_progress` is blocking + CPU-heavy; offload to the blocking
    // pool so it doesn't stall the worker's async runtime, threading progress
    // out through a channel to keep emitting `Encrypting` ticks.
    {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(i64, i64)>();
        let src = source_path.to_path_buf();
        let dst = tmp.clone();
        let wrap_task = tokio::task::spawn_blocking(move || {
            container::wrap_with_progress(&src, &dst, &key, |done, total| {
                let _ = tx.send((done as i64, total as i64));
            })
        });
        while let Some((done, total)) = rx.recv().await {
            on_progress(Phase::Encrypting, done, total);
        }
        match wrap_task.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(format!("container wrap failed: {e}"));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(format!("container wrap task failed: {e}"));
            }
        }
    }

    let result = upload_to_remote(pool, &tmp, &remote_path, |phase, done, total| {
        let p = match phase {
            UploadPhase::Hashing => Phase::Hashing,
            UploadPhase::Uploading => Phase::Uploading,
        };
        on_progress(p, done, total);
    })
    .await;
    let _ = std::fs::remove_file(&tmp);
    let upload = result?;
    Ok(WrapUploadOutcome::Single {
        relative_path,
        upload,
    })
}

/// Generate an opaque, extension-less remote token unique within `files.path`.
/// A 128-bit collision is astronomically unlikely, but the de-dup invariant is
/// cheap and keeps `files.path UNIQUE` honest.
async fn unique_token(pool: &sqlx::SqlitePool, exclude_file_id: i64) -> Result<String, String> {
    loop {
        let token = container::random_token();
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM files WHERE path = ? AND id != ?")
                .bind(&token)
                .bind(exclude_file_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;
        if existing.is_none() {
            return Ok(token);
        }
    }
}

/// Split path: encrypt and upload `source_path` as `ceil(total / PART_SIZE)`
/// independent parts, one at a time (temp footprint = one part). Each part is a
/// standalone `.bbx` object; on any part failure, already-uploaded part objects
/// are best-effort deleted so a partial split never leaks. Overall progress
/// advances one part-chunk per part during its Uploading phase (the slow leg);
/// Encrypting/Hashing hold the baseline so the bar stays monotonic, while the
/// phase label still updates.
async fn wrap_and_upload_split(
    pool: &sqlx::SqlitePool,
    app_root: &str,
    source_path: &std::path::Path,
    exclude_file_id: i64,
    total: u64,
    key: &[u8; 32],
    mut on_progress: impl FnMut(Phase, i64, i64),
) -> Result<WrapUploadOutcome, String> {
    use crate::providers::baidu_netdisk::UploadPhase;

    let placeholder_path = unique_token(pool, exclude_file_id).await?;
    let n_parts = total.div_ceil(PART_SIZE);
    let total_i = total as i64;
    let mut parts: Vec<UploadedPart> = Vec::with_capacity(n_parts as usize);
    let mut completed_plaintext: i64 = 0;

    for i in 0..n_parts {
        let offset = i * PART_SIZE;
        let len = PART_SIZE.min(total - offset);
        let object_name = container::random_token();
        let remote_path = format!("{app_root}/{object_name}");
        let tmp = std::env::temp_dir()
            .join(format!("{WRAP_TEMP_PREFIX}{}.bbx", container::random_token()));

        // Encrypt this range → tmp on the blocking pool. The bar holds at the
        // baseline; we drain the channel only to keep emitting the phase label.
        {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
            let src = source_path.to_path_buf();
            let dst = tmp.clone();
            let key = *key;
            let wrap_task = tokio::task::spawn_blocking(move || {
                container::wrap_range_with_progress(&src, &dst, &key, offset, len, |_, _| {
                    let _ = tx.send(());
                })
            });
            while rx.recv().await.is_some() {
                on_progress(Phase::Encrypting, completed_plaintext, total_i);
            }
            match wrap_task.await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    let _ = std::fs::remove_file(&tmp);
                    cleanup_part_objects(pool, app_root, &parts).await;
                    return Err(format!("container wrap (part {i}) failed: {e}"));
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    cleanup_part_objects(pool, app_root, &parts).await;
                    return Err(format!("container wrap task (part {i}) failed: {e}"));
                }
            }
        }

        let part_len = len as i64;
        let base = completed_plaintext;
        let result = upload_to_remote(pool, &tmp, &remote_path, |phase, done, ph_total| {
            let p = match phase {
                UploadPhase::Hashing => Phase::Hashing,
                UploadPhase::Uploading => Phase::Uploading,
            };
            // Advance the overall bar only during Uploading, scaled to this
            // part's share of the whole-file plaintext; Hashing holds baseline.
            let overall = if matches!(phase, UploadPhase::Uploading) && ph_total > 0 {
                base + (done as i128 * part_len as i128 / ph_total as i128) as i64
            } else {
                base
            };
            on_progress(p, overall, total_i);
        })
        .await;
        let _ = std::fs::remove_file(&tmp);

        let upload = match result {
            Ok(u) => u,
            Err(e) => {
                cleanup_part_objects(pool, app_root, &parts).await;
                return Err(format!("upload (part {i}) failed: {e}"));
            }
        };

        parts.push(UploadedPart {
            index: i as i64,
            object_name,
            fs_id: upload.fs_id,
            md5: upload.md5,
            ciphertext_size: upload.size,
            plaintext_size: part_len,
        });
        completed_plaintext += part_len;
    }

    Ok(WrapUploadOutcome::Split {
        placeholder_path,
        total_plaintext: total_i,
        parts,
    })
}

/// Best-effort delete of already-uploaded part objects when a split upload
/// aborts mid-way, so a failed multi-part upload doesn't leak remote objects.
async fn cleanup_part_objects(pool: &sqlx::SqlitePool, app_root: &str, parts: &[UploadedPart]) {
    for part in parts {
        let remote_path = format!("{app_root}/{}", part.object_name);
        if let Err(e) = crate::commands::remote::delete_on_remote(pool, &remote_path).await {
            eprintln!("Split upload cleanup: failed to delete orphan part {remote_path}: {e}");
        }
    }
}

/// Persist a split file's parts: clear any prior rows for this file (so a retry
/// is idempotent) then insert the new set.
pub(crate) async fn persist_remote_parts(
    pool: &sqlx::SqlitePool,
    file_id: i64,
    parts: &[UploadedPart],
) -> Result<(), String> {
    sqlx::query("DELETE FROM remote_parts WHERE file_id = ?")
        .bind(file_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    for part in parts {
        sqlx::query(
            "INSERT INTO remote_parts \
             (file_id, part_index, object_name, fs_id, md5, ciphertext_size, plaintext_size) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(file_id)
        .bind(part.index)
        .bind(&part.object_name)
        .bind(&part.fs_id)
        .bind(&part.md5)
        .bind(part.ciphertext_size)
        .bind(part.plaintext_size)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn emit(app: &AppHandle, file_id: i64, file_name: &str, status: &str, error: Option<String>) {
    let _ = app.emit(
        "remote-upload-progress",
        &RemoteUploadProgressEvent {
            file_id,
            file_name: file_name.to_string(),
            status: status.to_string(),
            error,
            uploaded_bytes: None,
            total_bytes: None,
            phase: None,
        },
    );
}

/// Emit a progress tick carrying byte counts + the active phase. Status stays
/// `uploading` for all three phases (the panel's spinner is the same); the
/// `phase` label distinguishes Encrypting / Hashing / Uploading.
fn emit_progress(
    app: &AppHandle,
    file_id: i64,
    file_name: &str,
    phase: Phase,
    done: i64,
    total: i64,
) {
    let phase_str = match phase {
        Phase::Encrypting => "encrypting",
        Phase::Hashing => "hashing",
        Phase::Uploading => "uploading",
    };
    let _ = app.emit(
        "remote-upload-progress",
        &RemoteUploadProgressEvent {
            file_id,
            file_name: file_name.to_string(),
            status: "uploading".to_string(),
            error: None,
            uploaded_bytes: Some(done),
            total_bytes: Some(total),
            phase: Some(phase_str.to_string()),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn sweep_removes_only_wrap_temps() {
        // Isolated scratch dir so we never touch the real temp dir.
        let dir = std::env::temp_dir().join(format!("biblio-sweep-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // Real orphans (upload + reencrypt), plus files the sweep must NOT
        // delete: an unrelated `.bbx`/`.raw` and a wrap-prefixed file with a
        // different extension.
        fs::write(dir.join("biblio-wrap-aaa.bbx"), b"x").unwrap();
        fs::write(dir.join("biblio-wrap-bbb.bbx"), b"y").unwrap();
        fs::write(dir.join("biblio-reenc-ddd.raw"), b"r").unwrap();
        fs::write(dir.join("someone-elses.bbx"), b"z").unwrap();
        fs::write(dir.join("someone-elses.raw"), b"q").unwrap();
        fs::write(dir.join("biblio-wrap-ccc.tmp"), b"w").unwrap();

        let removed = sweep_dir(&dir);

        assert_eq!(removed, 3, "the wrap + reenc temps should be removed");
        assert!(!dir.join("biblio-wrap-aaa.bbx").exists());
        assert!(!dir.join("biblio-wrap-bbb.bbx").exists());
        assert!(!dir.join("biblio-reenc-ddd.raw").exists());
        assert!(dir.join("someone-elses.bbx").exists(), "unrelated .bbx must survive");
        assert!(dir.join("someone-elses.raw").exists(), "unrelated .raw must survive");
        assert!(dir.join("biblio-wrap-ccc.tmp").exists(), "non-.bbx must survive");

        let _ = fs::remove_dir_all(&dir);
    }
}
