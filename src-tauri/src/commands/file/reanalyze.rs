use super::*;
#[derive(Serialize)]
pub struct ReanalyzeError {
    pub file_id: i64,
    pub display_name: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct ReanalyzeResponse {
    pub processed: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub errors: Vec<ReanalyzeError>,
}

#[derive(serde::Serialize, Clone)]
struct EmitTagsBulkChange {
    id: i64,
}

/// Count novels with zero tags — feeds the affected-count badge on
/// `/cleanup`'s Debug action card so the user sees the scale before
/// committing to the LLM run.
#[tauri::command]
pub async fn file_count_novels_missing_tags(app: AppHandle) -> Result<i64, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM files f
         JOIN categories c ON c.id = f.category_id
         WHERE c.schema_slug = 'novel'
           AND f.file_status = 'available'
           AND NOT EXISTS (SELECT 1 FROM file_tags WHERE file_id = f.id)",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(count)
}

/// Find novel-schema files that have zero `file_tags` rows, run them
/// through the import-time LLM content-extraction pipeline, and apply the
/// returned tags + category. Used by `/cleanup`'s Debug actions.
///
/// Re-uses `extract_content_metadata` / `sample_text_content` so the
/// behavior matches what the import flow does for fresh files. Tags the
/// LLM proposes that don't exist yet are created on the spot (same
/// validation path as `tag_create`); category names that don't match any
/// existing category are ignored (the file's current category stays).
///
/// Per-file failures are collected into the `errors` list and don't stop
/// the run. The whole thing is one blocking IPC; live progress is a v2.
#[tauri::command]
pub async fn file_reanalyze_missing_tags(app: AppHandle) -> Result<ReanalyzeResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Fail fast if LLM isn't configured — re-using the same loader the
    // import flow uses, so the error string is the same one the user
    // already sees elsewhere.
    let config = crate::commands::llm::llm_config_get(app.clone()).await?;

    #[derive(sqlx::FromRow)]
    struct Candidate {
        id: i64,
        display_name: String,
        path: String,
        local_cache_path: Option<String>,
        storage_kind: Option<String>,
        category_id: Option<i64>,
    }

    let candidates: Vec<Candidate> = sqlx::query_as(
        "SELECT f.id, f.display_name, f.path, f.local_cache_path,
                f.storage_kind, f.category_id
         FROM files f
         JOIN categories c ON c.id = f.category_id
         WHERE c.schema_slug = 'novel'
           AND f.file_status = 'available'
           AND NOT EXISTS (SELECT 1 FROM file_tags WHERE file_id = f.id)
         ORDER BY f.id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if candidates.is_empty() {
        return Ok(ReanalyzeResponse {
            processed: 0,
            succeeded: 0,
            failed: 0,
            errors: Vec::new(),
        });
    }

    // Pre-load categories + tags once. The LLM gets the name lists in its
    // prompt; we use the ids to resolve its string output back to rows.
    let categories: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM categories")
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
    let category_names: Vec<String> = categories.iter().map(|(_, n)| n.clone()).collect();
    let mut existing_tags: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM tags")
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
    let tag_names: Vec<String> = existing_tags.iter().map(|(_, n)| n.clone()).collect();

    // Load roots once for path resolution. Candidates have storage_kind
    // varying per row, so the helper chooses the right root per row.
    let roots = super::settings::load_path_roots(&pool).await?;

    let mut succeeded = 0i64;
    let mut failed = 0i64;
    let mut errors: Vec<ReanalyzeError> = Vec::new();

    for c in &candidates {
        // Read path: local rows use `path` directly (resolved against
        // storage_path); remote rows need a cached copy. Without one we
        // skip the file rather than silently triggering a download
        // (would amplify LLM cost without consent).
        let kind = c.storage_kind.as_deref().unwrap_or("local");
        let read_path = match kind {
            "local" => crate::path_resolve::to_absolute(
                kind,
                &c.path,
                &roots.storage_path,
                &roots.app_root,
            )
            .to_string_lossy()
            .to_string(),
            _ => match c.local_cache_path.as_deref().filter(|s| !s.is_empty()) {
                Some(cache) => crate::path_resolve::cache_to_absolute(cache, &roots.storage_path)
                    .to_string_lossy()
                    .to_string(),
                None => {
                    failed += 1;
                    errors.push(ReanalyzeError {
                        file_id: c.id,
                        display_name: c.display_name.clone(),
                        message: "skipped: remote file not cached locally".to_string(),
                    });
                    continue;
                }
            },
        };

        // Inline the read → decode → sample steps with explicit error
        // attribution per step. Lumping all three into one "decode failed"
        // (which sample_text_content does) hides whether the file is
        // missing on disk, empty, or in an encoding the detector can't
        // pin down — and the right user response differs for each.
        let sample = match std::fs::read(&read_path) {
            Err(e) => {
                failed += 1;
                errors.push(ReanalyzeError {
                    file_id: c.id,
                    display_name: c.display_name.clone(),
                    message: format!("file unreadable at {}: {}", read_path, e),
                });
                continue;
            }
            Ok(bytes) if bytes.is_empty() => {
                failed += 1;
                errors.push(ReanalyzeError {
                    file_id: c.id,
                    display_name: c.display_name.clone(),
                    message: "file is empty".to_string(),
                });
                continue;
            }
            Ok(bytes) => {
                let Some(text) = crate::pipeline::nodes::decode_to_utf8(&bytes) else {
                    failed += 1;
                    errors.push(ReanalyzeError {
                        file_id: c.id,
                        display_name: c.display_name.clone(),
                        message: "encoding detection failed — try opening \
                                  the file in a text editor and re-saving \
                                  as UTF-8"
                            .to_string(),
                    });
                    continue;
                };
                let Some(sample) = crate::pipeline::nodes::sample_from_text(&text, 5, 1000) else {
                    failed += 1;
                    errors.push(ReanalyzeError {
                        file_id: c.id,
                        display_name: c.display_name.clone(),
                        message: "no content after decoding (zero chars)".to_string(),
                    });
                    continue;
                };
                sample
            }
        };

        let meta = match crate::commands::llm::extract_content_metadata(
            &config,
            &pool,
            &sample,
            Some(&c.display_name),
            &category_names,
            &tag_names,
        )
        .await
        {
            Ok(m) => m,
            Err(e) => {
                failed += 1;
                errors.push(ReanalyzeError {
                    file_id: c.id,
                    display_name: c.display_name.clone(),
                    message: format!("LLM error: {e}"),
                });
                continue;
            }
        };

        // Apply category — only if the LLM picked one that maps to an
        // existing row and it's different from the file's current. Move
        // via the existing primitive so the disk move happens too.
        if let Some(new_cat_name) = meta.category.as_deref() {
            if let Some((new_cat_id, _)) = categories.iter().find(|(_, n)| n == new_cat_name) {
                if Some(*new_cat_id) != c.category_id {
                    if let Err(e) = file_move_category(app.clone(), c.id, Some(*new_cat_id)).await {
                        errors.push(ReanalyzeError {
                            file_id: c.id,
                            display_name: c.display_name.clone(),
                            message: format!("category move failed: {e}"),
                        });
                    }
                }
            }
        }

        // Apply tags — resolve names to ids, creating any unknown ones
        // via INSERT OR IGNORE so a race with a concurrent insert (e.g.
        // a parallel import) doesn't double-create.
        let mut tag_ids: Vec<i64> = Vec::new();
        for tag_name in &meta.tags {
            let trimmed = tag_name.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some((tid, _)) = existing_tags.iter().find(|(_, n)| n == trimmed) {
                tag_ids.push(*tid);
                continue;
            }
            // Tag is new to this run. Create-or-lookup.
            let insert = sqlx::query("INSERT OR IGNORE INTO tags (name) VALUES (?)")
                .bind(trimmed)
                .execute(&pool)
                .await;
            let new_id = match insert {
                Ok(r) if r.rows_affected() > 0 => Some(r.last_insert_rowid()),
                _ => {
                    // Already existed (lost the race or wasn't in our
                    // cached list); look up by name.
                    sqlx::query_as::<_, (i64,)>("SELECT id FROM tags WHERE name = ?")
                        .bind(trimmed)
                        .fetch_optional(&pool)
                        .await
                        .ok()
                        .flatten()
                        .map(|(id,)| id)
                }
            };
            if let Some(id) = new_id {
                tag_ids.push(id);
                // Cache locally so a later file in the same run sees it.
                existing_tags.push((id, trimmed.to_string()));
            }
        }

        for tid in &tag_ids {
            let _ = sqlx::query("INSERT OR IGNORE INTO file_tags (file_id, tag_id) VALUES (?, ?)")
                .bind(c.id)
                .bind(tid)
                .execute(&pool)
                .await;
        }

        if !tag_ids.is_empty() {
            succeeded += 1;
        } else {
            // LLM returned no usable tags. Count as failure with a hint
            // so the user knows why this file is still untagged.
            failed += 1;
            errors.push(ReanalyzeError {
                file_id: c.id,
                display_name: c.display_name.clone(),
                message: "LLM returned no tags".to_string(),
            });
        }
    }

    // One bulk event at the end matches the cleanup commit's pattern —
    // the existing listenTagAuthorChanges listener refetches the picker
    // lists and refreshes the active file view.
    let _ = app.emit("tag-deleted", EmitTagsBulkChange { id: 0 });

    Ok(ReanalyzeResponse {
        processed: candidates.len() as i64,
        succeeded,
        failed,
        errors,
    })
}

