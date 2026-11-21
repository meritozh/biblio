pub mod commands;
pub mod database;
mod library;
pub use library::{Library, Services};
mod path_resolve;
mod pipeline;
mod providers;
mod schema;
mod services;

use std::sync::atomic::{AtomicU64, Ordering};

/// Generation-based cancellation for the processing pipeline.
///
/// A single global flag used to be flipped by `cancel_processing` and reset by
/// every `enqueue_import`, so a new enqueue could silently un-cancel a
/// still-draining prior batch (and one cancel could spill onto later work).
/// Instead each batch of work claims a monotonic generation; `cancel_all`
/// records the highest generation claimed so far as cancelled, and a later
/// `begin()` claims a higher generation the cancel doesn't cover.
pub struct ProcessingCancel {
    next_generation: AtomicU64,
    cancelled_through: AtomicU64,
}

impl ProcessingCancel {
    fn new() -> Self {
        Self {
            next_generation: AtomicU64::new(1),
            cancelled_through: AtomicU64::new(0),
        }
    }

    /// Claim a fresh generation for a new batch of work.
    pub fn begin(&self) -> u64 {
        self.next_generation.fetch_add(1, Ordering::SeqCst)
    }

    /// Cancel every batch claimed so far. Work claimed by a later `begin()`
    /// is unaffected, so a cancel can't defeat a subsequently-enqueued batch.
    pub fn cancel_all(&self) {
        let latest = self
            .next_generation
            .load(Ordering::SeqCst)
            .saturating_sub(1);
        self.cancelled_through.fetch_max(latest, Ordering::SeqCst);
    }

    /// Whether the given batch generation has been cancelled.
    pub fn is_cancelled(&self, generation: u64) -> bool {
        generation <= self.cancelled_through.load(Ordering::SeqCst)
    }
}
