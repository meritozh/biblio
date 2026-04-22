//! Node implementations and the stock pipeline compositions.
//!
//! Each node is a small, self-contained unit responsible for one step of
//! the import flow. The `novel_and_generic` composition reproduces the
//! behavior of the old monolithic `file_prepare_import` exactly; the
//! comic and remote variants add their own nodes on top of the shared
//! Phase-1 set.

mod archive_cover;
mod archive_unzip;
mod author_resolve;
mod cleanup_temp_dir;
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
pub use archive_unzip::ArchiveUnzipNode;
pub use author_resolve::AuthorResolveNode;
pub use cleanup_temp_dir::CleanupTempDirNode;
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

use super::runner::{Pipeline, PipelineBuilder};

/// Default composition that handles every category biblio imports today —
/// novels, generic files, single-image comics, and archive comics — via
/// per-node `applies()` gates. Comic archives follow the same order but
/// pick up additional nodes (unzip → LLM-ranked candidates → vision check
/// → compress → cleanup). Returns a `PipelineBuilder` so callers can
/// append command-layer nodes (e.g. a "file-prepared" event emitter)
/// before building.
pub fn default_pipeline() -> PipelineBuilder {
    Pipeline::builder()
        // ── Phase 1 — disk / CPU ─────────────────────────────────────
        .add_phase1(MimeDetectNode)
        .add_phase1(PdfMetaNode)
        .add_phase1(ExifNode)
        .add_phase1(ContentSampleNode)
        // Archive fallback cover (first image alphabetically) runs first
        // so it provides a baseline; the Phase-2 vision path may override.
        .add_phase1(ArchiveFirstImageCoverNode)
        // Unzips archive images to a temp dir only when LLM is enabled —
        // enabling the vision-ranking path downstream.
        .add_phase1(ArchiveUnzipNode)
        .add_phase1(SingleImageCoverNode)
        // ── Phase 2 — LLM / DB ───────────────────────────────────────
        .add_phase2(FilenameLlmNode)
        .add_phase2(ContentLlmNode)
        .add_phase2(LlmCoverCandidatesNode)
        .add_phase2(LlmVisionCoverCheckNode)
        .add_phase2(CoverCompressNode)
        .add_phase2(AuthorResolveNode)
        .add_phase2(DbDuplicateDetectNode)
        .add_phase2(CleanupTempDirNode)
        .add_phase2(StatusEmitNode)
}

/// Backwards-compatible alias — kept so callers written during Phase 0
/// continue to work. New code should use `default_pipeline`.
pub fn novel_and_generic() -> PipelineBuilder {
    default_pipeline()
}
