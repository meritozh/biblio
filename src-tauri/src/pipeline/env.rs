use std::collections::HashMap;
use std::collections::HashSet;
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

    /// Category explicitly selected by the import caller. This remains
    /// authoritative for duplicate matching even if content analysis later
    /// proposes a different category in `FileContext`. `None` for internal
    /// maintenance pipelines that did not originate from an import target.
    pub requested_category_id: Option<i64>,

    /// The schema this batch runs under: the requested category's
    /// `schema_slug`, or the extension-routed built-in for legacy
    /// no-category imports. LLM nodes resolve prompts under this slug
    /// first, falling back to `schema_template` when the custom schema
    /// has no active prompt for a step.
    pub schema_slug: String,
    /// The built-in pipeline template (`novel` / `comic` / `galgame`)
    /// the schema routes through. Doubles as the prompt-fallback slug.
    pub schema_template: String,
    /// Enabled `schema_pipeline_steps.step_key`s for `schema_slug`.
    /// Nodes whose step is absent skip themselves in `applies()` —
    /// this is what the schema editor's step toggles drive at runtime.
    pub enabled_steps: HashSet<String>,

    /// Path roots used by duplicate detection (and any future node that
    /// needs to stat existing rows on disk). Each `existing_files` row's
    /// `path` is relative to one of these; resolve via `path_resolve`.
    pub storage_path: String,
    pub app_root: String,

    pub settings: PipelineSettings,
}
