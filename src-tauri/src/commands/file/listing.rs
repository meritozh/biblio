use super::*;
#[tauri::command]
pub async fn file_list(
    app: AppHandle,
    category_id: Option<i64>,
    _tag_ids: Option<Vec<i64>>,
    status: Option<String>,
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
    // Default mirrors the previous hardcoded `ORDER BY created_at DESC` so
    // callers that don't pass sort options keep their existing ordering.
    let order_by = order_by_clause(sort_by.as_deref(), sort_desc.unwrap_or(true), "");
    let (filter_sql, filter_binds) = build_filter_sql(conditions.as_deref().unwrap_or(&[]), "");

    // Build the WHERE clause once so both the row query and the count query
    // see the same filter — otherwise `total` misreports what's loadable,
    // which breaks pagination ("N remaining" stays non-zero forever).
    let mut where_clause = String::from(" WHERE 1=1");
    if let Some(cat_id) = category_id {
        where_clause.push_str(&format!(" AND category_id = {}", cat_id));
    }
    if let Some(s) = &status {
        // Allow-list the status value before interpolating — it crosses the
        // IPC boundary as an arbitrary String (the TS `FileStatus` union is
        // compile-time only), so an unchecked value here is a SQL-injection
        // sink. Mirrors the airtight guard in `build_filter_sql`. Unknown
        // values are rejected rather than silently dropped.
        if !matches!(s.as_str(), "available" | "missing" | "moved") {
            return Err(format!("Invalid file_status filter: {s}"));
        }
        where_clause.push_str(&format!(" AND file_status = '{}'", s));
    }
    where_clause.push_str(&filter_sql);

    let row_query = format!(
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, is_favorite, created_at, updated_at FROM files{} {} LIMIT {} OFFSET {}",
        where_clause, order_by, limit, offset
    );
    let mut row_stmt = sqlx::query_as::<_, FileEntry>(&row_query);
    for b in &filter_binds {
        row_stmt = row_stmt.bind(b);
    }
    let files: Vec<FileEntry> = row_stmt.fetch_all(&pool).await.map_err(|e| e.to_string())?;

    let count_query = format!("SELECT COUNT(*) FROM files{}", where_clause);
    let mut count_stmt = sqlx::query_as::<_, (i64,)>(&count_query);
    for b in &filter_binds {
        count_stmt = count_stmt.bind(b);
    }
    let total: (i64,) = count_stmt
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    // Resolve stored relative paths → absolute at the IPC boundary so
    // every TS consumer keeps seeing the same shape it always has.
    // Single roots lookup amortizes across every row in this list.
    let roots = crate::commands::settings::load_path_roots(&pool).await?;

    let mut file_items = Vec::with_capacity(files.len());
    for file in files {
        let tags: Vec<Tag> = sqlx::query_as(
            "SELECT t.id, t.name, t.color, t.created_at FROM tags t
             INNER JOIN file_tags ft ON t.id = ft.tag_id WHERE ft.file_id = ?",
        )
        .bind(file.id)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

        let authors: Vec<Author> = sqlx::query_as(
            "SELECT a.id, a.name, a.created_at FROM authors a
             INNER JOIN file_authors fa ON a.id = fa.author_id WHERE fa.file_id = ?",
        )
        .bind(file.id)
        .fetch_all(&pool)
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

        file_items.push(FileListItem {
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

    Ok(FileListResponse {
        files: file_items,
        total: total.0,
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

// ── Duplicate-name detection ──────────────────────────────────────────────────
//
// The /cleanup page on the frontend uses these defaults verbatim. If you
// change a value here, update the matching constant in
// `src/routes/cleanup.tsx` (`DEFAULT_MIN_PREFIX_CHARS` / `DEFAULT_PREFIX_RATIO`)
// so the popover's initial state still reflects what's actually applied.
