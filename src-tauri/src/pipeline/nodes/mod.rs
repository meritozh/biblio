//! Node implementations and the stock pipeline compositions.
//!
//! Each node is a small, self-contained unit responsible for one step of
//! the import flow. There are two stock pipelines, dispatched by the
//! command layer based on file extension: `novel_pipeline()` for plain
//! text (.txt) and `comic_pipeline()` for archives
//! (.cbz / .zip / .cbr / .rar) AND directories of loose images. Many
//! nodes (mime detect, filename LLM, author resolve, duplicate detect,
//! status emit) appear in both — keeping them as separate compositions
//! makes the per-type node list explicit at the call site instead of
//! hidden behind per-node `applies()` gates. Inside `comic_pipeline()`,
//! the archive- vs folder-flavored filename extraction nodes coexist
//! and self-select via `applies()`.

mod archive_cover;
mod archive_list;
mod author_resolve;
mod content_llm;
mod content_sample;
mod cover_candidates_llm;
mod cover_compress;
mod duplicate_detect;
mod filename_llm;
mod mime;
mod parent_dir_author;
mod status_emit;
mod vision_cover_llm;

pub use archive_cover::ArchiveFirstImageCoverNode;
pub use archive_list::ArchiveListImagesNode;
pub use author_resolve::AuthorResolveNode;
pub use content_llm::ContentLlmNode;
pub use content_sample::{decode_to_utf8, sample_from_text, ContentSampleNode};
pub use cover_candidates_llm::LlmCoverCandidatesNode;
pub use cover_compress::CoverCompressNode;
pub use duplicate_detect::DbDuplicateDetectNode;
pub use filename_llm::FilenameLlmNode;
pub use mime::MimeDetectNode;
pub use parent_dir_author::ParentDirAuthorHintNode;
pub use status_emit::StatusEmitNode;
pub use vision_cover_llm::LlmVisionCoverCheckNode;

use std::path::Path;

use super::runner::{Pipeline, PipelineBuilder};

/// File-type kinds the dispatcher routes between. Determined from the
/// path extension, not the MIME magic bytes — magic-byte detection is
/// the pipeline's job once the file is on the right path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Comic,
    Novel,
    Galgame,
}

const COMIC_EXTS: &[&str] = &["cbz", "zip", "cbr", "rar"];
const NOVEL_EXTS: &[&str] = &["txt"];
/// Galgame archives. `.zip` overlaps with comics, but category-first import
/// disambiguates by the chosen category's schema — extension routing only
/// applies to legacy no-category imports, which never target galgame.
const GALGAME_EXTS: &[&str] = &["zip", "7z", "rar"];

impl FileKind {
    /// The pipeline kind a category's schema expects. Category-first import
    /// uses this to pick the pipeline and to validate that the dropped input
    /// matches the chosen category. Exhaustive over `SchemaSlug` so a new
    /// schema variant forces a decision here.
    pub fn for_schema(slug: crate::schema::SchemaSlug) -> Self {
        use crate::schema::SchemaSlug;
        match slug {
            SchemaSlug::Novel => FileKind::Novel,
            SchemaSlug::Comic => FileKind::Comic,
            SchemaSlug::Galgame => FileKind::Galgame,
        }
    }

    /// Whether `path` is a valid input shape for this pipeline kind. Used by
    /// category-first import as a validator. Unlike `kind_for_path` (which
    /// returns one answer), this resolves the comic/galgame `.zip`+folder
    /// overlap: the chosen category already fixes the kind, so we only check
    /// the input *fits* it.
    pub fn accepts_path(self, path: &Path) -> bool {
        let is_dir = path.is_dir();
        match self {
            // Novels are single text files, never directories.
            FileKind::Novel => !is_dir && has_ext(path, NOVEL_EXTS),
            // Comics + galgames accept a directory (zipped on commit) or an
            // archive in their respective extension sets.
            FileKind::Comic => is_dir || has_ext(path, COMIC_EXTS),
            FileKind::Galgame => is_dir || has_ext(path, GALGAME_EXTS),
        }
    }
}

/// Lowercased-extension membership test shared by `kind_for_path` and
/// `FileKind::accepts_path`.
fn has_ext(path: &Path, exts: &[&str]) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    exts.iter().any(|e| *e == ext)
}

/// Pick the pipeline kind for a path. Returns `None` for any extension
/// outside the supported set so the dispatcher can reject unsupported
/// inputs explicitly instead of silently falling through.
///
/// Directories are routed to the comic pipeline: the only directory
/// inputs that reach this point come from `list_files_in_folder`'s
/// image-only-subtree collapse, so they're always image-folder comics.
pub fn kind_for_path(path: &Path) -> Option<FileKind> {
    if path.is_dir() {
        return Some(FileKind::Comic);
    }
    if has_ext(path, COMIC_EXTS) {
        Some(FileKind::Comic)
    } else if has_ext(path, NOVEL_EXTS) {
        Some(FileKind::Novel)
    } else {
        None
    }
}

