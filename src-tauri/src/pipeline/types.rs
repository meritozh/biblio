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
    /// On-disk byte length of the existing file. None when the file is
    /// missing (file_status='missing'), the path is unreadable, or for
    /// uncached remote rows where the local copy doesn't exist. The
    /// comparison panel renders None as "—" so the user knows we couldn't
    /// resolve a size, vs. "0 B" which is a real zero-byte file.
    pub existing_size: Option<i64>,
    /// Byte length of the file being imported. None when the source is a
    /// directory (folder-to-zip imports have no single-file size yet) or
    /// the path is unreadable.
    pub new_size: Option<i64>,
    /// Author names attached to the existing row. Denormalized into this
    /// DTO so the dupe compare panel can render side-by-side authors
    /// without a follow-up IPC round-trip. Empty vec when the existing
    /// file has no authors.
    pub existing_author_names: Vec<String>,
    pub recommendation: DuplicateAction,
}

#[derive(Debug, Clone)]
pub struct Cover {
    pub data: Vec<u8>,
    pub mime_type: String,
}

/// One image entry inside a comic archive. The LLM sees `basename` when
/// ranking cover candidates; the vision node reads bytes lazily by re-
/// opening the archive and indexing into it via `archive_index`.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub basename: String,
    pub archive_index: usize,
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
    /// LLM-cleaned author candidates derived from the user-picked folder
    /// name (e.g. `[作者] 系列` → `["作者"]`). Filled per file in
    /// `file_prepare_import` (one LLM call per unique folder root, fanned
    /// out across files) and consumed by `ParentDirAuthorHintNode`. Empty
    /// for single-file picks, drag-drop, the trivial folder-to-zip case
    /// (the picked folder IS the comic), or when the LLM is disabled and
    /// the extractor produced nothing.
    pub parent_author_candidates: Vec<String>,
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
    /// Image entries enumerated from the archive (basename + zip index),
    /// preserving zip order so "first = cover" heuristics match what
    /// readers display. Bytes are read on demand by the vision node.
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
    pub fn new(
        file_path: PathBuf,
        input_index: usize,
        total: usize,
        parent_author_candidates: Vec<String>,
    ) -> Self {
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Self {
            file_path,
            file_name,
            parent_author_candidates,
            input_index,
            total,
            mime: None,
            extracted_metadata: Vec::new(),
            suggested_author_names: Vec::new(),
            content_sample: None,
            cover: None,
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

    /// Key used by `processing-progress` events. The frontend matches each
    /// event against the placeholder row's `path` (the full absolute path it
    /// was created with), so progress events MUST carry the full path, not the
    /// basename — otherwise none of them land and the StatusEmitNode verdict
    /// (`partial`/`error`) is lost. `file-prepared` already keys on the full
    /// path; this keeps the contract consistent.
    pub fn event_key(&self) -> String {
        self.file_path.to_string_lossy().to_string()
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