// ── Assign author to authorless files in a category ──────────────────────────

#[derive(Serialize)]
pub struct AssignAuthorResponse {
    pub assigned: i64,
}

#[derive(serde::Serialize, Clone)]
struct EmitAuthorsBulkChange {
    id: i64,
}

/// Count files with no `file_authors` row, optionally scoped to one
/// category. Feeds the affected-count badge on `/cleanup`'s "Assign
/// author" Debug card. `category_id = None` counts across all
/// categories.
#[tauri::command]
pub async fn file_count_authorless_in_category(
    app: AppHandle,
    category_id: Option<i64>,
) -> Result<i64, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM files f
         WHERE (?1 IS NULL OR f.category_id = ?1)
           AND f.file_status = 'available'
           AND NOT EXISTS (SELECT 1 FROM file_authors WHERE file_id = f.id)",
    )
    .bind(category_id)
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(count)
}

/// Insert a `file_authors` link from `author_id` to every file in the
/// given category (or library-wide when `category_id` is None) that
/// currently has zero authors. Single transaction via `INSERT ... SELECT`;
/// `INSERT OR IGNORE` defends against the unlikely race where a parallel
/// import inserted a row between our COUNT and our INSERT.
///
/// Emits one `author-updated` event (sentinel id `0`) so the existing
/// `listenTagAuthorChanges` listener picks the change up across the app.
#[tauri::command]
pub async fn file_assign_author_to_authorless(
    app: AppHandle,
    category_id: Option<i64>,
    author_id: i64,
) -> Result<AssignAuthorResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let result = sqlx::query(
        "INSERT OR IGNORE INTO file_authors (file_id, author_id)
         SELECT f.id, ?2 FROM files f
         WHERE (?1 IS NULL OR f.category_id = ?1)
           AND f.file_status = 'available'
           AND NOT EXISTS (SELECT 1 FROM file_authors WHERE file_id = f.id)",
    )
    .bind(category_id)
    .bind(author_id)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let assigned = result.rows_affected() as i64;
    if assigned > 0 {
        let _ = app.emit("author-updated", EmitAuthorsBulkChange { id: 0 });
    }

    Ok(AssignAuthorResponse { assigned })
}