/// Pipeline for plain text novels (.txt). MIME detect runs as a sanity
/// check; content sampling feeds the LLM classifier.
pub fn novel_pipeline() -> PipelineBuilder {
    Pipeline::builder()
        // ── Phase 1 — disk / CPU ─────────────────────────────────────
        .add_phase1(MimeDetectNode)
        .add_phase1(ContentSampleNode)
        // ── Phase 2 — LLM / DB ───────────────────────────────────────
        .add_phase2(FilenameLlmNode::text())
        .add_phase2(ContentLlmNode)
        .add_phase2(CoverCompressNode)
        .add_phase2(AuthorResolveNode)
        .add_phase2(DbDuplicateDetectNode)
        .add_phase2(StatusEmitNode)
}

/// Pipeline for comic inputs — both archive files (.cbz / .zip / .cbr /
/// .rar) AND directories of loose images (which `file_create` zips on
/// commit). Adds the list → LLM-ranked candidates → vision check chain
/// on top of the shared filename / author / dedupe stack. The vision
/// node reads candidate bytes lazily through the archive abstraction,
/// so the same nodes work against both source kinds with no temp-dir
/// extraction.
///
/// Source-conditional nodes: both `FilenameLlmNode::archive()` and
/// `FilenameLlmNode::folder()` are listed; their `applies()` gates pick
/// exactly one per file based on `ctx.file_path.is_dir()`. The other is
/// recorded as `Skipped`. Cost is one no-op `applies()` check per file.
/// Rule of thumb: keep using `applies()`-based switching while ≤ 2
/// nodes differ; split into separate compositions if 3+ diverge.
pub fn comic_pipeline() -> PipelineBuilder {
    Pipeline::builder()
        // ── Phase 1 — disk / CPU ─────────────────────────────────────
        .add_phase1(MimeDetectNode)
        // Fallback cover (alphabetical first image) runs first so it
        // provides a baseline; the Phase-2 vision path may override.
        .add_phase1(ArchiveFirstImageCoverNode)
        // Records (basename, archive_index) for every image entry only
        // when the LLM is enabled — the vision node reads bytes on
        // demand.
        .add_phase1(ArchiveListImagesNode)
        // Folder imports often nest as <root>/<author>/<work>.cbz; seed
        // the parent dir name as an author candidate so AuthorResolveNode
        // can match it against the existing author_map.
        .add_phase1(ParentDirAuthorHintNode)
        // ── Phase 2 — LLM / DB ───────────────────────────────────────
        // Filename extraction — exactly one of the two fires per file.
        // archive() handles real archives; folder() handles directory
        // inputs and uses a dedicated prompt that does NOT extract
        // authors (those come from the picked-root LLM cleanup).
        .add_phase2(FilenameLlmNode::archive())
        .add_phase2(FilenameLlmNode::folder())
        .add_phase2(LlmCoverCandidatesNode)
        .add_phase2(LlmVisionCoverCheckNode)
        .add_phase2(CoverCompressNode)
        .add_phase2(AuthorResolveNode)
        .add_phase2(DbDuplicateDetectNode)
        .add_phase2(StatusEmitNode)
}

/// Pipeline for galgame inputs — archives (.zip / .7z / .rar) AND directories
/// (which `file_create` zips on commit, like comics). Composed from the same
/// reusable node library, NOT by calling `comic_pipeline()`. Phase B carries
/// only the shared mime / dedupe / status stack; the filename-LLM and VNDB
/// search nodes are added in a later phase. Cover/author enrichment comes from
/// the user-confirmed VNDB match at commit time, so no CoverCompress or
/// AuthorResolve here (their inputs are never set during a galgame run).
pub fn galgame_pipeline() -> PipelineBuilder {
    Pipeline::builder()
        // ── Phase 1 — disk / CPU ─────────────────────────────────────
        .add_phase1(MimeDetectNode)
        // ── Phase 2 — LLM / DB ───────────────────────────────────────
        // Cleans the archive/folder name into `ctx.display_name` so the
        // review UI seeds its VNDB search with a real title instead of the
        // raw `[Brand] Game (2020) [DL版]` filename. Skipped when the LLM is
        // disabled — the frontend then seeds the search from `file_name`.
        .add_phase2(FilenameLlmNode::galgame())
        .add_phase2(DbDuplicateDetectNode)
        .add_phase2(StatusEmitNode)
}
