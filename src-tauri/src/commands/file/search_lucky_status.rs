use super::*;
/// Minimum query length, in Unicode characters, for an FTS5 trigram lookup.
/// Trigram only indexes 3-character windows, so anything shorter has no
/// indexed term to match.
const TRIGRAM_MIN_CHARS: usize = 3;

/// What kind of SQL filter to apply for a typed search query. Returned by
/// [`prepare_search_filter`] so `file_search` can pick the matching SQL.
pub(super) enum SearchFilter {
    /// Use FTS5 `MATCH` with the given expression. Fast, ranked.
    Fts(String),
    /// Use `display_name LIKE %p% OR path LIKE %p%` with the given pattern
    /// (already wrapped with `%` and SQL wildcards escaped). Used for
    /// queries shorter than the trigram window.
    Like(String),
}

/// Translate a user-typed query into either an FTS5 MATCH expression or a
/// LIKE fallback for sub-trigram-length queries.
///
/// Strategy:
///   1. Replace every FTS5 operator character (`"'`:()-+*^`) with a space
///      so we never accidentally parse a user's punctuation as syntax.
///   2. Split on whitespace; if the longest remaining token is < 3 chars,
///      fall back to `LIKE '%q%'` against the trimmed raw query — trigram
///      indexes 3-character windows and returns nothing for shorter input.
///   3. Otherwise quote each token and AND them together. The trigram
///      tokenizer matches substrings inside indexed text, so we don't need
///      a `*` prefix marker.
///
/// Returns None if the query has no usable content (all whitespace, all
/// punctuation, etc.) — callers should treat that as "no filter" rather
/// than issuing an empty MATCH (which FTS5 rejects).
pub(super) fn prepare_search_filter(raw: &str) -> Option<SearchFilter> {
    let sanitized: String = raw
        .chars()
        .map(|c| match c {
            '"' | '\'' | ':' | '(' | ')' | '+' | '-' | '*' | '^' => ' ',
            c => c,
        })
        .collect();

    let terms: Vec<&str> = sanitized
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .collect();

    if terms.is_empty() {
        return None;
    }

    // FTS5's trigram tokenizer only indexes 3-char windows, so a sub-trigram
    // token can never MATCH and — ANDed with the rest — would zero out the
    // whole query (e.g. "JP 火影忍者" returned nothing). Keep only tokens long
    // enough for the index; drop the shorter ones from the FTS expression.
    let fts_terms: Vec<&&str> = terms
        .iter()
        .filter(|t| t.chars().count() >= TRIGRAM_MIN_CHARS)
        .collect();

    if fts_terms.is_empty() {
        // No token reaches the trigram window — fall back to a LIKE scan over
        // the raw trimmed query. Escape `%`, `_`, and `\` so the user can't
        // smuggle wildcards into the pattern.
        let pattern = raw
            .trim()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        if pattern.is_empty() {
            return None;
        }
        return Some(SearchFilter::Like(format!("%{}%", pattern)));
    }

    // Quote each term so spaces, slashes, and other token-internal
    // punctuation are searched literally rather than parsed as FTS5
    // syntax. Trigram needs the entire term as a contiguous substring.
    let quoted: Vec<String> = fts_terms.iter().map(|t| format!("\"{}\"", t)).collect();
    Some(SearchFilter::Fts(quoted.join(" ")))
}

