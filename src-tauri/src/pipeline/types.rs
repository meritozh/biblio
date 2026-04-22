use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Progress event emitted to the frontend at key points in a file's
/// pipeline run (`gathering_signals`, `extracting_name`, `analyzing_content`,
/// and the terminal `ready`/`partial`/`error`).
#[derive(Debug, Clone, Serialize)]
pub struct ProcessingProgress {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
    pub status: String,
}

/// A single metadata field extracted from a file, surfaced to the frontend
/// through `FilePreparedImport` and `FileAnalysisResult`. The serde shape
/// must stay stable since the TypeScript side reads these fields directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedField {
    pub key: String,
    pub value: String,
    pub data_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DuplicateAction {
    Replace,
    /// Do not import; delete the new source file from disk.
    Delete,
    ImportAnyway,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateInfo {
    pub existing_file_id: i64,
    pub existing_display_name: String,
    pub existing_progress: Option<String>,
    pub recommendation: DuplicateAction,
}

#[derive(Debug, Clone)]
pub struct Cover {
    pub data: Vec<u8>,
    pub mime_type: String,
}

/// One image entry extracted from a comic archive. The LLM sees `basename`
/// when ranking cover candidates; vision calls read from `extracted_path`.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub basename: String,
    pub extracted_path: PathBuf,
}

/// Per-node outcome recorded on the `FileContext`. StatusEmitNode reads
/// these at the end of Phase 2 to decide the overall `ready`/`partial`/
/// `error` status emitted to the frontend.
#[derive(Debug, Clone)]
pub struct NodeOutcome {
    pub name: &'static str,
    pub status: NodeStatus,
}

#[derive(Debug, Clone)]
pub enum NodeStatus {
    Ok,
    /// `applies()` returned false — node did not run. Not an error.
    Skipped,
    /// Node ran and failed. The payload is surfaced via `Debug` for logs;
    /// callers check the variant, not the string.
    Err(#[allow(dead_code)] String),
}

/// Error surfaced from a node's `run`. String-typed because most underlying
/// failures are already stringified error messages; we keep the shape simple
/// so nodes don't need to agree on an error enum.
#[derive(Debug, Clone)]
pub struct NodeError(pub String);

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for NodeError {
    fn from(s: String) -> Self {
        NodeError(s)
    }
}

impl From<&str> for NodeError {
    fn from(s: &str) -> Self {
        NodeError(s.to_string())
    }
}

/// Mutable state carried through one file's pipeline run. Nodes append to
/// it; the runner reads it at the end to build the per-file output.
#[derive(Debug)]
pub struct FileContext {
    // ── Inputs (set once, never mutated) ─────────────────────────────
    pub file_path: PathBuf,
    pub file_name: String,
    /// Original position in the batch input. Retained for future nodes that
    /// need input-order correlation (e.g. progress reporting that runs
    /// before Phase 2 has set `processed_ordinal`).
    #[allow(dead_code)]
    pub input_index: usize,
    pub total: usize,

    // ── Phase 1 outputs ──────────────────────────────────────────────
    pub mime: Option<String>,
    pub extracted_metadata: Vec<ExtractedField>,
    /// Author names harvested by Phase-1 processors (PDF `Author`, etc.)
    /// and later appended by FilenameLlmNode. AuthorResolveNode turns this
    /// into `author_ids` + `unresolved_authors`.
    pub suggested_author_names: Vec<String>,
    pub content_sample: Option<String>,
    pub cover: Option<Cover>,

    // ── Comic / archive state ────────────────────────────────────────
    /// Temp directory holding the unzipped image entries. Owned by the
    /// pipeline run; `CleanupTempDirNode` removes it after Phase 2.
    pub archive_temp_dir: Option<PathBuf>,
    /// Image entries extracted from the archive, preserving zip order so
    /// "first = cover" heuristics match what readers display.
    pub archive_entries: Vec<ArchiveEntry>,
    /// LLM-ranked filenames (best first, up to 5) from the archive entries.
    /// Filled by `LlmCoverCandidatesNode` and consumed by the vision node.
    pub cover_candidates: Vec<String>,

    // ── Phase 2 outputs (final values returned to the frontend) ──────
    pub display_name: Option<String>,
    pub progress: Option<String>,
    pub category_id: Option<i64>,
    pub tag_ids: Vec<i64>,
    pub suggested_tags: Vec<String>,
    pub author_ids: Vec<i64>,
    pub unresolved_authors: Vec<String>,
    pub duplicate_of: Option<DuplicateInfo>,

    // ── Bookkeeping ──────────────────────────────────────────────────
    /// 1-based position in the Phase-2 output stream. Set by the runner
    /// just before Phase-2 starts so nodes can emit "current / total"
    /// progress that matches the sequential Phase-2 order (rather than the
    /// parallel Phase-1 order).
    pub processed_ordinal: usize,
    pub outcomes: Vec<NodeOutcome>,
}

impl FileContext {
    pub fn new(file_path: PathBuf, input_index: usize, total: usize) -> Self {
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            file_path,
            file_name,
            input_index,
            total,
            mime: None,
            extracted_metadata: Vec::new(),
            suggested_author_names: Vec::new(),
            content_sample: None,
            cover: None,
            archive_temp_dir: None,
            archive_entries: Vec::new(),
            cover_candidates: Vec::new(),
            display_name: None,
            progress: None,
            category_id: None,
            tag_ids: Vec::new(),
            suggested_tags: Vec::new(),
            author_ids: Vec::new(),
            unresolved_authors: Vec::new(),
            duplicate_of: None,
            processed_ordinal: 0,
            outcomes: Vec::new(),
        }
    }

    pub fn record(&mut self, name: &'static str, status: NodeStatus) {
        self.outcomes.push(NodeOutcome { name, status });
    }

    /// Look up the recorded status for a specific node. Used by
    /// StatusEmitNode to read FilenameLlmNode / ContentLlmNode results.
    pub fn outcome_of(&self, name: &str) -> Option<&NodeStatus> {
        self.outcomes
            .iter()
            .find(|o| o.name == name)
            .map(|o| &o.status)
    }
}
