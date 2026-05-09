//! Long-running background workers.
//!
//! Each worker owns a tokio mpsc receiver and drains it sequentially. The
//! matching `Sender` is stored in Tauri state at app setup; commands push
//! jobs and return immediately. This decouples user-facing command latency
//! from the work itself, and lets the user keep enqueueing while previous
//! work is in flight (the producer-consumer pattern).

pub mod import_worker;
pub mod upload_worker;
