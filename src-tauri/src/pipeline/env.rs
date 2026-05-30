use std::collections::HashMap;
use std::sync::Arc;

use crate::ProcessingCancel;
use crate::commands::FileEntry;
use crate::commands::llm::LlmConfig;

/// User-configurable per-batch settings. Loaded once at the start of
/// `run_batch` from `app_settings` so per-file nodes don't re-query the DB.
#[derive(Debug, Clone)]
pub struct PipelineSettings {
    pub analyze_content: bool,
}

/// Read-only context shared across every node for every file in a batch.
/// Wrapped in `Arc` by the runner so Phase-1 tasks dispatched to
/// `spawn_blocking` cheaply share the handle.
pub struct PipelineEnv {
    pub pool: sqlx::SqlitePool,
    pub llm_config: LlmConfig,
    pub app: tauri::AppHandle,
    /// Shared cancellation state plus this batch's generation. The runner
    /// checks `cancel.is_cancelled(cancel_generation)` so a cancel of an
    /// earlier batch can't stop this one and vice versa.
    pub cancel: Arc<ProcessingCancel>,
    pub cancel_generation: u64,

    pub category_map: HashMap<String, i64>,
    pub author_map: HashMap<String, i64>,
    pub tag_map: HashMap<String, i64>,
    pub category_names: Vec<String>,
    pub tag_names: Vec<String>,
    pub existing_files: Vec<FileEntry>,

    /// Path roots used by duplicate detection (and any future node that
    /// needs to stat existing rows on disk). Each `existing_files` row's
    /// `path` is relative to one of these; resolve via `path_resolve`.
    pub storage_path: String,
    pub app_root: String,

    pub settings: PipelineSettings,
}
