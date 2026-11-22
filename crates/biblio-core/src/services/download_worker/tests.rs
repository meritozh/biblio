use super::*;
use crate::{
    Library,
    commands::{cache::readable_copy, remote::file_download_from_remote},
};
use serde_json::{Value, json};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time::timeout;

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

struct Fetch {
    id: String,
    response: oneshot::Sender<Result<Vec<u8>, String>>,
}

struct Objects(mpsc::UnboundedSender<Fetch>);

#[async_trait::async_trait]
impl DownloadObject for Objects {
    async fn fetch(&self, id: &str, dest: &Path) -> Result<(), String> {
        fs::write(dest, b"incomplete").unwrap();
        let (response, receiver) = oneshot::channel();
        self.0
            .send(Fetch {
                id: id.into(),
                response,
            })
            .unwrap();
        let bytes = receiver.await.unwrap()?;
        fs::write(dest, bytes).map_err(|e| e.to_string())
    }
}

struct Fixture {
    app: Library,
    profile: tempfile::TempDir,
    storage: PathBuf,
    fetches: mpsc::UnboundedReceiver<Fetch>,
    events: mpsc::UnboundedReceiver<Value>,
}

impl Fixture {
    async fn new() -> Self {
        let profile = tempfile::tempdir().unwrap();
        let (events_tx, events) = mpsc::unbounded_channel();
        let mut app = Library::open(profile.path().join("profile"), move |name, event| {
            if name == "remote-download-progress" {
                let _ = events_tx.send(event);
            }
        })
        .await
        .unwrap();
        let (sender, fetches) = mpsc::unbounded_channel();
        let queue = spawn_with((*app).clone(), Objects(sender));
        Arc::get_mut(&mut app.queues).unwrap().download = queue;
        let storage = profile.path().join("library");
        fs::create_dir(&storage).unwrap();
        sqlx::query("UPDATE app_settings SET value = ? WHERE key = 'storage_path'")
            .bind(storage.to_str().unwrap())
            .execute(&app.pool)
            .await
            .unwrap();
        Self {
            app,
            profile,
            storage,
            fetches,
            events,
        }
    }

    async fn remote(&self, id: i64, marker: Option<&str>, fs_id: Option<&str>, size: Option<i64>) {
        sqlx::query(
            "INSERT INTO files (id, path, display_name, original_path, storage_kind,
                         remote_container, remote_fs_id, remote_size)
                     VALUES (?, ?, 'Book', 'category/book.txt', 'remote', ?, ?, ?)",
        )
        .bind(id)
        .bind(format!("object-{id}"))
        .bind(marker)
        .bind(fs_id)
        .bind(size)
        .execute(&self.app.pool)
        .await
        .unwrap();
    }

    fn path(&self, id: i64) -> PathBuf {
        self.storage.join(".cache").join(format!("{id}_book.txt"))
    }

    async fn stored(&self, id: i64) -> Option<String> {
        sqlx::query_scalar("SELECT local_cache_path FROM files WHERE id = ?")
            .bind(id)
            .fetch_one(&self.app.pool)
            .await
            .unwrap()
    }

    async fn fetch(&mut self, id: &str) -> oneshot::Sender<Result<Vec<u8>, String>> {
        let next = timeout(Duration::from_secs(5), self.fetches.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next.id, id);
        next.response
    }

    async fn event(&mut self) -> Value {
        timeout(Duration::from_secs(5), self.events.recv())
            .await
            .unwrap()
            .unwrap()
    }

    async fn wrapped(&self, plaintext: &[u8]) -> Vec<u8> {
        let key = container::get_or_create_key(&self.app.pool).await.unwrap();
        let src = self.profile.path().join("plain");
        let dst = self.profile.path().join("wrapped");
        fs::write(&src, plaintext).unwrap();
        container::wrap_with_progress(&src, &dst, &key, |_, _| {}).unwrap();
        fs::read(dst).unwrap()
    }
}

