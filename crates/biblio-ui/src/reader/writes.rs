//! One serial lane per window. A read on this lane observes every earlier edit,
//! including a position whose periodic write has not run yet.
use crate::app::run_service;
use biblio_core::text_reader::{
    book::Checkpoint,
    store::{BookEdit, ReaderStore},
};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

type Operation = Pin<Box<dyn Future<Output = ()> + Send>>;

type Latest = Mutex<Option<Checkpoint>>;

enum Message {
    Position(ReaderStore, i64, Arc<Latest>),
    Barrier(Operation),
    Drain(std::sync::mpsc::Sender<Result<(), String>>),
}

#[derive(Clone)]
pub struct ReaderWrites(Arc<Mutex<Mailbox>>);

struct Mailbox {
    tx: mpsc::UnboundedSender<Message>,
    // Only the tail position may be updated. Reads and edits seal it.
    last: Option<(i64, Weak<Latest>)>,
}

impl ReaderWrites {
    pub(crate) fn new() -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();
        run_service(async move {
            let mut pending = None;
            let mut error = None;
            let mut deadline = tokio::time::Instant::now();
            loop {
                let message = if pending.is_some() {
                    tokio::select! {
                        biased;
                        _ = tokio::time::sleep_until(deadline) => {
                            flush(&mut pending, &mut error).await;
                            continue;
                        }
                        next = rx.recv() => next,
                    }
                } else {
                    rx.recv().await
                };
                match message {
                    Some(Message::Position(store, id, at)) => {
                        if pending.as_ref().is_some_and(|(_, file, _)| *file != id) {
                            flush(&mut pending, &mut error).await;
                        }
                        if pending.is_none() {
                            deadline = tokio::time::Instant::now() + Duration::from_millis(350);
                        }
                        pending = Some((store, id, at));
                    }
                    other => {
                        flush(&mut pending, &mut error).await;
                        match other {
                            Some(Message::Barrier(work)) => work.await,
                            Some(Message::Drain(reply)) => {
                                let _ = reply.send(error.clone().map_or(Ok(()), Err));
                            }
                            None => break,
                            _ => unreachable!(),
                        }
                    }
                }
            }
        });
        Self(Arc::new(Mutex::new(Mailbox { tx, last: None })))
    }

    pub(crate) fn position(&self, store: ReaderStore, file: i64, checkpoint: Checkpoint) {
        let mut mailbox = self.0.lock().unwrap();
        if let Some((id, slot)) = &mailbox.last
            && *id == file
            && let Some(slot) = slot.upgrade()
            && let Some(pending) = slot.lock().unwrap().as_mut()
        {
            *pending = checkpoint;
            return;
        }
        let at = Arc::new(Mutex::new(Some(checkpoint)));
        mailbox.last = Some((file, Arc::downgrade(&at)));
        if mailbox.tx.send(Message::Position(store, file, at)).is_err() {
            eprintln!("Reader checkpoint could not be queued");
        }
    }

    fn barrier(&self, message: Message) -> Result<(), String> {
        let mut mailbox = self.0.lock().unwrap();
        mailbox.last = None;
        mailbox
            .tx
            .send(message)
            .map_err(|_| "Reader writer stopped".into())
    }

    /// Enqueue synchronously, before handing the receiver to an owner-bound UI task.
    pub(crate) fn ordered<T: Send + 'static>(
        &self,
        work: impl Future<Output = T> + Send + 'static,
    ) -> oneshot::Receiver<T> {
        let (tx, rx) = oneshot::channel();
        let _ = self.barrier(Message::Barrier(Box::pin(async move {
            let _ = tx.send(work.await);
        })));
        rx
    }

    pub(crate) fn finish_session(&self) {
        drop(self.ordered(async {}));
    }

    /// Wait for accepted edits, including the latest scroll position. Called
    /// from the window/host thread, never from the service worker.
    pub fn drain(&self) -> Result<(), String> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.barrier(Message::Drain(tx))?;
        rx.recv().map_err(|_| "Reader writer stopped")?
    }
}

async fn flush(pending: &mut Option<(ReaderStore, i64, Arc<Latest>)>, error: &mut Option<String>) {
    let Some((store, file, latest)) = pending.take() else {
        return;
    };
    let checkpoint = latest
        .lock()
        .unwrap()
        .take()
        .expect("only the worker consumes a position");
    if let Err(e) = store.edit_book(file, BookEdit::Position(checkpoint)).await {
        eprintln!("Could not save the reading position for file {file}: {e}");
        *error = Some(e.to_string());
    }
}

// NSH drops stored values on full engine teardown without running reactive
// cleanup callbacks. RAII therefore covers both native and headless closes.
pub(crate) struct WindowWriter(pub ReaderWrites);
impl Drop for WindowWriter {
    fn drop(&mut self) {
        if let Err(error) = self.0.drain() {
            eprintln!("Reader save: {error}");
        }
    }
}
