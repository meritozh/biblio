use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use tauri::Emitter;

use super::env::PipelineEnv;
use super::traits::{Phase1Node, Phase2Node};
use super::types::{FileContext, NodeStatus, ProcessingProgress};

/// Cap concurrent Phase-1 (disk-bound) tasks. Large imports would otherwise
/// spawn hundreds of blocking threads simultaneously — EPUB/ZIP parsing and
/// cover reads — and thrash the disk / spike memory.
const PHASE1_CONCURRENCY: usize = 8;

pub type Phase1List = Arc<[Box<dyn Phase1Node>]>;
pub type Phase2List = Arc<[Box<dyn Phase2Node>]>;

/// A composed import pipeline. Phase-1 nodes run concurrently per-file on
/// the blocking pool; Phase-2 nodes run sequentially per-file on the
/// tokio runtime. Within a file, nodes execute in the order they were
/// added.
pub struct Pipeline {
    phase1: Phase1List,
    phase2: Phase2List,
}

impl Pipeline {
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder {
            phase1: Vec::new(),
            phase2: Vec::new(),
        }
    }

    /// Run every path through both phases. Phase-1 work for different files
    /// overlaps via an internal semaphore; Phase-2 work is drained
    /// sequentially through an mpsc channel so downstream event ordering
    /// matches the "current N of total" display in the frontend.
    /// Run every path through both phases. `parent_author_candidates_by_path`
    /// supplies the per-file folder-name-derived author hint (filled by
    /// `file_prepare_import` once per unique picked root). Paths missing
    /// from the map get an empty hint, matching the non-folder-pick case.
    pub async fn run_batch(
        &self,
        paths: Vec<PathBuf>,
        env: Arc<PipelineEnv>,
        mut parent_author_candidates_by_path: HashMap<PathBuf, Vec<String>>,
    ) -> Vec<FileContext> {
        let total = paths.len();
        if total == 0 {
            return Vec::new();
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel::<FileContext>(total);
        let sem = Arc::new(tokio::sync::Semaphore::new(PHASE1_CONCURRENCY));

        let phase1 = Arc::clone(&self.phase1);
        let dispatch_env = Arc::clone(&env);

        tokio::spawn(async move {
            for (idx, path) in paths.into_iter().enumerate() {
                let Ok(permit) = sem.clone().acquire_owned().await else {
                    break;
                };
                let tx = tx.clone();
                let phase1 = Arc::clone(&phase1);
                let env = Arc::clone(&dispatch_env);

                let candidates_for_task = parent_author_candidates_by_path
                    .remove(&path)
                    .unwrap_or_default();
                tauri::async_runtime::spawn_blocking(move || {
                    let _permit = permit;
                    let mut ctx = FileContext::new(
                        path,
                        idx,
                        total,
                        candidates_for_task,
                    );

                    emit_progress(&env.app, idx + 1, total, &ctx.file_name, "gathering_signals");

                    for node in phase1.iter() {
                        if !node.applies(&ctx, &env) {
                            ctx.record(node.name(), NodeStatus::Skipped);
                            continue;
                        }
                        match node.run(&mut ctx, &env) {
                            Ok(()) => ctx.record(node.name(), NodeStatus::Ok),
                            Err(e) => ctx.record(node.name(), NodeStatus::Err(e.0)),
                        }
                    }

                    let _ = tx.blocking_send(ctx);
                });
            }
            // tx drops here; Phase-2 drains and the while-let exits.
        });

        let mut results: Vec<FileContext> = Vec::new();
        while let Some(mut ctx) = rx.recv().await {
            if env.cancelled.load(Ordering::Relaxed) {
                break;
            }
            ctx.processed_ordinal = results.len() + 1;

            for node in self.phase2.iter() {
                if env.cancelled.load(Ordering::Relaxed) {
                    break;
                }
                if !node.applies(&ctx, &env) {
                    ctx.record(node.name(), NodeStatus::Skipped);
                    continue;
                }
                match node.run(&mut ctx, &env).await {
                    Ok(()) => ctx.record(node.name(), NodeStatus::Ok),
                    Err(e) => ctx.record(node.name(), NodeStatus::Err(e.0)),
                }
            }

            results.push(ctx);
        }

        // Diagnostic — reports abnormal exits (e.g. user-triggered cancel).
        // The old monolithic handler used the same line; keep it to avoid
        // breaking any log-watching in dev.
        eprintln!(
            "pipeline run_batch finished — {} of {} files processed, cancelled={}",
            results.len(),
            total,
            env.cancelled.load(Ordering::Relaxed)
        );

        results
    }
}

pub struct PipelineBuilder {
    phase1: Vec<Box<dyn Phase1Node>>,
    phase2: Vec<Box<dyn Phase2Node>>,
}

impl PipelineBuilder {
    pub fn add_phase1<N: Phase1Node + 'static>(mut self, node: N) -> Self {
        self.phase1.push(Box::new(node));
        self
    }

    pub fn add_phase2<N: Phase2Node + 'static>(mut self, node: N) -> Self {
        self.phase2.push(Box::new(node));
        self
    }

    pub fn build(self) -> Pipeline {
        Pipeline {
            phase1: self.phase1.into(),
            phase2: self.phase2.into(),
        }
    }
}

/// Helper available to nodes that need to emit an intermediate progress
/// event. Thin wrapper so we don't repeat the construction boilerplate.
pub fn emit_progress(
    app: &tauri::AppHandle,
    current: usize,
    total: usize,
    file_name: &str,
    status: &str,
) {
    let _ = app.emit(
        "processing-progress",
        &ProcessingProgress {
            current,
            total,
            current_file: file_name.to_string(),
            status: status.to_string(),
        },
    );
}
