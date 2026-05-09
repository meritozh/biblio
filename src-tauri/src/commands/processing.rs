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
use crate::services::import_worker::{ImportJob, ImportQueueSender};

// Re-export the pipeline types that appear in FilePreparedImport's serde
// shape. Keeping them under this path preserves existing call sites; new
// code can import them from crate::pipeline directly.
pub use crate::pipeline::{DuplicateInfo, ExtractedField};

/// The per-file result streamed to the frontend via the `file-prepared`
/// event. Serde shape must stay stable.
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
    /// Cross-batch duplicate signal. Set to None under the queue model;
    /// cross-session duplicate detection now lives client-side because
    /// "the batch" no longer has a defined boundary.
    pub batch_duplicate_group: Option<String>,
    /// True when the source path is a directory of images. Tells the
    /// review UI to surface a "Folder → .zip" hint, since the import
    /// flow will package the folder on commit.
    pub source_is_directory: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ProcessingProgressEvent {
    current: usize,
    total: usize,
    current_file: String,
    status: String,
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

#[tauri::command]
pub async fn cancel_processing(app: tauri::AppHandle) {
    let cancelled = app.state::<ProcessingCancelled>();
    cancelled.0.store(true, Ordering::Relaxed);
}

/// Push a batch of paths into the import worker queue and return immediately.
///
/// The worker drains jobs serially in the order received; the user can call
/// this again while previous work is in flight and the new paths append to
/// the queue. Each call resets the shared cancel flag so a prior cancel
/// doesn't poison new work.
///
/// `path_folder_roots` is a per-path map of source folder — keys are entries
/// in `paths`, values are the absolute folder paths the user picked. Empty
/// for non-folder picks. Drives per-comic parent-dir author hints; the
/// suppression rule is applied per call (a folder pick that lands one comic
/// IS-the-root suppresses the candidate, matching the previous behavior).
#[tauri::command]
pub async fn enqueue_import(
    app: tauri::AppHandle,
    paths: Vec<String>,
    path_folder_roots: Option<HashMap<String, String>>,
) -> Result<(), String> {
    // Reset cancellation so new work after a prior cancel proceeds normally.
    app.state::<ProcessingCancelled>()
        .0
        .store(false, Ordering::Relaxed);

    let path_folder_roots = path_folder_roots.unwrap_or_default();
    let parent_authors = derive_parent_authors(&paths, &path_folder_roots);

    let sender = app.state::<ImportQueueSender>();
    for raw in paths {
        let path = PathBuf::from(&raw);
        let candidates = parent_authors.get(&path).cloned().unwrap_or_default();
        let job = ImportJob {
            path,
            parent_authors: candidates,
        };
        sender.0.send(job).map_err(|e| {
            format!("Import queue is closed: {e}. The worker has stopped accepting jobs.")
        })?;
    }

    Ok(())
}

/// Per-path analysis driver invoked by the import worker. Builds the env,
/// dispatches by kind, and lets the pipeline's nodes emit
/// `processing-progress` and `file-prepared` events as it runs.
pub(crate) async fn process_import_path(
    app: &tauri::AppHandle,
    path: PathBuf,
    parent_authors: Vec<String>,
) -> Result<(), String> {
    use crate::pipeline::nodes::{FileKind, kind_for_path};

    // Bail early on unsupported extensions so the frontend's placeholder
    // doesn't sit in pending forever.
    let Some(kind) = kind_for_path(&path) else {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let _ = app.emit(
            "processing-progress",
            &ProcessingProgressEvent {
                current: 1,
                total: 1,
                current_file: file_name,
                status: "error".into(),
            },
        );
        return Ok(());
    };

    let env = build_pipeline_env(app).await?;

    let pipeline = match kind {
        FileKind::Novel => pipeline::nodes::novel_pipeline()
            .add_phase2(EmitPreparedNode)
            .build(),
        FileKind::Comic => pipeline::nodes::comic_pipeline()
            .add_phase2(EmitPreparedNode)
            .build(),
    };

    let mut candidates_map: HashMap<PathBuf, Vec<String>> = HashMap::new();
    candidates_map.insert(path.clone(), parent_authors);

    pipeline
        .run_batch(vec![path], Arc::clone(&env), candidates_map)
        .await;

    Ok(())
}

async fn build_pipeline_env(app: &tauri::AppHandle) -> Result<Arc<PipelineEnv>, String> {
    use super::{Author, Category, FileEntry, Tag};

    let cancelled: Arc<std::sync::atomic::AtomicBool> =
        Arc::clone(&app.state::<ProcessingCancelled>().0);

    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

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
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, created_at, updated_at FROM files",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to load existing files: {e}"))?;

    let category_map: HashMap<String, i64> = categories
        .iter()
        .map(|c| (c.name.to_lowercase(), c.id))
        .collect();
    // Author lookup keys are NFC-normalized + lowercased so suggestions
    // sourced from APFS file paths (often NFD) collide with the same name
    // sourced from the LLM (typically NFC). Without this, NFD/NFC variants
    // of the same name slip past resolution and become duplicate authors.
    let author_map: HashMap<String, i64> = authors
        .iter()
        .map(|a| (normalize_author_key(&a.name), a.id))
        .collect();
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

    Ok(Arc::new(PipelineEnv {
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
        settings: PipelineSettings { analyze_content },
    }))
}