#[derive(Serialize)]
pub struct RegenerateCoversResponse {
    pub processed: i64,
    pub regenerated: i64,
    /// File on disk missing, or remote without a local cache. The user
    /// fixes by restoring the source or downloading the remote copy and
    /// re-running.
    pub skipped: i64,
    /// Archive unreadable, image decode failure — won't be fixed by a
    /// re-run.
    pub failed: i64,
    pub errors: Vec<ReanalyzeError>,
}

/// Count comic-schema files with no row in `covers`. Feeds the affected-
/// count badge on `/cleanup`'s "Regenerate missing comic covers" Debug
/// card so the user sees the scale before triggering the run.
#[tauri::command]
pub async fn file_count_comics_missing_covers(app: AppHandle) -> Result<i64, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM files f
         JOIN categories c ON c.id = f.category_id
         WHERE c.schema_slug = 'comic'
           AND f.file_status = 'available'
           AND NOT EXISTS (SELECT 1 FROM covers WHERE file_id = f.id)",
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(count)
}

/// Re-extract the cover image for every comic archive whose `covers` row
/// is missing and INSERT OR REPLACE the result. Runs the cover-focused
/// subset of the normal comic import pipeline against each in-storage
/// archive, so the picking quality matches a fresh import:
///
/// - `ArchiveFirstImageCoverNode` (Phase 1) — baseline pick from the
///   archive entries, used as fallback if LLM is off or fails.
/// - `ArchiveListImagesNode` (Phase 1) — records every image entry's
///   basename + archive index so the LLM has a candidate list.
/// - `LlmCoverCandidatesNode` (Phase 2) — basenames → LLM-ranked picks.
/// - `LlmVisionCoverCheckNode` (Phase 2) — vision model verifies the
///   ranked candidates' bytes and picks the best.
/// - `CoverCompressNode` (Phase 2) — re-encodes to ≤ ~200 KB JPEG.
///
/// Each node self-gates via `applies()`: when LLM is disabled or no
/// candidates survive ranking, the baseline pick stands. So this works
/// gracefully whether the user has LLM configured or not — the floor
/// is always the basename heuristic.
///
/// Pre-filters candidates by existence on disk so an unreadable / moved
/// archive is reported as `skipped` rather than churning through the
/// pipeline. Pipeline-side failures (archive open errors, decode errors)
/// surface as `failed` rows with the node's error message.
#[tauri::command]
pub async fn file_regenerate_missing_covers(
    app: AppHandle,
) -> Result<RegenerateCoversResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    #[derive(sqlx::FromRow)]
    struct Candidate {
        id: i64,
        display_name: String,
        path: String,
        local_cache_path: Option<String>,
        storage_kind: Option<String>,
    }

    let candidates: Vec<Candidate> = sqlx::query_as(
        "SELECT f.id, f.display_name, f.path, f.local_cache_path, f.storage_kind
         FROM files f
         JOIN categories c ON c.id = f.category_id
         WHERE c.schema_slug = 'comic'
           AND f.file_status = 'available'
           AND NOT EXISTS (SELECT 1 FROM covers WHERE file_id = f.id)
         ORDER BY f.id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if candidates.is_empty() {
        return Ok(RegenerateCoversResponse {
            processed: 0,
            regenerated: 0,
            skipped: 0,
            failed: 0,
            errors: Vec::new(),
        });
    }

    let roots = super::settings::load_path_roots(&pool).await?;

    let mut skipped = 0i64;
    let mut errors: Vec<ReanalyzeError> = Vec::new();

    // Resolve each candidate to an on-disk archive path. Rows whose
    // source is missing (remote without cache, file moved/deleted) get
    // reported as `skipped` and never reach the pipeline.
    let mut runnable: Vec<(PathBuf, i64, String)> = Vec::new();
    for c in &candidates {
        let kind = c.storage_kind.as_deref().unwrap_or("local");
        let abs = match kind {
            "local" => crate::path_resolve::to_absolute(
                kind,
                &c.path,
                &roots.storage_path,
                &roots.app_root,
            ),
            _ => match c.local_cache_path.as_deref().filter(|s| !s.is_empty()) {
                Some(cache) => crate::path_resolve::cache_to_absolute(cache, &roots.storage_path),
                None => {
                    skipped += 1;
                    errors.push(ReanalyzeError {
                        file_id: c.id,
                        display_name: c.display_name.clone(),
                        message: "skipped: remote file not cached locally".to_string(),
                    });
                    continue;
                }
            },
        };

        if !abs.exists() {
            skipped += 1;
            errors.push(ReanalyzeError {
                file_id: c.id,
                display_name: c.display_name.clone(),
                message: "skipped: source file not on disk".to_string(),
            });
            continue;
        }

        runnable.push((abs, c.id, c.display_name.clone()));
    }

    if runnable.is_empty() {
        return Ok(RegenerateCoversResponse {
            processed: candidates.len() as i64,
            regenerated: 0,
            skipped,
            failed: 0,
            errors,
        });
    }

    // Claim a fresh cancellation generation for this recovery run. A prior
    // `cancel_processing` only covers earlier generations, so this batch
    // starts un-cancelled without resetting (and un-cancelling) any other
    // still-draining batch. `enqueue_import` claims its generation the same
    // way for the normal import path.
    let generation = app.state::<crate::ProcessingCancelled>().0.begin();

    // Build the cover-only subset of the comic pipeline. Re-using the
    // node implementations directly guarantees byte-for-byte parity with
    // what a fresh import would have produced.
    let pipeline = crate::pipeline::runner::Pipeline::builder()
        .add_phase1(crate::pipeline::nodes::ArchiveFirstImageCoverNode)
        .add_phase1(crate::pipeline::nodes::ArchiveListImagesNode)
        .add_phase2(crate::pipeline::nodes::LlmCoverCandidatesNode)
        .add_phase2(crate::pipeline::nodes::LlmVisionCoverCheckNode)
        .add_phase2(crate::pipeline::nodes::CoverCompressNode)
        .build();

    let env = crate::commands::processing::build_pipeline_env(&app, generation).await?;
    let paths: Vec<PathBuf> = runnable.iter().map(|(p, _, _)| p.clone()).collect();

    // run_batch caps Phase-1 fan-out internally (PHASE1_CONCURRENCY=8)
    // and drains Phase-2 sequentially, so concurrent LLM calls stay
    // bounded by the pipeline's existing limits.
    let results = pipeline
        .run_batch(paths, env, std::collections::HashMap::new())
        .await;

    // Map results back to (file_id, display_name) by source path. The
    // pipeline preserves input order but we key on path to be defensive
    // against future scheduling changes.
    use std::collections::HashMap;
    let id_by_path: HashMap<PathBuf, (i64, String)> = runnable
        .into_iter()
        .map(|(p, id, name)| (p, (id, name)))
        .collect();

    let mut regenerated = 0i64;
    let mut failed = 0i64;

    for ctx in results {
        let Some((file_id, display_name)) = id_by_path.get(&ctx.file_path).cloned() else {
            continue;
        };

        // Pull the error message off the most-relevant failed node, if any.
        // Pipeline-side errors are recorded as NodeStatus::Err and don't
        // halt later nodes — but if all cover-producing nodes failed we
        // won't have ctx.cover to store.
        let node_error: Option<String> = ctx.outcomes.iter().find_map(|o| match &o.status {
            crate::pipeline::NodeStatus::Err(msg) => Some(format!("{}: {}", o.name, msg)),
            _ => None,
        });

        let Some(cover) = ctx.cover else {
            failed += 1;
            errors.push(ReanalyzeError {
                file_id,
                display_name,
                message: node_error
                    .unwrap_or_else(|| "no cover extracted from archive".to_string()),
            });
            continue;
        };

        if let Err(e) =
            sqlx::query("INSERT OR REPLACE INTO covers (file_id, data, mime_type) VALUES (?, ?, ?)")
                .bind(file_id)
                .bind(&cover.data)
                .bind(&cover.mime_type)
                .execute(&pool)
                .await
        {
            failed += 1;
            errors.push(ReanalyzeError {
                file_id,
                display_name,
                message: format!("DB write failed: {e}"),
            });
            continue;
        }

        regenerated += 1;
    }

    Ok(RegenerateCoversResponse {
        processed: candidates.len() as i64,
        regenerated,
        skipped,
        failed,
        errors,
    })
}
