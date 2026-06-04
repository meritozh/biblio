use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
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
///
/// Cover bytes used to ride along here as a base64 string; for a large
/// import batch that accumulated tens of MB of base64 in JS state per file
/// (review dialog never released them until close) and was crashing the
/// macOS WebContent process. The bytes now stay in `PreparedCoverCache` on
/// the Rust side; `cover_mime_type` here is the "has a staged cover" signal
/// and the frontend fetches the preview via `prepared_cover_get` on demand.
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

/// Per-import-batch staging for cover bytes produced by Phase 2 and
/// consumed at commit time by `file_create` / `file_replace`. Keyed by
/// the source path the frontend already carries in `item.path`.
///
/// Cleared explicitly via `prepared_cover_clear` (the import dialog calls
/// it when the user closes the dialog without committing). Individual
/// entries drop as commits consume them via `take`, so the cache stays
/// lean even mid-batch. Crucially, `cancel_processing` does NOT clear
/// the cache — the commit button calls cancel right before reading these
/// bytes, so clearing there would silently lose every auto-staged cover.
pub struct PreparedCoverCache(Arc<RwLock<HashMap<String, StagedCover>>>);

struct StagedCover {
    data: Vec<u8>,
    mime_type: String,
}

impl PreparedCoverCache {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    fn insert(&self, path: String, data: Vec<u8>, mime_type: String) {
        if let Ok(mut map) = self.0.write() {
            map.insert(path, StagedCover { data, mime_type });
        }
    }

    fn get(&self, path: &str) -> Option<(Vec<u8>, String)> {
        let map = self.0.read().ok()?;
        let cover = map.get(path)?;
        Some((cover.data.clone(), cover.mime_type.clone()))
    }

    pub fn take(&self, path: &str) -> Option<(Vec<u8>, String)> {
        let mut map = self.0.write().ok()?;
        let cover = map.remove(path)?;
        Some((cover.data, cover.mime_type))
    }

    fn clear(&self) {
        if let Ok(mut map) = self.0.write() {
            map.clear();
        }
    }
}

/// Cover preview bytes for a path the Phase-2 pipeline staged but the user
/// has not yet committed. Encoded base64 to keep parity with `cover_get`
/// — the form drops the result straight into a Blob URL and releases the
/// base64 string immediately, so the heap never holds more than one cover
/// at a time even when many rows are visible.
#[derive(Debug, Serialize)]
pub struct PreparedCover {
    pub data: String,
    pub mime_type: String,
}

#[tauri::command]
pub async fn prepared_cover_get(
    app: tauri::AppHandle,
    path: String,
) -> Result<PreparedCover, String> {
    use base64::Engine;
    let cache = app.state::<PreparedCoverCache>();
    let (bytes, mime_type) = cache
        .get(&path)
        .ok_or_else(|| "STAGED_COVER_NOT_FOUND".to_string())?;
    Ok(PreparedCover {
        data: base64::engine::general_purpose::STANDARD.encode(&bytes),
        mime_type,
    })
}

