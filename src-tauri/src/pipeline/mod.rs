//! Composable import pipeline.
//!
//! Each file passes through a list of Phase-1 (blocking, CPU/disk) nodes
//! and a list of Phase-2 (async, LLM/DB) nodes. Nodes read and write a
//! shared `FileContext`; the composition replaces the hard-coded branches
//! in the old `file_prepare_import`. Novel and comic paths share the same
//! runner — they differ only in which node list is built.

mod env;
pub mod nodes;
mod runner;
mod traits;
mod types;

pub use env::{PipelineEnv, PipelineSettings};
pub use traits::{Phase1Node, Phase2Node};
pub use types::{
    ArchiveEntry, Cover, DuplicateAction, DuplicateInfo, ExtractedField, FileContext, NodeError,
    NodeStatus,
};
