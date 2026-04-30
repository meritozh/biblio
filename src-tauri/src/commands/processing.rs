use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};

use crate::ProcessingCancelled;
use crate::pipeline::{
    self, FileContext, NodeError, Phase2Node, PipelineEnv, PipelineSettings,
};

// Re-export the pipeline types that appear in FilePreparedImport's serde
// shape. Keeping them under this path preserves existing call sites; new
// code can import them from crate::pipeline directly.
pub use crate::pipeline::{DuplicateInfo, ExtractedField};

/// The per-file result returned by `file_prepare_import` and streamed to the
/// frontend via the `file-prepared` event. Serde shape must stay stable.
#[derive(Debug, Clone, Serialize)]
pub struct FilePreparedImport {
    pub path: String,
    pub file_name: String,
    pub display_name: String,
    pub category_id: Option<i64>,
    pub tag_ids: Vec<i64>,
    pub author_ids: Vec<i64>,
    pub metadata: Vec<ExtractedField>,
    pub unresolved_author_names: Vec<String>,
    /// Base64-encoded cover bytes. Serialized as a string so the frontend
    /// can drop it straight into a `data:` URL without converting a
    /// JS-side number array first.
    pub cover_data: Option<String>,
    pub cover_mime_type: Option<String>,
    pub progress: Option<String>,
    pub suggested_tags: Vec<String>,
    pub duplicate_of: Option<DuplicateInfo>,
    pub batch_duplicate_group: Option<String>,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

async fn load_bool_setting(pool: &sqlx::SqlitePool, key: &str, default: bool) -> bool {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    match row {
        Some((v,)) => matches!(v.as_str(), "1" | "true" | "True" | "TRUE"),
        None => default,
    }
}

#[tauri::command]
pub async fn cancel_processing(app: tauri::AppHandle) {
    let cancelled = app.state::<ProcessingCancelled>();
    cancelled.0.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub async fn file_prepare_import(
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<Vec<FilePreparedImport>, String> {
    use super::{Author, Category, FileEntry, Tag};

    // Reset the cancellation flag at the start of every batch so a cancel
    // from a previous import doesn't poison this one.
    let cancelled_state = app.state::<ProcessingCancelled>();
    cancelled_state.0.store(false, Ordering::Relaxed);
    let cancelled: Arc<std::sync::atomic::AtomicBool> = Arc::clone(&cancelled_state.0);

    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Fetch every piece of shared state the pipeline needs, up front, so
    // per-file nodes don't hit the DB in their hot paths.
    let categories: Vec<Category> = sqlx::query_as(
        "SELECT id, name, description, icon, is_default, folder_name, created_at FROM categories",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to load categories: {e}"))?;

    let authors: Vec<Author> = sqlx::query_as("SELECT id, name, created_at FROM authors")
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to load authors: {e}"))?;

    let tags: Vec<Tag> = sqlx::query_as("SELECT id, name, color, created_at FROM tags")
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to load tags: {e}"))?;

    let existing_files: Vec<FileEntry> = sqlx::query_as(
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, created_at, updated_at FROM files",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to load existing files: {e}"))?;

    let category_map: HashMap<String, i64> = categories
        .iter()
        .map(|c| (c.name.to_lowercase(), c.id))
        .collect();
    let author_map: HashMap<String, i64> =
        authors.iter().map(|a| (a.name.to_lowercase(), a.id)).collect();
    let tag_map: HashMap<String, i64> =
        tags.iter().map(|t| (t.name.to_lowercase(), t.id)).collect();

    let category_names: Vec<String> = categories
        .iter()
        .map(|c| match &c.description {
            Some(desc) if !desc.is_empty() => format!("{} ({})", c.name, desc),
            _ => c.name.clone(),
        })
        .collect();
    let tag_names: Vec<String> = tags.iter().map(|t| t.name.clone()).collect();

    let llm_config = super::llm::load_config(&pool).await?;
    let analyze_content = llm_config.analyze_content;
    let process_novel_epub = load_bool_setting(&pool, "process_novel_epub", true).await;
    let process_novel_pdf = load_bool_setting(&pool, "process_novel_pdf", false).await;

    let env = Arc::new(PipelineEnv {
        pool,
        llm_config,
        app: app.clone(),
        cancelled,
        category_map,
        author_map,
        tag_map,
        category_names,
        tag_names,
        existing_files,
        settings: PipelineSettings {
            process_novel_epub,
            process_novel_pdf,
            analyze_content,
        },
    });

    // EmitPreparedNode is command-layer: it knows how to convert a
    // FileContext to the command's public FilePreparedImport shape, which
    // the pipeline module is deliberately agnostic about.
    //
    // Two pipelines, one dispatcher: extension picks novel vs comic.
    // We still want streaming `file-prepared` events ordered by completion,
    // so EmitPreparedNode is appended to BOTH compositions.
    let novel_pipeline = pipeline::nodes::novel_pipeline()
        .add_phase2(EmitPreparedNode)
        .build();
    let comic_pipeline = pipeline::nodes::comic_pipeline()
        .add_phase2(EmitPreparedNode)
        .build();

    let mut novel_paths: Vec<PathBuf> = Vec::new();
    let mut comic_paths: Vec<PathBuf> = Vec::new();
    for raw in paths {
        let pb = PathBuf::from(raw);
        match pipeline::nodes::kind_for_path(&pb) {
            pipeline::nodes::FileKind::Comic => comic_paths.push(pb),
            pipeline::nodes::FileKind::Novel => novel_paths.push(pb),
        }
    }

    // Run sequentially: both pipelines emit `processed_ordinal` starting
    // from 1, so concurrent execution would surface two interleaved
    // counters in the UI. Empty path lists short-circuit `run_batch`, so
    // the common single-type import pays nothing for the unused branch.
    let novel_ctxs = novel_pipeline
        .run_batch(novel_paths, Arc::clone(&env))
        .await;
    let comic_ctxs = comic_pipeline
        .run_batch(comic_paths, Arc::clone(&env))
        .await;

    let mut results: Vec<FilePreparedImport> = novel_ctxs
        .into_iter()
        .chain(comic_ctxs.into_iter())
        .map(prepared_from_ctx)
        .collect();

    // Phase 3 — batch-level duplicate detection (same display_name across
    // files being imported together). Runs on the collected results so the
    // grouping is stable across streaming order.
    detect_batch_duplicates(&mut results);

    Ok(results)
}

/// Build a FilePreparedImport from an owned FileContext at the end of the
/// batch. Also used (cloned version) by EmitPreparedNode for the per-file
/// streaming emission.
fn prepared_from_ctx(ctx: FileContext) -> FilePreparedImport {
    use base64::Engine;
    let (cover_data, cover_mime_type) = match ctx.cover {
        Some(c) => (
            Some(base64::engine::general_purpose::STANDARD.encode(&c.data)),
            Some(c.mime_type),
        ),
        None => (None, None),
    };
    let display_name = ctx.display_name.unwrap_or_else(|| ctx.file_name.clone());
    FilePreparedImport {
        path: ctx.file_path.to_string_lossy().to_string(),
        file_name: ctx.file_name,
        display_name,
        category_id: ctx.category_id,
        tag_ids: ctx.tag_ids,
        author_ids: ctx.author_ids,
        metadata: ctx.extracted_metadata,
        unresolved_author_names: ctx.unresolved_authors,
        cover_data,
        cover_mime_type,
        progress: ctx.progress,
        suggested_tags: ctx.suggested_tags,
        duplicate_of: ctx.duplicate_of,
        batch_duplicate_group: None,
    }
}

/// Streaming-emission variant that clones the cover bytes so the
/// FileContext can be reused downstream. Called from EmitPreparedNode.
fn prepared_from_ctx_ref(ctx: &FileContext) -> FilePreparedImport {
    use base64::Engine;
    let (cover_data, cover_mime_type) = match ctx.cover.as_ref() {
        Some(c) => (
            Some(base64::engine::general_purpose::STANDARD.encode(&c.data)),
            Some(c.mime_type.clone()),
        ),
        None => (None, None),
    };
    let display_name = ctx
        .display_name
        .clone()
        .unwrap_or_else(|| ctx.file_name.clone());
    FilePreparedImport {
        path: ctx.file_path.to_string_lossy().to_string(),
        file_name: ctx.file_name.clone(),
        display_name,
        category_id: ctx.category_id,
        tag_ids: ctx.tag_ids.clone(),
        author_ids: ctx.author_ids.clone(),
        metadata: ctx.extracted_metadata.clone(),
        unresolved_author_names: ctx.unresolved_authors.clone(),
        cover_data,
        cover_mime_type,
        progress: ctx.progress.clone(),
        suggested_tags: ctx.suggested_tags.clone(),
        duplicate_of: ctx.duplicate_of.clone(),
        batch_duplicate_group: None,
    }
}

/// Phase-2 node that emits the streaming `file-prepared` event as each
/// file finishes. Defined in the command layer because the pipeline is
/// intentionally agnostic of `FilePreparedImport`.
struct EmitPreparedNode;

#[async_trait]
impl Phase2Node for EmitPreparedNode {
    fn name(&self) -> &'static str {
        "EmitPrepared"
    }

    async fn run(&self, ctx: &mut FileContext, env: &PipelineEnv) -> Result<(), NodeError> {
        let prepared = prepared_from_ctx_ref(ctx);
        let _ = env.app.emit("file-prepared", &prepared);
        Ok(())
    }
}

/// Tag each file in a batch that shares a display_name with another file
/// so the frontend can group and warn about same-batch duplicates.
fn detect_batch_duplicates(results: &mut [FilePreparedImport]) {
    let mut name_groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, result) in results.iter().enumerate() {
        let normalized = result.display_name.trim().to_lowercase();
        name_groups.entry(normalized).or_default().push(idx);
    }

    for (name, indices) in &name_groups {
        if indices.len() > 1 {
            let group_id = format!("batch_{}", name);
            for &idx in indices {
                results[idx].batch_duplicate_group = Some(group_id.clone());
            }
        }
    }
}