#[tauri::command]
pub async fn prepared_cover_clear(app: tauri::AppHandle) {
    let cache = app.state::<PreparedCoverCache>();
    cache.clear();
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
    // Cancel every batch claimed so far. A batch enqueued AFTER this call
    // claims a higher generation and is unaffected, so re-adding files after
    // a cancel no longer un-cancels the still-draining prior batch.
    cancelled.0.cancel_all();
    // Intentionally do NOT clear `PreparedCoverCache` here. One caller of
    // this command is the import dialog's commit button, which fires it to
    // halt still-queued analysis work right BEFORE running the commit loop
    // that reads staged cover bytes via `cache.take(...)`. Clearing the
    // cache here would silently drop covers for every file the user is
    // about to commit (the manually-uploaded ones survive only because
    // `file_create` short-circuits on inline `cover_data`). Callers that
    // genuinely want to discard staged bytes — like the dialog-close path
    // — invoke `prepared_cover_clear` explicitly.
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
    category_id: Option<i64>,
) -> Result<(), String> {
    // Claim a fresh cancellation generation for this batch. A prior cancel
    // only covers earlier generations, so this batch starts un-cancelled
    // without resetting (and thereby un-cancelling) any still-draining batch.
    let generation = app.state::<ProcessingCancelled>().0.begin();

    let path_folder_roots = path_folder_roots.unwrap_or_default();
    let parent_authors = derive_parent_authors(&paths, &path_folder_roots);

    let sender = app.state::<ImportQueueSender>();
    for raw in paths {
        let path = PathBuf::from(&raw);
        let candidates = parent_authors.get(&path).cloned().unwrap_or_default();
        let job = ImportJob {
            path,
            parent_authors: candidates,
            category_id,
            generation,
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
    category_id: Option<i64>,
    generation: u64,
) -> Result<(), String> {
    use crate::pipeline::nodes::{FileKind, kind_for_path};

    let emit_error = |reason: &str| {
        eprintln!("import: {} for {}", reason, path.to_string_lossy());
        let _ = app.emit(
            "processing-progress",
            &ProcessingProgressEvent {
                current: 1,
                total: 1,
                // Match on the full path the frontend placeholder was created
                // with (the listener keys by path) so the error actually lands
                // instead of leaving the item pending.
                current_file: path.to_string_lossy().to_string(),
                status: "error".into(),
            },
        );
    };

    // Category-first routing: when the import UI passed a target category, the
    // category's schema picks the pipeline and `kind_for_path` is demoted to a
    // validator (so "Novel category + dropped .zip" surfaces as an error
    // instead of silently running the comic pipeline). When no category was
    // supplied (legacy callers), fall back to extension-based routing.
    let env = build_pipeline_env(app, generation).await?;

    let kind = if let Some(cat_id) = category_id {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT schema_slug FROM categories WHERE id = ?")
                .bind(cat_id)
                .fetch_optional(&env.pool)
                .await
                .map_err(|e| format!("Failed to load category schema: {e}"))?;
        let Some((slug,)) = row else {
            emit_error("target category not found");
            return Ok(());
        };
        let schema = crate::schema::SchemaSlug::from_str(&slug);
        let want = FileKind::for_schema(schema);
        // Validate the input *fits* the chosen category's schema. A mismatch
        // (e.g. a .txt dropped into a comic category) is a user error, not a
        // silent reroute. `accepts_path` (not `kind_for_path` equality)
        // resolves the comic/galgame `.zip`+folder overlap — the category
        // already fixed the kind.
        if !want.accepts_path(&path) {
            emit_error("input does not match the selected category's schema");
            return Ok(());
        }
        want
    } else {
        // Legacy path: route purely by extension.
        let Some(detected) = kind_for_path(&path) else {
            emit_error("unsupported file type");
            return Ok(());
        };
        detected
    };

    let pipeline = match kind {
        FileKind::Novel => pipeline::nodes::novel_pipeline()
            .add_phase2(EmitPreparedNode)
            .build(),
        FileKind::Comic => pipeline::nodes::comic_pipeline()
            .add_phase2(EmitPreparedNode)
            .build(),
        FileKind::Galgame => pipeline::nodes::galgame_pipeline()
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

pub(crate) async fn build_pipeline_env(
    app: &tauri::AppHandle,
    generation: u64,
) -> Result<Arc<PipelineEnv>, String> {
    use super::{Author, Category, FileEntry, Tag};

    let cancel = Arc::clone(&app.state::<ProcessingCancelled>().0);

    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let categories: Vec<Category> = sqlx::query_as(
        "SELECT id, name, description, icon, is_default, folder_name, schema_slug, view_config, created_at FROM categories",
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
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, created_at, updated_at FROM files",
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

    // Load the two path roots so duplicate detection (and future pipeline
    // nodes that stat existing rows) can resolve stored relative paths.
    let roots = super::settings::load_path_roots(&pool).await?;

    Ok(Arc::new(PipelineEnv {
        pool,
        llm_config,
        app: app.clone(),
        cancel,
        cancel_generation: generation,
        category_map,
        author_map,
        tag_map,
        category_names,
        tag_names,
        existing_files,
        storage_path: roots.storage_path,
        app_root: roots.app_root,
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

/// Build the event payload from the per-file context. The cover bytes are
/// removed from `ctx` and stashed in `PreparedCoverCache`; the event itself
/// only carries the mime type as the "has a staged cover" signal.
fn drain_into_prepared(
    ctx: &mut FileContext,
    cache: &PreparedCoverCache,
) -> FilePreparedImport {
    let path = ctx.file_path.to_string_lossy().to_string();
    let cover_mime_type = match ctx.cover.take() {
        Some(c) => {
            let mime = c.mime_type.clone();
            cache.insert(path.clone(), c.data, c.mime_type);
            Some(mime)
        }
        None => None,
    };
    let display_name = ctx
        .display_name
        .clone()
        .unwrap_or_else(|| ctx.file_name.clone());
    let source_is_directory = ctx.file_path.is_dir();
    FilePreparedImport {
        path,
        file_name: ctx.file_name.clone(),
        display_name,
        category_id: ctx.category_id,
        tag_ids: ctx.tag_ids.clone(),
        author_ids: ctx.author_ids.clone(),
        metadata: ctx.extracted_metadata.clone(),
        unresolved_author_names: ctx.unresolved_authors.clone(),
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
        let cache = env.app.state::<PreparedCoverCache>();
        let prepared = drain_into_prepared(ctx, &cache);
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