async fn terminal(mut receiver: watch::Receiver<CopyState>) -> CopyState {
    timeout(Duration::from_secs(5), async {
        loop {
            let state = receiver.borrow_and_update().clone();
            if matches!(state, CopyState::Ready(_) | CopyState::Failed(_)) {
                return state;
            }
            receiver.changed().await.unwrap();
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn uncached_open_joins_download_and_resolves_committed_copy() {
    let mut f = Fixture::new().await;
    f.remote(1, None, Some("first"), None).await;
    f.remote(2, None, Some("second"), Some(4)).await;
    file_download_from_remote(f.app.clone(), vec![1, 2, 2])
        .await
        .unwrap();
    let first = f.fetch("first").await;
    let queued = readable_copy(&f.app, 2).await.unwrap();
    assert_eq!(*queued.borrow(), CopyState::Queued);
    drop(queued);

    first.send(Ok(b"first".to_vec())).unwrap();
    let second = f.fetch("second").await;
    let abandoned = readable_copy(&f.app, 2).await.unwrap();
    assert_eq!(*abandoned.borrow(), CopyState::Downloading);
    drop(abandoned);
    let rejoined = readable_copy(&f.app, 2).await.unwrap();
    let another = readable_copy(&f.app, 2).await.unwrap();
    file_download_from_remote(f.app.clone(), vec![2])
        .await
        .unwrap();
    assert!(rejoined.same_channel(&another));
    assert!(!f.path(2).exists());
    assert_eq!(f.stored(2).await, None);

    second.send(Ok(b"book".to_vec())).unwrap();
    let ready = CopyState::Ready(f.path(2));
    assert_eq!(terminal(rejoined).await, ready);
    assert_eq!(terminal(another).await, ready);
    assert_eq!(
        f.stored(2).await,
        Some(
            Path::new(".cache")
                .join("2_book.txt")
                .to_string_lossy()
                .into_owned()
        )
    );
    assert_eq!(fs::read(f.path(2)).unwrap(), b"book");
    assert_eq!(*readable_copy(&f.app, 2).await.unwrap().borrow(), ready);
    for id in [1, 2] {
        assert_eq!(
            f.event().await,
            json!({"file_id": id, "file_name": "Book", "status":"downloading"})
        );
        let path = f.path(id);
        assert_eq!(
            f.event().await,
            json!({"file_id": id, "file_name": "Book", "status":"success", "local_cache_path": path})
        );
    }

    // A completed file may be explicitly refreshed. Its existing readable
    // copy stays usable during the transfer and after a transfer failure.
    file_download_from_remote(f.app.clone(), vec![2])
        .await
        .unwrap();
    let refresh = f.fetch("second").await;
    assert_eq!(*readable_copy(&f.app, 2).await.unwrap().borrow(), ready);
    refresh.send(Err("network failure".into())).unwrap();
    assert_eq!(f.event().await["status"], "downloading");
    assert_eq!(
        f.event().await,
        json!({"file_id":2, "file_name":"Book", "status":"error", "error":"network failure"})
    );
    assert_eq!(fs::read(f.path(2)).unwrap(), b"book");
}

#[tokio::test]
async fn download_retries_a_failed_attempt_and_a_moved_cache() {
    let mut f = Fixture::new().await;
    f.remote(1, None, Some("raw"), Some(2)).await;
    let (failed, peer) = tokio::join!(readable_copy(&f.app, 1), readable_copy(&f.app, 1));
    let (failed, peer) = (failed.unwrap(), peer.unwrap());
    assert!(failed.same_channel(&peer));
    file_download_from_remote(f.app.clone(), vec![1])
        .await
        .unwrap();
    f.fetch("raw").await.send(Err("offline".into())).unwrap();
    for receiver in [failed, peer] {
        assert_eq!(
            terminal(receiver).await,
            CopyState::Failed("offline".into())
        );
    }
    assert!(f.stored(1).await.is_none());
    assert_eq!(fs::read_dir(f.storage.join(".cache")).unwrap().count(), 0);

    let retry = readable_copy(&f.app, 1).await.unwrap();
    f.fetch("raw").await.send(Ok(b"ok".to_vec())).unwrap();
    assert_eq!(terminal(retry).await, CopyState::Ready(f.path(1)));
    fs::rename(f.path(1), f.storage.join("moved")).unwrap();
    let missing = readable_copy(&f.app, 1).await.unwrap();
    f.fetch("raw").await.send(Ok(b"ok".to_vec())).unwrap();
    assert_eq!(terminal(missing).await, CopyState::Ready(f.path(1)));
    assert_eq!(fs::read(f.path(1)).unwrap(), b"ok");
}

#[tokio::test]
async fn download_decrypts_single_objects_and_checks_ordered_split_plaintext() {
    let mut f = Fixture::new().await;
    let a = f.wrapped(b"one").await;
    let b = f.wrapped(b"two").await;
    f.remote(1, Some("bbx1"), Some("whole"), Some(a.len() as i64))
        .await;
    f.remote(2, Some("bbx1-split"), None, Some(6)).await;
    for (index, id) in [(1, "b"), (0, "a")] {
        sqlx::query(
            "INSERT INTO remote_parts(file_id, part_index, object_name, fs_id) VALUES (2, ?, ?, ?)",
        )
        .bind(index)
        .bind(id)
        .bind(id)
        .execute(&f.app.pool)
        .await
        .unwrap();
    }

    let single = readable_copy(&f.app, 1).await.unwrap();
    f.fetch("whole").await.send(Ok(a.clone())).unwrap();
    assert_eq!(terminal(single).await, CopyState::Ready(f.path(1)));
    assert_eq!(fs::read(f.path(1)).unwrap(), b"one");

    let split = readable_copy(&f.app, 2).await.unwrap();
    f.fetch("a").await.send(Ok(a.clone())).unwrap();
    let tail = f.fetch("b").await;
    assert!(!f.path(2).exists());
    assert_eq!(f.stored(2).await, None);
    tail.send(Ok(b"invalid container".to_vec())).unwrap();
    assert!(
        matches!(terminal(split).await, CopyState::Failed(e) if e.contains("Part 1 unwrap failed"))
    );
    assert_eq!(fs::read_dir(f.storage.join(".cache")).unwrap().count(), 1);

    let wrong_size = readable_copy(&f.app, 2).await.unwrap();
    f.fetch("a").await.send(Ok(a.clone())).unwrap();
    let short = f.wrapped(b"x").await;
    f.fetch("b").await.send(Ok(short)).unwrap();
    assert!(
        matches!(terminal(wrong_size).await, CopyState::Failed(e) if e.contains("4 != expected 6"))
    );
    assert!(!f.path(2).exists());

    let split = readable_copy(&f.app, 2).await.unwrap();
    f.fetch("a").await.send(Ok(a)).unwrap();
    f.fetch("b").await.send(Ok(b)).unwrap();
    assert_eq!(terminal(split).await, CopyState::Ready(f.path(2)));
    assert_eq!(fs::read(f.path(2)).unwrap(), b"onetwo");
}

#[tokio::test]
async fn download_catalog_failure_rolls_back_cache_publication() {
    let mut f = Fixture::new().await;
    f.remote(1, None, Some("raw"), None).await;
    f.remote(2, None, Some("new"), None).await;
    let old = readable_copy(&f.app, 1).await.unwrap();
    f.fetch("raw").await.send(Ok(b"original".to_vec())).unwrap();
    assert_eq!(terminal(old).await, CopyState::Ready(f.path(1)));
    let stored = f.stored(1).await;
    sqlx::query(
        "CREATE TRIGGER reject_cache BEFORE UPDATE OF local_cache_path ON files
                 BEGIN SELECT RAISE(ABORT, 'injected catalog failure'); END",
    )
    .execute(&f.app.pool)
    .await
    .unwrap();
    file_download_from_remote(f.app.clone(), vec![1])
        .await
        .unwrap();
    f.fetch("raw")
        .await
        .send(Ok(b"replacement".to_vec()))
        .unwrap();
    let fresh = readable_copy(&f.app, 2).await.unwrap();
    f.fetch("new").await.send(Ok(b"new".to_vec())).unwrap();
    assert!(
        matches!(terminal(fresh).await, CopyState::Failed(e) if e.contains("injected catalog failure"))
    );
    assert_eq!(fs::read(f.path(1)).unwrap(), b"original");
    assert_eq!(f.stored(1).await, stored);
    assert_eq!(f.stored(2).await, None);
    assert_eq!(fs::read_dir(f.storage.join(".cache")).unwrap().count(), 1);
}

#[tokio::test]
async fn download_resolves_local_files_without_contacting_the_worker() {
    let f = Fixture::new().await;
    sqlx::query("INSERT INTO files(id, path, display_name, local_cache_path) VALUES(1, 'book.txt', 'Book', 'stale-cache')")
        .execute(&f.app.pool)
        .await
        .unwrap();
    let path = f.storage.join("book.txt");
    fs::write(&path, b"text").unwrap();
    assert_eq!(
        *readable_copy(&f.app, 1).await.unwrap().borrow(),
        CopyState::Ready(path.clone())
    );
    fs::remove_file(&path).unwrap();
    assert!(
        readable_copy(&f.app, 1)
            .await
            .unwrap_err()
            .contains("book.txt")
    );
    fs::create_dir(&path).unwrap();
    assert!(
        readable_copy(&f.app, 1)
            .await
            .unwrap_err()
            .contains("not a regular file")
    );
    assert_eq!(
        readable_copy(&f.app, 404).await.unwrap_err(),
        "FILE_NOT_FOUND"
    );
}

#[test]
fn download_worker_shutdown_wakes_active_and_queued_subscribers() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (fixture, active, queued, response) = runtime.block_on(async {
        let mut f = Fixture::new().await;
        f.remote(1, None, Some("active"), None).await;
        f.remote(2, None, Some("queued"), None).await;
        let active = readable_copy(&f.app, 1).await.unwrap();
        let response = f.fetch("active").await;
        let queued = readable_copy(&f.app, 2).await.unwrap();
        assert_eq!(*active.borrow(), CopyState::Downloading);
        assert_eq!(*queued.borrow(), CopyState::Queued);
        (f, active, queued, response)
    });
    drop(runtime);
    assert!(response.is_closed());
    let closed = CopyState::Failed("Download queue is closed".into());
    assert_eq!(*active.borrow(), closed);
    assert_eq!(*queued.borrow(), closed);
    let next = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let error = next
        .block_on(file_download_from_remote(fixture.app.clone(), vec![1]))
        .unwrap_err();
    assert!(error.contains("closed"));
}
