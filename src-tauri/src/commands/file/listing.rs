use super::*;

/// Paginated file list with optional free-text `query`.
///
/// Blank / absent `query` → plain list. Non-blank applies the FTS5 / LIKE
/// filter from [`prepare_search_filter`]. A non-blank query that sanitizes
/// to nothing (all punctuation) returns an empty page.
#[tauri::command]
pub async fn file_list(
    app: AppHandle,
    category_id: Option<i64>,
    _tag_ids: Option<Vec<i64>>,
    status: Option<String>,
    sort_by: Option<String>,
    sort_desc: Option<bool>,
    conditions: Option<Vec<FilterCondition>>,
    query: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
) -> Result<FileListResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    // Always alias `files` as `f` so the list path and the FTS join path
    // share one SQL shape — `field:<key>` sorts and filter SQL both need it.
    // Default mirrors the previous hardcoded `ORDER BY created_at DESC`.
    let sort_desc = sort_desc.unwrap_or(true);
    let (sort_join, order_by) = match sort_by.as_deref() {
        Some(s) if s.starts_with("field:") => {
            match custom_sort_clause(&pool, s, sort_desc, "f").await {
                Some((join, order)) => (join, order),
                None => (String::new(), order_by_clause(sort_by.as_deref(), sort_desc, "f")),
            }
        }
        _ => (String::new(), order_by_clause(sort_by.as_deref(), sort_desc, "f")),
    };
    let (filter_sql, filter_binds) = build_filter_sql(conditions.as_deref().unwrap_or(&[]), "f");

    // Text filter resolution:
    //   - None / whitespace → no text predicate (plain list)
    //   - non-blank that sanitizes to nothing → empty result set
    //   - otherwise → FTS or LIKE from prepare_search_filter
    let text_filter = match query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        None => None,
        Some(q) => match prepare_search_filter(q) {
            None => {
                return Ok(FileListResponse {
                    files: Vec::new(),
                    total: 0,
                });
            }
            Some(f) => Some(f),
        },
    };

    // Shared WHERE tail for both the row query and the count query so
    // `total` matches what's loadable (pagination "N remaining" correctness).
    let mut where_tail = String::new();
    if let Some(cat_id) = category_id {
        where_tail.push_str(&format!(" AND f.category_id = {}", cat_id));
    }
    if let Some(s) = &status {
        // Allow-list before interpolating — crosses the IPC boundary as an
        // arbitrary String (the TS `FileStatus` union is compile-time only).
        if !matches!(s.as_str(), "available" | "missing" | "moved") {
            return Err(format!("Invalid file_status filter: {s}"));
        }
        where_tail.push_str(&format!(" AND f.file_status = '{}'", s));
    }
    where_tail.push_str(&filter_sql);

    const SELECT_COLS: &str = "f.id, f.path, f.display_name, f.category_id, f.file_status, \
        f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, \
        f.local_cache_path, f.is_favorite, f.created_at, f.updated_at";

    let (files, total) = match &text_filter {
        Some(SearchFilter::Fts(expr)) => {
            let row_query = format!(
                "SELECT {SELECT_COLS} FROM files f \
                 JOIN files_fts ON files_fts.rowid = f.id{sort_join} \
                 WHERE files_fts MATCH ?{where_tail} \
                 {order_by} LIMIT {limit} OFFSET {offset}"
            );
            let count_query = format!(
                "SELECT COUNT(*) FROM files f \
                 JOIN files_fts ON files_fts.rowid = f.id \
                 WHERE files_fts MATCH ?{where_tail}"
            );
            let mut row_stmt = sqlx::query_as::<_, FileEntry>(&row_query).bind(expr);
            for b in &filter_binds {
                row_stmt = row_stmt.bind(b);
            }
            let files: Vec<FileEntry> = row_stmt.fetch_all(&pool).await.map_err(|e| e.to_string())?;

            let mut count_stmt = sqlx::query_as::<_, (i64,)>(&count_query).bind(expr);
            for b in &filter_binds {
                count_stmt = count_stmt.bind(b);
            }
            let total: (i64,) = count_stmt
                .fetch_one(&pool)
                .await
                .map_err(|e| e.to_string())?;
            (files, total.0)
        }
        Some(SearchFilter::Like(pattern)) => {
            let row_query = format!(
                "SELECT {SELECT_COLS} FROM files f{sort_join} \
                 WHERE (f.display_name LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'){where_tail} \
                 {order_by} LIMIT {limit} OFFSET {offset}"
            );
            let count_query = format!(
                "SELECT COUNT(*) FROM files f \
                 WHERE (f.display_name LIKE ? ESCAPE '\\' OR f.path LIKE ? ESCAPE '\\'){where_tail}"
            );
            let mut row_stmt = sqlx::query_as::<_, FileEntry>(&row_query)
                .bind(pattern)
                .bind(pattern);
            for b in &filter_binds {
                row_stmt = row_stmt.bind(b);
            }
            let files: Vec<FileEntry> = row_stmt.fetch_all(&pool).await.map_err(|e| e.to_string())?;

            let mut count_stmt = sqlx::query_as::<_, (i64,)>(&count_query)
                .bind(pattern)
                .bind(pattern);
            for b in &filter_binds {
                count_stmt = count_stmt.bind(b);
            }
            let total: (i64,) = count_stmt
                .fetch_one(&pool)
                .await
                .map_err(|e| e.to_string())?;
            (files, total.0)
        }
        None => {
            let row_query = format!(
                "SELECT {SELECT_COLS} FROM files f{sort_join} \
                 WHERE 1=1{where_tail} \
                 {order_by} LIMIT {limit} OFFSET {offset}"
            );
            let count_query =
                format!("SELECT COUNT(*) FROM files f WHERE 1=1{where_tail}");
            let mut row_stmt = sqlx::query_as::<_, FileEntry>(&row_query);
            for b in &filter_binds {
                row_stmt = row_stmt.bind(b);
            }
            let files: Vec<FileEntry> = row_stmt.fetch_all(&pool).await.map_err(|e| e.to_string())?;

            let mut count_stmt = sqlx::query_as::<_, (i64,)>(&count_query);
            for b in &filter_binds {
                count_stmt = count_stmt.bind(b);
            }
            let total: (i64,) = count_stmt
                .fetch_one(&pool)
                .await
                .map_err(|e| e.to_string())?;
            (files, total.0)
        }
    };

    let file_items = hydrate_file_items(&pool, files).await?;

    Ok(FileListResponse {
        files: file_items,
        total,
    })
}

