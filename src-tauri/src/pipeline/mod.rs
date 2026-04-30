//! Composable import pipeline.
//!
//! Each file passes through a list of Phase-1 (blocking, CPU/disk) nodes
//! and a list of Phase-2 (async, LLM/DB) nodes. Nodes read and write a
//! shared `FileContext`. Two stock pipelines exist — `novel_pipeline` for
//! text inputs and `comic_pipeline` for archives. The command layer picks
//! between them per input path via `kind_for_path`. Many node types
//! appear in both compositions; that overlap is expressed at the call
//! site rather than hidden behind per-node `applies()` gates.

mod env;
pub mod nodes;
pub mod runner;
mod traits;
mod types;

pub use env::{PipelineEnv, PipelineSettings};
pub use traits::{Phase1Node, Phase2Node};
pub use types::{
    ArchiveEntry, Cover, DuplicateAction, DuplicateInfo, ExtractedField, FileContext, NodeError,
    NodeStatus,
};