#[tauri::command]
pub async fn file_search(
    app: AppHandle,
    query: String,
    category_id: Option<i64>,
    _tag_ids: Option<Vec<i64>>,
    _metadata_filters: Option<Vec<MetadataFilter>>,
    sort_by: Option<String>,
    sort_desc: Option<bool>,
    conditions: Option<Vec<FilterCondition>>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<FileListResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    // The aliased form is needed because the search query joins `files_fts`
    // and references columns through `f.`.
    let order_by = order_by_clause(sort_by.as_deref(), sort_desc.unwrap_or(true), "f");
    let (filter_sql, filter_binds) = build_filter_sql(conditions.as_deref().unwrap_or(&[]), "f");

    // If the user's query sanitizes to nothing, return an empty result set
    // with total = 0. The frontend routes empty queries to `file_list`, so
    // this path really only covers the all-punctuation / all-whitespace
    // edge case.
    let Some(filter) = prepare_search_filter(&query) else {
        return Ok(FileListResponse {
            files: Vec::new(),
            total: 0,
        });
    };

    // Same pattern as file_list: build the WHERE once and share it between
    // the row query and the count query so `total` matches what's loadable.
    let mut where_tail = String::new();
    if let Some(cat_id) = category_id {
        where_tail.push_str(&format!(" AND f.category_id = {}", cat_id));
    }
    where_tail.push_str(&filter_sql);

    let (row_query, count_query, bind_value) = match &filter {
        SearchFilter::Fts(expr) => (
            format!(
                "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.local_cache_path, f.is_favorite, f.created_at, f.updated_at \
                 FROM files f \
                 JOIN files_fts ON files_fts.rowid = f.id \
                 WHERE files_fts MATCH ?{} \
                 {} LIMIT {} OFFSET {}",
                where_tail, order_by, limit, offset
            ),
            format!(
                "SELECT COUNT(*) FROM files f \
                 JOIN files_fts ON files_fts.rowid = f.id \
                 WHERE files_fts MATCH ?{}",
                where_tail
            ),
            expr.clone(),
        ),
        SearchFilter::Like(pattern) => (
            format!(
                "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.local_cache_path, f.is_favorite, f.created_at, f.updated_at \
                 FROM files f \
                 WHERE (f.display_name LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'){} \
                 {} LIMIT {} OFFSET {}",
                where_tail, order_by, limit, offset
            ),
            format!(
                "SELECT COUNT(*) FROM files f \
                 WHERE (f.display_name LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'){}",
                where_tail
            ),
            pattern.clone(),
        ),
    };

    let mut row_stmt = sqlx::query_as::<_, FileEntry>(&row_query).bind(&bind_value);
    if matches!(filter, SearchFilter::Like(_)) {
        row_stmt = row_stmt.bind(&bind_value);
    }
    for b in &filter_binds {
        row_stmt = row_stmt.bind(b);
    }
    let files: Vec<FileEntry> = row_stmt.fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let mut count_stmt = sqlx::query_as::<_, (i64,)>(&count_query).bind(&bind_value);
    if matches!(filter, SearchFilter::Like(_)) {
        count_stmt = count_stmt.bind(&bind_value);
    }
    for b in &filter_binds {
        count_stmt = count_stmt.bind(b);
    }
    let total: (i64,) = count_stmt
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let items = hydrate_file_items(&pool, files).await?;

    Ok(FileListResponse {
        files: items,
        total: total.0,
    })
}

#[tauri::command]
pub async fn file_lucky(
    app: AppHandle,
    category_id: Option<i64>,
    query: Option<String>,
    conditions: Option<Vec<FilterCondition>>,
    limit: Option<i32>,
) -> Result<Vec<FileListItem>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let limit = limit.unwrap_or(3).clamp(1, 12);
    let (filter_sql, filter_binds) = build_filter_sql(conditions.as_deref().unwrap_or(&[]), "f");

    let mut where_tail = String::new();
    if let Some(cat_id) = category_id {
        where_tail.push_str(&format!(" AND f.category_id = {}", cat_id));
    }
    where_tail.push_str(&filter_sql);

    let trimmed_query = query.as_deref().map(str::trim).filter(|q| !q.is_empty());

    let files: Vec<FileEntry> = if let Some(q) = trimmed_query {
        let Some(search_filter) = prepare_search_filter(q) else {
            return Ok(Vec::new());
        };
        let (row_query, bind_value) = match &search_filter {
            SearchFilter::Fts(expr) => (
                format!(
                    "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.local_cache_path, f.is_favorite, f.created_at, f.updated_at \
                     FROM files f \
                     JOIN files_fts ON files_fts.rowid = f.id \
                     WHERE files_fts MATCH ?{} \
                     ORDER BY RANDOM() LIMIT {}",
                    where_tail, limit
                ),
                expr.clone(),
            ),
            SearchFilter::Like(pattern) => (
                format!(
                    "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.local_cache_path, f.is_favorite, f.created_at, f.updated_at \
                     FROM files f \
                     WHERE (f.display_name LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'){} \
                     ORDER BY RANDOM() LIMIT {}",
                    where_tail, limit
                ),
                pattern.clone(),
            ),
        };

        let mut row_stmt = sqlx::query_as::<_, FileEntry>(&row_query).bind(&bind_value);
        if matches!(search_filter, SearchFilter::Like(_)) {
            row_stmt = row_stmt.bind(&bind_value);
        }
        for b in &filter_binds {
            row_stmt = row_stmt.bind(b);
        }
        row_stmt.fetch_all(&pool).await.map_err(|e| e.to_string())?
    } else {
        let row_query = format!(
            "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.local_cache_path, f.is_favorite, f.created_at, f.updated_at \
             FROM files f \
             WHERE 1=1{} \
             ORDER BY RANDOM() LIMIT {}",
            where_tail, limit
        );
        let mut row_stmt = sqlx::query_as::<_, FileEntry>(&row_query);
        for b in &filter_binds {
            row_stmt = row_stmt.bind(b);
        }
        row_stmt.fetch_all(&pool).await.map_err(|e| e.to_string())?
    };

    hydrate_file_items(&pool, files).await
}

