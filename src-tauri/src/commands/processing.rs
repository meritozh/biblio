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
    /// True when the source path is a directory of images. Tells the
    /// review UI to surface a "Folder → .zip" hint, since the import
    /// flow will package the folder on commit.
    pub source_is_directory: bool,
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

/// Prepare a batch of paths for import: run both pipelines, stream
/// per-file `file-prepared` events, and return the final list.
///
/// `path_folder_roots` is a per-path map of source folder — keys are
/// entries in `paths`, values are the absolute folder paths the user
/// picked. Empty/None for non-folder picks. Drives per-comic parent-dir
/// author hints; each unique folder root gets one LLM cleanup call
/// regardless of how many comics were scanned out of it.
#[tauri::command]
pub async fn file_prepare_import(
    app: tauri::AppHandle,
    paths: Vec<String>,
    path_folder_roots: Option<HashMap<String, String>>,
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
        settings: PipelineSettings { analyze_content },
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

    // Frontend filters by extension before invoking, but skip anything
    // that slipped past (drag-drop, IPC abuse, future code paths). The
    // missing entries surface in the result map's final-sync as errored
    // items, matching the existing "Analysis failed" UX.
    let mut novel_paths: Vec<PathBuf> = Vec::new();
    let mut comic_paths: Vec<PathBuf> = Vec::new();
    for raw in paths {
        let pb = PathBuf::from(raw);
        match pipeline::nodes::kind_for_path(&pb) {
            Some(pipeline::nodes::FileKind::Comic) => comic_paths.push(pb),
            Some(pipeline::nodes::FileKind::Novel) => novel_paths.push(pb),
            None => {} // dropped; frontend final-sync flags it as failed
        }
    }

    // Per-path parent-dir author candidates. For every unique folder root
    // in `path_folder_roots`, derive an author candidate from the picked
    // folder name and fan it out across every comic scanned from that
    // root. Non-folder picks and novels get an empty candidate list.
    //
    // Cleanup is a static rule (`[author]` → `author`, see
    // `clean_folder_author_name`), not an LLM call — folder authors
    // overwhelmingly follow the bracket convention, the LLM round-trip
    // was both slow and brittle (one malformed JSON response sank the
    // whole batch).
    //
    // Trivial-pick suppression is per root: when a root contributes
    // exactly one comic AND that comic IS the root directory itself
    // (image-folder pick that auto-zips on commit), the basename would
    // duplicate the comic's own name as its author — skip it.
    let path_folder_roots = path_folder_roots.unwrap_or_default();
    let mut comics_per_root: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for path in &comic_paths {
        let key = path.to_string_lossy().to_string();
        if let Some(root) = path_folder_roots.get(&key) {
            comics_per_root.entry(root.clone()).or_default().push(path.clone());
        }
    }

    let mut parent_author_candidates_by_path: HashMap<PathBuf, Vec<String>> = HashMap::new();
    for (root, comics_in_root) in comics_per_root {
        let root_path = std::path::PathBuf::from(&root);
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
            parent_author_candidates_by_path.insert(path, candidates.clone());
        }
    }

    // Run sequentially: both pipelines emit `processed_ordinal` starting
    // from 1, so concurrent execution would surface two interleaved
    // counters in the UI. Empty path lists short-circuit `run_batch`, so
    // the common single-type import pays nothing for the unused branch.
    let novel_ctxs = novel_pipeline
        .run_batch(novel_paths, Arc::clone(&env), HashMap::new())
        .await;
    let comic_ctxs = comic_pipeline
        .run_batch(comic_paths, Arc::clone(&env), parent_author_candidates_by_path)
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
    let source_is_directory = ctx.file_path.is_dir();
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
        source_is_directory,
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

/// Tag each file in a batch that shares a display_name AND category with
/// another file so the frontend can group and warn about same-batch
/// duplicates. Same name in different categories (e.g. a novel and a
/// comic that share a title) is not a duplicate.
fn detect_batch_duplicates(results: &mut [FilePreparedImport]) {
    let mut name_groups: HashMap<(String, Option<i64>), Vec<usize>> = HashMap::new();
    for (idx, result) in results.iter().enumerate() {
        let normalized = result.display_name.trim().to_lowercase();
        name_groups
            .entry((normalized, result.category_id))
            .or_default()
            .push(idx);
    }

    for ((name, _category_id), indices) in &name_groups {
        if indices.len() > 1 {
            let group_id = format!("batch_{}", name);
            for &idx in indices {
                results[idx].batch_duplicate_group = Some(group_id.clone());
            }
        }
    }
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

    fn prepared_with(name: &str, category_id: Option<i64>) -> FilePreparedImport {
        FilePreparedImport {
            path: format!("/tmp/{name}"),
            file_name: name.to_string(),
            display_name: name.to_string(),
            category_id,
            tag_ids: vec![],
            author_ids: vec![],
            metadata: vec![],
            unresolved_author_names: vec![],
            cover_data: None,
            cover_mime_type: None,
            progress: None,
            suggested_tags: vec![],
            duplicate_of: None,
            batch_duplicate_group: None,
            source_is_directory: false,
        }
    }

    #[test]
    fn batch_dup_groups_same_name_same_category() {
        let mut results = vec![
            prepared_with("三体", Some(1)),
            prepared_with("三体", Some(1)),
        ];
        detect_batch_duplicates(&mut results);
        assert!(results[0].batch_duplicate_group.is_some());
        assert_eq!(results[0].batch_duplicate_group, results[1].batch_duplicate_group);
    }

    #[test]
    fn batch_dup_skips_same_name_different_category() {
        let mut results = vec![
            prepared_with("三体", Some(1)),
            prepared_with("三体", Some(2)),
        ];
        detect_batch_duplicates(&mut results);
        assert!(results[0].batch_duplicate_group.is_none());
        assert!(results[1].batch_duplicate_group.is_none());
    }

    #[test]
    fn batch_dup_groups_when_both_categories_unset() {
        let mut results = vec![
            prepared_with("三体", None),
            prepared_with("三体", None),
        ];
        detect_batch_duplicates(&mut results);
        assert!(results[0].batch_duplicate_group.is_some());
        assert_eq!(results[0].batch_duplicate_group, results[1].batch_duplicate_group);
    }
}