#[derive(Serialize)]
pub struct FileListResponse {
    pub files: Vec<FileListItem>,
    pub total: i64,
}

/// Hydrate a list of `FileEntry` rows into `FileListItem`s by fetching their
/// associated tags and authors. Matches the per-file loop used inside `file_list`.
pub(super) async fn hydrate_file_items(
    pool: &sqlx::SqlitePool,
    files: Vec<FileEntry>,
) -> Result<Vec<FileListItem>, String> {
    // Same boundary-resolve shape as file_list — load roots once, then
    // rebuild absolute paths from the stored relative values per row.
    let roots = crate::commands::settings::load_path_roots(pool).await?;

    let mut items = Vec::with_capacity(files.len());
    for file in files {
        let tags: Vec<Tag> = sqlx::query_as(
            "SELECT t.id, t.name, t.color, t.created_at FROM tags t
             INNER JOIN file_tags ft ON t.id = ft.tag_id WHERE ft.file_id = ?",
        )
        .bind(file.id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let authors: Vec<Author> = sqlx::query_as(
            "SELECT a.id, a.name, a.created_at FROM authors a
             INNER JOIN file_authors fa ON a.id = fa.author_id WHERE fa.file_id = ?",
        )
        .bind(file.id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let storage_kind_ref = file.storage_kind.as_deref().unwrap_or("local");
        let abs_path = crate::path_resolve::to_absolute(
            storage_kind_ref,
            &file.path,
            &roots.storage_path,
            &roots.app_root,
        )
        .to_string_lossy()
        .to_string();
        let abs_cache = file.local_cache_path.as_ref().map(|p| {
            crate::path_resolve::cache_to_absolute(p, &roots.storage_path)
                .to_string_lossy()
                .to_string()
        });

        items.push(FileListItem {
            id: file.id,
            path: abs_path,
            display_name: file.display_name,
            category_id: file.category_id,
            file_status: file.file_status,
            in_storage: file.in_storage,
            original_path: file.original_path,
            progress: file.progress,
            storage_kind: file.storage_kind,
            remote_provider: file.remote_provider,
            local_cache_path: abs_cache,
            is_favorite: file.is_favorite,
            created_at: file.created_at,
            updated_at: file.updated_at,
            tags,
            authors,
        });
    }
    Ok(items)
}

/// Fetch a set of files by id, hydrated with tags/authors so the
/// frontend's `fileStore.byId` can be populated directly. Used by the
/// collection drill-down: `collection_list` returns ids only; the route
/// hydrates the drilled-into collection's rows on click so the FileList
/// grid finds them in `byId` regardless of which page they sit on in
/// the main paginated view. Also used for the post-fetch cover-row
/// hydration so novel collection cards can render `<NovelCover>` with
/// the picked member's tags + display_name.
#[tauri::command]
pub async fn file_list_by_ids(app: AppHandle, ids: Vec<i64>) -> Result<Vec<FileListItem>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // SQLite has a 999-parameter limit; chunk to stay safely under it. For
    // the comic-drill use case `ids.len()` is rarely above a few hundred,
    // but the chunked loop keeps the command honest for larger drills.
    const CHUNK: usize = 500;
    let mut files: Vec<FileEntry> = Vec::with_capacity(ids.len());
    for chunk in ids.chunks(CHUNK) {
        let placeholders = std::iter::repeat("?")
            .take(chunk.len())
            .collect::<Vec<_>>()
            .join(",");
        let query = format!(
            "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status,
                    f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.local_cache_path, f.is_favorite, f.created_at, f.updated_at
             FROM files f WHERE f.id IN ({placeholders})",
        );
        let mut q = sqlx::query_as::<_, FileEntry>(&query);
        for id in chunk {
            q = q.bind(id);
        }
        let mut rows: Vec<FileEntry> = q
            .fetch_all(&pool)
            .await
            .map_err(|e| format!("Failed to load files by ids: {e}"))?;
        files.append(&mut rows);
    }
    hydrate_file_items(&pool, files).await
}
