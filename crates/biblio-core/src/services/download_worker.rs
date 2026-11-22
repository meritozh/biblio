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

use crate::{
    Services,
    commands::{remote::ensure_access_token, settings::load_path_roots},
    providers::baidu_netdisk::{download_to, get_download_dlink},
    services::container,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyState {
    Queued,
    Downloading,
    Ready(PathBuf),
    Failed(String),
}

type InFlightDownloads = Arc<Mutex<HashMap<i64, watch::Sender<CopyState>>>>;

pub(crate) struct DownloadQueue {
    jobs: mpsc::UnboundedSender<DownloadJob>,
    flights: InFlightDownloads,
}

impl DownloadQueue {
    pub(crate) fn enqueue(&self, file_id: i64) -> Result<watch::Receiver<CopyState>, String> {
        let mut flights = self.flights.lock().unwrap();
        if let Some(state) = flights.get(&file_id) {
            return Ok(state.subscribe());
        }
        let (state, receiver) = watch::channel(CopyState::Queued);
        flights.insert(file_id, state.clone());
        let job = DownloadJob {
            file_id,
            state,
            flights: self.flights.clone(),
        };
        drop(flights);
        self.jobs
            .send(job)
            .map_err(|_| "Download queue is closed".to_string())?;
        Ok(receiver)
    }
}

struct DownloadJob {
    file_id: i64,
    state: watch::Sender<CopyState>,
    flights: InFlightDownloads,
}

impl DownloadJob {
    fn settle(&self, state: CopyState) {
        self.flights.lock().unwrap().remove(&self.file_id);
        self.state.send_replace(state);
    }
}

impl Drop for DownloadJob {
    fn drop(&mut self) {
        // Covers a closed inbox, cancellation mid-transfer and jobs still in the inbox.
        if matches!(
            *self.state.borrow(),
            CopyState::Queued | CopyState::Downloading
        ) {
            self.settle(CopyState::Failed("Download queue is closed".into()));
        }
    }
}

#[async_trait::async_trait]
trait DownloadObject: Send + Sync {
    async fn fetch(&self, fs_id: &str, dest: &Path) -> Result<(), String>;
}

struct BaiduDownload(sqlx::SqlitePool);

#[async_trait::async_trait]
impl DownloadObject for BaiduDownload {
    async fn fetch(&self, fs_id: &str, dest: &Path) -> Result<(), String> {
        let token = ensure_access_token(&self.0).await?;
        let dlink = get_download_dlink(&token, fs_id).await.map_err(|e| e.0)?;
        download_to(&token, &dlink, dest)
            .await
            .map(|_| ())
            .map_err(|e| e.0)
    }
}

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

pub(crate) fn spawn(app: Services) -> DownloadQueue {
    let objects = BaiduDownload(app.pool.clone());
    spawn_with(app, objects)
}

fn spawn_with(app: Services, objects: impl DownloadObject + 'static) -> DownloadQueue {
    let (jobs, mut inbox) = mpsc::unbounded_channel::<DownloadJob>();
    tokio::spawn(async move {
        while let Some(job) = inbox.recv().await {
            let row = sqlx::query_as::<_, DownloadFile>(
                "SELECT path, display_name, original_path, remote_fs_id,
                        COALESCE(storage_kind, 'local') AS storage_kind, remote_container,
                        remote_size FROM files WHERE id = ?",
            )
            .bind(job.file_id)
            .fetch_optional(&app.pool)
            .await;
            let (name, result) = match row {
                Ok(Some(file)) => {
                    let result = download(&app, &job, &file, &objects).await;
                    (file.display_name, result)
                }
                Ok(None) => (String::new(), Err("File not found".into())),
                Err(e) => (String::new(), Err(e.to_string())),
            };
            match result {
                Ok(path) => {
                    let absolute = path.to_string_lossy().into_owned();
                    job.settle(CopyState::Ready(path));
                    emit(&app, job.file_id, &name, "success", None, Some(absolute));
                }
                Err(e) => {
                    job.settle(CopyState::Failed(e.clone()));
                    emit(&app, job.file_id, &name, "error", Some(e), None);
                }
            }
        }
    });
    DownloadQueue {
        jobs,
        flights: InFlightDownloads::default(),
    }
}

#[derive(sqlx::FromRow)]
struct DownloadFile {
    path: String,
    display_name: String,
    original_path: Option<String>,
    remote_fs_id: Option<String>,
    storage_kind: String,
    remote_container: Option<String>,
    remote_size: Option<i64>,
}

enum Source<'a> {
    Single { id: &'a str, wrapped: bool },
    Split(Vec<(i64, String, Option<String>)>),
}

async fn download(
    app: &Services,
    job: &DownloadJob,
    file: &DownloadFile,
    objects: &dyn DownloadObject,
) -> Result<PathBuf, String> {
    if file.storage_kind != "remote" {
        return Err("File is not in remote storage".into());
    }
    let roots = load_path_roots(&app.pool).await?;
    let source = if is_multipart(file.remote_container.as_deref()) {
        let parts = sqlx::query_as(
            "SELECT part_index, object_name, fs_id FROM remote_parts
             WHERE file_id = ? ORDER BY part_index",
        )
        .bind(job.file_id)
        .fetch_all(&app.pool)
        .await
        .map_err(|e| e.to_string())?;
        if parts.is_empty() {
            return Err("Split file has no parts recorded".into());
        }
        Source::Split(parts)
    } else {
        let id = file
            .remote_fs_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or("Row missing remote_fs_id; cannot resolve dlink")?;
        Source::Single {
            id,
            wrapped: file.remote_container.as_deref() == Some("bbx1"),
        }
    };
    if roots.storage_path.is_empty() {
        return Err("Storage path is not configured".into());
    }
    let cache_dir = Path::new(&roots.storage_path).join(".cache");
    fs::create_dir_all(&cache_dir).map_err(|e| format!("Failed to create cache dir: {e}"))?;
    let basename = derive_basename(
        &file.path,
        file.original_path.as_deref(),
        &file.display_name,
    );
    let stored = Path::new(".cache").join(format!("{}_{}", job.file_id, basename));

    // The provider may create its own temporary sibling. Keep its entire
    // workspace scoped to this attempt, including when the task is dropped.
    let work = tempfile::tempdir_in(&cache_dir).map_err(|e| e.to_string())?;
    let object = work.path().join("object");
    let staged = tempfile::NamedTempFile::new_in(&cache_dir)
        .map_err(|e| e.to_string())?
        .into_temp_path();

    job.state.send_replace(CopyState::Downloading);
    emit(
        app,
        job.file_id,
        &file.display_name,
        "downloading",
        None,
        None,
    );
    match source {
        Source::Single { id, wrapped } => {
            objects.fetch(id, &object).await?;
            // Single objects record their wire size. Split rows record plaintext size.
            check_size(&object, file.remote_size)?;
            if wrapped {
                let key = container::get_or_create_key(&app.pool).await?;
                container::unwrap(&object, &staged, &key)
                    .map_err(|e| format!("container unwrap failed: {e}"))?;
                fs::remove_file(&object).map_err(|e| e.to_string())?;
            } else {
                fs::rename(&object, &staged).map_err(|e| e.to_string())?;
            }
        }
        Source::Split(parts) => {
            let key = container::get_or_create_key(&app.pool).await?;
            download_split(objects, parts, &object, &staged, &key).await?;
            check_size(&staged, file.remote_size)?;
        }
    }

    publish(
        app,
        job.file_id,
        &roots.storage_path,
        &stored,
        staged,
        work.path(),
    )
    .await
}

fn is_multipart(remote_container: Option<&str>) -> bool {
    remote_container == Some("bbx1-split")
}

fn check_size(path: &Path, expected: Option<i64>) -> Result<(), String> {
    if let Some(expected) = expected {
        let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
        if Some(size) != u64::try_from(expected).ok() {
            return Err(format!("Download size {size} != expected {expected}"));
        }
    }
    Ok(())
}

async fn download_split(
    objects: &dyn DownloadObject,
    parts: Vec<(i64, String, Option<String>)>,
    object: &Path,
    staged: &Path,
    key: &[u8; 32],
) -> Result<(), String> {
    let mut output = fs::File::create(staged).map_err(|e| e.to_string())?;
    for (index, name, id) in parts {
        let id = id
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("Part {index} ({name}) missing fs_id"))?;
        objects.fetch(&id, object).await?;
        container::unwrap_append(object, &mut output, key)
            .map_err(|e| format!("Part {index} unwrap failed: {e}"))?;
        fs::remove_file(object).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Hold the previous bytes through the catalog commit, including on a failed
/// explicit refresh. Both the backup and staged data are on the cache volume.
async fn publish(
    app: &Services,
    file_id: i64,
    storage_root: &str,
    stored: &Path,
    staged: tempfile::TempPath,
    work: &Path,
) -> Result<PathBuf, String> {
    let path =
        std::path::absolute(Path::new(storage_root).join(stored)).map_err(|e| e.to_string())?;
    let _write = app.cache_access.write().await;
    let previous = if path.try_exists().map_err(|e| e.to_string())? {
        let backup = work.join("previous");
        // Some library volumes do not support hard links.
        if fs::hard_link(&path, &backup).is_err() {
            tokio::fs::copy(&path, &backup)
                .await
                .map_err(|e| format!("Cannot preserve {}: {e}", path.display()))?;
        }
        Some(backup)
    } else {
        None
    };
    staged
        .persist(&path)
        .map_err(|e| format!("Cannot publish {}: {e}", path.display()))?;
    let mut rollback = PublishedCopy {
        path: &path,
        previous,
        committed: false,
    };
    let updated = sqlx::query(
        "UPDATE files SET local_cache_path = ? WHERE id = ? AND storage_kind = 'remote'
         AND (SELECT value FROM app_settings WHERE key = 'storage_path') = ?",
    )
    .bind(stored.to_string_lossy().as_ref())
    .bind(file_id)
    .bind(storage_root)
    .execute(&app.pool)
    .await
    .map_err(|e| format!("DB update failed: {e}"))?;
    if updated.rows_affected() != 1 {
        return Err("File or storage path changed during download".into());
    }
    rollback.committed = true;
    drop(rollback);
    Ok(path)
}

struct PublishedCopy<'a> {
    path: &'a Path,
    previous: Option<PathBuf>,
    committed: bool,
}

impl Drop for PublishedCopy<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let result = match &self.previous {
                Some(backup) => fs::rename(backup, self.path),
                None => fs::remove_file(self.path),
            };
            if let Err(e) = result {
                eprintln!(
                    "Could not roll back cache publication at {}: {e}",
                    self.path.display()
                );
            }
        }
    }
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
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '\0') {
                '_'
            } else {
                c
            }
        })
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
    app: &Services,
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
mod tests;
