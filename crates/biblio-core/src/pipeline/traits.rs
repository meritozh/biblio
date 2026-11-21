use async_trait::async_trait;

use super::env::PipelineEnv;
use super::types::{FileContext, NodeError};

/// A Phase-1 node: synchronous, CPU/disk bound, runs on the blocking pool.
/// Phase-1 nodes must not touch the DB or the network — the `PipelineEnv`
/// is present only for access to category/tag/author maps and settings.
pub trait Phase1Node: Send + Sync {
    fn name(&self) -> &'static str;

    fn applies(&self, _ctx: &FileContext, _env: &PipelineEnv) -> bool {
        true
    }

    fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError>;
}

/// A Phase-2 node: async, runs on the tokio runtime. DB queries and LLM
/// calls are permitted. The runner checks cancellation *between* Phase-2
/// nodes, so individual nodes don't need to.
#[async_trait]
pub trait Phase2Node: Send + Sync {
    fn name(&self) -> &'static str;

    fn applies(&self, _ctx: &FileContext, _env: &PipelineEnv) -> bool {
        true
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError>;
}