#[tauri::command]
pub async fn file_check_status(
    app: AppHandle,
    file_ids: Option<Vec<i64>>,
) -> Result<FileCheckStatusResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let files: Vec<FileEntry> = match file_ids {
        Some(ids) => {
            let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!(
                "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, is_favorite, created_at, updated_at FROM files WHERE id IN ({})",
                placeholders
            );
            sqlx::query_as(&query)
                .fetch_all(&pool)
                .await
                .map_err(|e| e.to_string())?
        }
        None => {
            sqlx::query_as("SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, is_favorite, created_at, updated_at FROM files")
                .fetch_all(&pool)
                .await
                .map_err(|e| e.to_string())?
        }
    };

    // Resolve stored relative path to absolute before stat-ing. Without
    // the resolve, `Path::exists()` would test a non-existent relative
    // path and mark every row as 'missing' once paths went root-relative.
    //
    // Remote-row behavior CHANGED with the path-relativization refactor:
    // previously this
    // function stat'd every row's path verbatim, which for remote rows
    // was a Baidu provider path (e.g. `/apps/biblio/<base64>.cbz`) that
    // never exists locally → every remote was incorrectly marked
    // 'missing' on every check. The branch below treats remote rows as
    // 'available' unless a local cache exists AND that cache file is
    // gone. Either case (no cache, or cache present and intact) leaves
    // the row available; only a missing cache flips a remote to
    // 'missing'. This matches "is the user's view of this row still
    // usable?" — what the file_status column actually means.
    let roots = super::settings::load_path_roots(&pool).await?;

    let mut updated = Vec::new();
    for file in files {
        let kind = file.storage_kind.as_deref().unwrap_or("local");
        let abs = crate::path_resolve::to_absolute(
            kind,
            &file.path,
            &roots.storage_path,
            &roots.app_root,
        );
        let exists = if kind == "remote" {
            file.local_cache_path
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(|p| crate::path_resolve::cache_to_absolute(p, &roots.storage_path).exists())
                .unwrap_or(true)
        } else {
            abs.exists()
        };
        let new_status = if exists { "available" } else { "missing" };

        if file.file_status != new_status {
            sqlx::query("UPDATE files SET file_status = ? WHERE id = ?")
                .bind(new_status)
                .bind(file.id)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;

            updated.push(FileStatusUpdate {
                id: file.id,
                status: new_status.to_string(),
            });
        }
    }

    Ok(FileCheckStatusResponse { updated })
}

#[derive(Serialize)]
pub struct FileStatusUpdate {
    pub id: i64,
    pub status: String,
}

#[derive(Serialize)]
pub struct FileCheckStatusResponse {
    pub updated: Vec<FileStatusUpdate>,
}

#[tauri::command]
pub async fn file_replace(
    app: AppHandle,
    existing_file_id: i64,
    path: String,
    display_name: String,
    category_id: Option<i64>,
    tag_ids: Option<Vec<i64>>,
    author_ids: Option<Vec<i64>>,
    metadata: Option<Vec<MetadataInput>>,
    progress: Option<String>,
    is_favorite: Option<bool>,
    cover_data: Option<Vec<u8>>,
    cover_mime_type: Option<String>,
    staged_cover_path: Option<String>,
) -> Result<FileCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Validate the replacement source EXISTS before destroying the old copy.
    // file_replace deletes the existing remote/local file + DB row and only
    // then calls file_create — so if the new source has vanished (a stale
    // Finder drag, an unmounted volume) the old library entry would be lost
    // with no replacement. Failing here keeps the existing entry intact.
    if !std::path::Path::new(&path).exists() {
        return Err("SOURCE_FILE_NOT_FOUND".into());
    }

    let existing: Option<(String, bool, String)> =
        sqlx::query_as("SELECT path, in_storage, storage_kind FROM files WHERE id = ?")
            .bind(existing_file_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

    if let Some((existing_stored, in_storage, storage_kind)) = existing {
        // Resolve relative → absolute before the external op. Same shape
        // as file_delete; see comments there.
        let roots = super::settings::load_path_roots(&pool).await?;
        let existing_path = crate::path_resolve::to_absolute(
            &storage_kind,
            &existing_stored,
            &roots.storage_path,
            &roots.app_root,
        )
        .to_string_lossy()
        .to_string();

        // Strict ordering, same as `file_delete`: prove the prior copy is
        // gone before we drop the DB row. Without this, a remote duplicate
        // resolved via "Replace" used to leave the Baidu file orphaned
        // (in_storage=false skipped the local branch, the DB row went
        // away, the cloud copy stayed).
        if storage_kind == "remote" {
            super::remote::delete_on_remote_for_file(&pool, existing_file_id, &existing_path)
                .await?;
        } else if in_storage {
            if let Err(e) = fs::remove_file(&existing_path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(format!("Failed to remove existing file: {e}"));
                }
            }
        }

        sqlx::query("DELETE FROM files WHERE id = ?")
            .bind(existing_file_id)
            .execute(&pool)
            .await
            .map_err(|e| format!("Failed to delete existing file: {e}"))?;
    }

    file_create(
        app,
        path,
        display_name,
        category_id,
        tag_ids,
        author_ids,
        metadata,
        progress,
        is_favorite,
        cover_data,
        cover_mime_type,
        staged_cover_path,
    )
    .await
}
