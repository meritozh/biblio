//! Node implementations and the stock pipeline compositions.
//!
//! Each node is a small, self-contained unit responsible for one step of
//! the import flow. There are two stock pipelines, dispatched by the
//! command layer based on file extension: `novel_pipeline()` for text
//! files (.txt / .epub / .pdf) and `comic_pipeline()` for archives
//! (.cbz / .zip). Many nodes (mime detect, filename LLM, author resolve,
//! duplicate detect, status emit) appear in both — keeping them as
//! separate compositions makes the per-type node list explicit at the
//! call site instead of hidden behind per-node `applies()` gates.

mod archive_cover;
mod archive_list;
mod author_resolve;
mod content_llm;
mod content_sample;
mod cover_candidates_llm;
mod cover_compress;
mod duplicate_detect;
mod exif;
mod filename_llm;
mod mime;
mod pdf_meta;
mod status_emit;
mod vision_cover_llm;

pub use archive_cover::{ArchiveFirstImageCoverNode, SingleImageCoverNode};
pub use archive_list::ArchiveListImagesNode;
pub use author_resolve::AuthorResolveNode;
pub use content_llm::ContentLlmNode;
pub use content_sample::ContentSampleNode;
pub use cover_candidates_llm::LlmCoverCandidatesNode;
pub use cover_compress::CoverCompressNode;
pub use duplicate_detect::DbDuplicateDetectNode;
pub use exif::ExifNode;
pub use filename_llm::FilenameLlmNode;
pub use mime::MimeDetectNode;
pub use pdf_meta::PdfMetaNode;
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
}

const COMIC_EXTS: &[&str] = &["cbz", "zip"];
const NOVEL_EXTS: &[&str] = &["txt", "epub", "pdf"];

/// Pick the pipeline kind for a path. Anything that isn't a recognized
/// archive falls into the novel/generic pipeline — single images, plain
/// text, etc. — which matches the pre-split behavior.
pub fn kind_for_path(path: &Path) -> FileKind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    if COMIC_EXTS.iter().any(|e| *e == ext) {
        FileKind::Comic
    } else if NOVEL_EXTS.iter().any(|e| *e == ext) {
        FileKind::Novel
    } else {
        FileKind::Novel
    }
}

/// Pipeline for text files (.txt / .epub / .pdf) and other non-archive
/// inputs. Skips every archive-specific node.
pub fn novel_pipeline() -> PipelineBuilder {
    Pipeline::builder()
        // ── Phase 1 — disk / CPU ─────────────────────────────────────
        .add_phase1(MimeDetectNode)
        .add_phase1(PdfMetaNode)
        .add_phase1(ExifNode)
        .add_phase1(ContentSampleNode)
        .add_phase1(SingleImageCoverNode)
        // ── Phase 2 — LLM / DB ───────────────────────────────────────
        .add_phase2(FilenameLlmNode::text())
        .add_phase2(ContentLlmNode)
        .add_phase2(CoverCompressNode)
        .add_phase2(AuthorResolveNode)
        .add_phase2(DbDuplicateDetectNode)
        .add_phase2(StatusEmitNode)
}

/// Pipeline for archive files (.cbz / .zip). Adds the list → LLM-ranked
/// candidates → vision check chain on top of the shared filename /
/// author / dedupe stack. The vision node reads candidate bytes lazily
/// from the source archive, so no temp-dir extraction or cleanup is
/// needed.
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
        // ── Phase 2 — LLM / DB ───────────────────────────────────────
        .add_phase2(FilenameLlmNode::archive())
        .add_phase2(LlmCoverCandidatesNode)
        .add_phase2(LlmVisionCoverCheckNode)
        .add_phase2(CoverCompressNode)
        .add_phase2(AuthorResolveNode)
        .add_phase2(DbDuplicateDetectNode)
        .add_phase2(StatusEmitNode)
}
