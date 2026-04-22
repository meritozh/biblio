//! Node implementations and the stock pipeline compositions.
//!
//! Each node is a small, self-contained unit responsible for one step of
//! the import flow. The `novel_and_generic` composition reproduces the
//! behavior of the old monolithic `file_prepare_import` exactly; the
//! comic and remote variants add their own nodes on top of the shared
//! Phase-1 set.

mod archive_cover;
mod author_resolve;
mod content_llm;
mod content_sample;
mod duplicate_detect;
mod exif;
mod filename_llm;
mod mime;
mod pdf_meta;
mod status_emit;

pub use archive_cover::{ArchiveFirstImageCoverNode, SingleImageCoverNode};
pub use author_resolve::AuthorResolveNode;
pub use content_llm::ContentLlmNode;
pub use content_sample::{ContentSampleNode, is_novel_file};
pub use duplicate_detect::DbDuplicateDetectNode;
pub use exif::ExifNode;
pub use filename_llm::FilenameLlmNode;
pub use mime::MimeDetectNode;
pub use pdf_meta::PdfMetaNode;
pub use status_emit::StatusEmitNode;

use super::runner::{Pipeline, PipelineBuilder};

/// Default composition that covers novels, generic files, and comic-image
/// single-file imports. Matches the behavior of the pre-refactor
/// `file_prepare_import`. Returns a `PipelineBuilder` so callers can
/// append command-layer nodes (e.g. a "file-prepared" event emitter)
/// before building.
pub fn novel_and_generic() -> PipelineBuilder {
    Pipeline::builder()
        // ── Phase 1 — disk / CPU ─────────────────────────────────────
        .add_phase1(MimeDetectNode)
        .add_phase1(PdfMetaNode)
        .add_phase1(ExifNode)
        .add_phase1(ContentSampleNode)
        .add_phase1(ArchiveFirstImageCoverNode)
        .add_phase1(SingleImageCoverNode)
        // ── Phase 2 — LLM / DB ───────────────────────────────────────
        .add_phase2(FilenameLlmNode)
        .add_phase2(ContentLlmNode)
        .add_phase2(AuthorResolveNode)
        .add_phase2(DbDuplicateDetectNode)
        .add_phase2(StatusEmitNode)
}