/// Apply the existing per-root parent-author candidate logic to a single
/// enqueue's paths. Each call is its own mini-batch — folder picks that
/// arrive together get the suppression rule (skip when the only comic from
/// a root IS the root directory itself); paths from different enqueue
/// calls don't cross-contaminate.
fn derive_parent_authors(
    paths: &[String],
    path_folder_roots: &HashMap<String, String>,
) -> HashMap<PathBuf, Vec<String>> {
    use crate::pipeline::nodes::{FileKind, kind_for_path};

    // Group only comics by their picked root. Novels never get parent-dir
    // author hints (they have their own metadata pipeline).
    let mut comics_per_root: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for raw in paths {
        let path = PathBuf::from(raw);
        if !matches!(kind_for_path(&path), Some(FileKind::Comic)) {
            continue;
        }
        if let Some(root) = path_folder_roots.get(raw) {
            comics_per_root
                .entry(root.clone())
                .or_default()
                .push(path);
        }
    }

    let mut by_path: HashMap<PathBuf, Vec<String>> = HashMap::new();
    for (root, comics_in_root) in comics_per_root {
        let root_path = PathBuf::from(&root);
        let Some(name) = root_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
        else {
            continue;
        };

        // Skip when the only comic from this root IS the root directory
        // (folder-as-comic auto-zip). Canonicalize both sides so trailing
        // slashes / symlinks don't produce false negatives.
        let suppress = matches!(
            comics_in_root.as_slice(),
            [single] if single.is_dir()
                && root_path.canonicalize().ok() == single.canonicalize().ok()
        );
        if suppress {
            continue;
        }

        let cleaned = clean_folder_author_name(&name);
        if cleaned.is_empty() {
            continue;
        }
        let candidates = vec![cleaned];

        for path in comics_in_root {
            by_path.insert(path, candidates.clone());
        }
    }

    by_path
}

/// Streaming-emission variant that clones cover bytes so the FileContext
/// can be reused downstream. Called from EmitPreparedNode.
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
    let source_is_directory = ctx.file_path.is_dir();
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
        source_is_directory,
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

/// Canonical key used to compare author names across sources (LLM, folder
/// path, DB). NFC + trim + lowercase so APFS-NFD filename slices collide
/// with the LLM's NFC output.
pub(crate) fn normalize_author_key(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    name.nfc().collect::<String>().trim().to_lowercase()
}

/// Strip the surrounding bracket convention from a folder name and return
/// the author candidate. Folder authors overwhelmingly use `[author]` or
/// `[author] series-title` shapes, so a static rule is both faster and
/// more reliable than the LLM round-trip we used to do here. Falls back
/// to the trimmed raw name when no leading bracket is found, matching
/// the pre-LLM behavior.
fn clean_folder_author_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            let inner = rest[..end].trim();
            if !inner.is_empty() {
                return inner.to_string();
            }
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_folder_author_strips_outer_brackets() {
        assert_eq!(clean_folder_author_name("[SAVAN]"), "SAVAN");
        assert_eq!(clean_folder_author_name("[作者A]"), "作者A");
        assert_eq!(clean_folder_author_name("[ハ\u{309a}ニックアメリカ]"), "ハ\u{309a}ニックアメリカ");
    }

    #[test]
    fn clean_folder_author_extracts_first_bracket_group() {
        assert_eq!(clean_folder_author_name("[作者] 系列名"), "作者");
        assert_eq!(clean_folder_author_name("[XTER] title vol1"), "XTER");
    }

    #[test]
    fn clean_folder_author_falls_back_when_no_brackets() {
        assert_eq!(clean_folder_author_name("plain name"), "plain name");
        assert_eq!(clean_folder_author_name("  spaced  "), "spaced");
    }

    #[test]
    fn clean_folder_author_handles_empty_brackets() {
        assert_eq!(clean_folder_author_name("[]"), "[]");
        assert_eq!(clean_folder_author_name("[ ] series"), "[ ] series");
    }

    #[test]
    fn clean_folder_author_ignores_unbalanced_or_nested() {
        assert_eq!(clean_folder_author_name("[unterminated"), "[unterminated");
        assert_eq!(clean_folder_author_name("title [extra]"), "title [extra]");
    }
}
