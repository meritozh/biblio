use crate::commands::*;
use crate::commands::validation::{validate_display_name, sanitize_folder_name};
use serde::Serialize;
use tauri::AppHandle;
use tauri::{Emitter, Manager};
use tauri_plugin_sql::{DbPool, DbInstances};
use std::path::PathBuf;
use std::fs;

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

/// Build the `ORDER BY` fragment for a file listing query. The column whitelist
/// keeps the caller from injecting arbitrary SQL via `sort_by`. `alias` is the
/// table alias (`""` for unaliased, `"f"` when joining FTS); the trailing dot is
/// added automatically when present.
fn order_by_clause(sort_by: Option<&str>, sort_desc: bool, alias: &str) -> String {
    let prefix = if alias.is_empty() {
        String::new()
    } else {
        format!("{}.", alias)
    };
    let column = match sort_by.unwrap_or("created") {
        "name" => format!("LOWER({}display_name)", prefix),
        "updated" => format!("{}updated_at", prefix),
        // "created" and any unknown value fall back to created_at, matching the
        // historical default before per-call sorting was wired up.
        _ => format!("{}created_at", prefix),
    };
    let direction = if sort_desc { "DESC" } else { "ASC" };
    format!("ORDER BY {} {}", column, direction)
}

/// One row of the FilterPanel editor, deserialized loosely so a single shape
/// covers the whole TS discriminated union. `field` and `op` together pick
/// which of the value-bearing fields are read; the rest are ignored. The
/// translator silently skips conditions whose value is missing — matches the
/// frontend's "half-built row is a no-op" rule.
#[derive(serde::Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct FilterCondition {
    pub field: String,
    pub op: String,
    pub n: Option<i64>,
    pub tag_id: Option<i64>,
    /// `includes_any` / `excludes_any` carry a tag set. Empty / absent
    /// behaves the same as a missing `tag_id` on the single-tag ops: the
    /// condition is treated as a no-op so a half-built editor row
    /// doesn't suddenly hide every file.
    pub tag_ids: Option<Vec<i64>>,
    /// Used by `authors includes` to point at a single author. Kept
    /// separate from `tag_id` so the wire shape stays self-describing
    /// — the discriminator is still `(field, op)`, but each id field
    /// names the entity it refers to.
    pub author_id: Option<i64>,
    pub text: Option<String>,
    pub value: Option<String>,
}

/// Translate a list of `FilterCondition`s into a SQL fragment plus an ordered
/// list of bind values. The fragment is appended verbatim after an existing
/// `WHERE`-clause start, so it always begins with ` AND `. Caller binds the
/// returned values in order on both the row query and the count query.
///
/// Only string-valued conditions go through bind; integer values (counts,
/// tag ids) are formatted directly because they're typed `i64` and can't
/// inject. Enum values (`file_status`, `storage_kind`) are whitelisted before
/// formatting so an attacker controlling the JSON can't smuggle SQL through.
fn build_filter_sql(
    conditions: &[FilterCondition],
    alias: &str,
) -> (String, Vec<String>) {
    let prefix = if alias.is_empty() {
        String::new()
    } else {
        format!("{}.", alias)
    };
    let mut sql = String::new();
    let mut binds: Vec<String> = Vec::new();
    for c in conditions {
        match c.field.as_str() {
            "authors" => match c.op.as_str() {
                "empty" => sql.push_str(&format!(
                    " AND NOT EXISTS (SELECT 1 FROM file_authors WHERE file_id = {p}id)",
                    p = prefix
                )),
                "not_empty" => sql.push_str(&format!(
                    " AND EXISTS (SELECT 1 FROM file_authors WHERE file_id = {p}id)",
                    p = prefix
                )),
                "count_gte" => {
                    if let Some(n) = c.n {
                        sql.push_str(&format!(
                            " AND (SELECT COUNT(*) FROM file_authors WHERE file_id = {p}id) >= {n}",
                            p = prefix,
                            n = n
                        ));
                    }
                }
                "count_lt" => {
                    if let Some(n) = c.n {
                        sql.push_str(&format!(
                            " AND (SELECT COUNT(*) FROM file_authors WHERE file_id = {p}id) < {n}",
                            p = prefix,
                            n = n
                        ));
                    }
                }
                "includes" => {
                    if let Some(a) = c.author_id {
                        sql.push_str(&format!(
                            " AND EXISTS (SELECT 1 FROM file_authors WHERE file_id = {p}id AND author_id = {a})",
                            p = prefix,
                            a = a
                        ));
                    }
                }
                _ => {}
            },
            "tags" => match c.op.as_str() {
                "empty" => sql.push_str(&format!(
                    " AND NOT EXISTS (SELECT 1 FROM file_tags WHERE file_id = {p}id)",
                    p = prefix
                )),
                "not_empty" => sql.push_str(&format!(
                    " AND EXISTS (SELECT 1 FROM file_tags WHERE file_id = {p}id)",
                    p = prefix
                )),
                "count_gte" => {
                    if let Some(n) = c.n {
                        sql.push_str(&format!(
                            " AND (SELECT COUNT(*) FROM file_tags WHERE file_id = {p}id) >= {n}",
                            p = prefix,
                            n = n
                        ));
                    }
                }
                "count_lt" => {
                    if let Some(n) = c.n {
                        sql.push_str(&format!(
                            " AND (SELECT COUNT(*) FROM file_tags WHERE file_id = {p}id) < {n}",
                            p = prefix,
                            n = n
                        ));
                    }
                }
                "includes" => {
                    if let Some(t) = c.tag_id {
                        sql.push_str(&format!(
                            " AND EXISTS (SELECT 1 FROM file_tags WHERE file_id = {p}id AND tag_id = {t})",
                            p = prefix,
                            t = t
                        ));
                    }
                }
                "excludes" => {
                    if let Some(t) = c.tag_id {
                        sql.push_str(&format!(
                            " AND NOT EXISTS (SELECT 1 FROM file_tags WHERE file_id = {p}id AND tag_id = {t})",
                            p = prefix,
                            t = t
                        ));
                    }
                }
                // `_any` ops: build an IN list inline. `tag_ids` is typed
                // `Vec<i64>` so values can't inject; empty list = no-op
                // (skip emitting any AND) to match the frontend.
                "includes_any" => {
                    if let Some(ids) = c.tag_ids.as_ref().filter(|v| !v.is_empty()) {
                        let list = ids
                            .iter()
                            .map(|n| n.to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        sql.push_str(&format!(
                            " AND EXISTS (SELECT 1 FROM file_tags WHERE file_id = {p}id AND tag_id IN ({list}))",
                            p = prefix,
                            list = list
                        ));
                    }
                }
                "excludes_any" => {
                    if let Some(ids) = c.tag_ids.as_ref().filter(|v| !v.is_empty()) {
                        let list = ids
                            .iter()
                            .map(|n| n.to_string())
                            .collect::<Vec<_>>()
                            .join(", ");
                        sql.push_str(&format!(
                            " AND NOT EXISTS (SELECT 1 FROM file_tags WHERE file_id = {p}id AND tag_id IN ({list}))",
                            p = prefix,
                            list = list
                        ));
                    }
                }
                _ => {}
            },
            "progress" => match c.op.as_str() {
                "empty" => sql.push_str(&format!(
                    " AND ({p}progress IS NULL OR TRIM({p}progress) = '')",
                    p = prefix
                )),
                "not_empty" => sql.push_str(&format!(
                    " AND {p}progress IS NOT NULL AND TRIM({p}progress) <> ''",
                    p = prefix
                )),
                "contains" => {
                    if let Some(t) = c.text.as_ref().filter(|s| !s.is_empty()) {
                        sql.push_str(&format!(
                            " AND LOWER({p}progress) LIKE ? ESCAPE '\\'",
                            p = prefix
                        ));
                        // Escape the LIKE meta-characters so the user's text
                        // is matched literally; `%text%` then performs a
                        // case-insensitive substring search.
                        let escaped = t
                            .to_lowercase()
                            .replace('\\', "\\\\")
                            .replace('%', "\\%")
                            .replace('_', "\\_");
                        binds.push(format!("%{}%", escaped));
                    }
                }
                _ => {}
            },
            // SQLite's LENGTH() on TEXT returns characters (not bytes), so the
            // comparison matches the user's intuition for CJK + Latin titles.
            // Frontend uses JS String.length (UTF-16 code units) which agrees
            // for everything outside surrogate-pair characters — close enough
            // for a coarse filter.
            "display_name" => match c.op.as_str() {
                "length_gte" => {
                    if let Some(n) = c.n {
                        sql.push_str(&format!(
                            " AND LENGTH({p}display_name) >= {n}",
                            p = prefix,
                            n = n
                        ));
                    }
                }
                "length_lt" => {
                    if let Some(n) = c.n {
                        sql.push_str(&format!(
                            " AND LENGTH({p}display_name) < {n}",
                            p = prefix,
                            n = n
                        ));
                    }
                }
                _ => {}
            },
            "file_status" => {
                if c.op == "is" {
                    if let Some(v) = c.value.as_deref() {
                        if matches!(v, "available" | "missing" | "moved") {
                            sql.push_str(&format!(
                                " AND {p}file_status = '{v}'",
                                p = prefix,
                                v = v
                            ));
                        }
                    }
                }
            }
            "storage_kind" => {
                if c.op == "is" {
                    if let Some(v) = c.value.as_deref() {
                        if matches!(v, "local" | "remote") {
                            sql.push_str(&format!(
                                " AND {p}storage_kind = '{v}'",
                                p = prefix,
                                v = v
                            ));
                        }
                    }
                }
            }
            // Direct nullness test on `local_cache_path`. `not_empty` is the
            // "files I've pulled to the local cache" pill (remote rows with a
            // path); `empty` covers local-only rows AND uncached remotes —
            // compose with `storage_kind = remote` to narrow to either side.
            "local_cache" => match c.op.as_str() {
                "empty" => sql.push_str(&format!(
                    " AND ({p}local_cache_path IS NULL OR {p}local_cache_path = '')",
                    p = prefix
                )),
                "not_empty" => sql.push_str(&format!(
                    " AND {p}local_cache_path IS NOT NULL AND {p}local_cache_path <> ''",
                    p = prefix
                )),
                _ => {}
            },
            _ => {}
        }
    }
    (sql, binds)
}

/// Generate a unique filename if file already exists
fn get_unique_destination(dest: &std::path::Path) -> PathBuf {
    if !dest.exists() {
        return dest.to_path_buf();
    }

    let parent = dest.parent().unwrap_or(std::path::Path::new("."));
    let stem = dest.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = dest.extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let mut counter = 1;
    loop {
        let new_name = if ext.is_empty() {
            format!("{} ({})", stem, counter)
        } else {
            format!("{} ({}){}", stem, counter, ext)
        };
        let new_path = parent.join(&new_name);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}

/// Build a clean filename from metadata: "<display_name> <progress> <authors>.ext"
pub fn build_novel_filename(
    display_name: &str,
    progress: Option<&str>,
    author_names: &[String],
    ext_with_dot: &str,
) -> String {
    let mut parts: Vec<String> = vec![display_name.to_string()];
    if let Some(p) = progress {
        if !p.is_empty() {
            parts.push(p.to_string());
        }
    }
    if !author_names.is_empty() {
        parts.push(author_names.join(", "));
    }
    format!("{}{}", parts.join(" "), ext_with_dot)
}

/// Remove filesystem-invalid characters from a filename
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect()
}

/// Copy a file to destination
/// Returns the final destination path
fn copy_file(source: &std::path::Path, dest: &std::path::Path) -> Result<PathBuf, String> {
    let final_dest = get_unique_destination(dest);

    fs::copy(source, &final_dest)
        .map_err(|e| {
            let err_str = e.to_string().to_lowercase();
            if err_str.contains("permission denied") {
                "PERMISSION_DENIED".to_string()
            } else if err_str.contains("disk full") || err_str.contains("no space") {
                "DISK_FULL".to_string()
            } else if err_str.contains("being used") || err_str.contains("locked") {
                "FILE_LOCKED".to_string()
            } else {
                format!("Failed to copy file: {}", e)
            }
        })?;

    Ok(final_dest)
}

/// Write every image file under `source_dir` (recursively, sorted) into
/// a `.zip` at `dest`. Stored (no compression) — comic images are already
/// JPEG/PNG/WebP, so deflate burns CPU for ~0% gain. Returns the final
/// destination path (after any unique-name disambiguation). Hidden files
/// and non-image files are skipped, matching the importer's collapse
/// rule.
fn zip_image_dir(source_dir: &std::path::Path, dest: &std::path::Path) -> Result<PathBuf, String> {
    use crate::pipeline::archive::is_image_filename;
    use std::io::Write;

    let final_dest = get_unique_destination(dest);
    let f = std::fs::File::create(&final_dest)
        .map_err(|e| format!("Failed to create zip: {e}"))?;
    let mut zw = zip::ZipWriter::new(f);
    let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    fn walk(
        root: &std::path::Path,
        dir: &std::path::Path,
        zw: &mut zip::ZipWriter<std::fs::File>,
        opts: &zip::write::SimpleFileOptions,
    ) -> Result<(), String> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read {}: {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                walk(root, &p, zw, opts)?;
            } else if p.is_file() && is_image_filename(&name_str) {
                let rel = p
                    .strip_prefix(root)
                    .map_err(|e| format!("strip_prefix: {e}"))?;
                // ZIP paths use forward slashes by spec.
                let zip_name = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join("/");
                zw.start_file(zip_name, *opts)
                    .map_err(|e| format!("zip start_file: {e}"))?;
                let bytes = std::fs::read(&p)
                    .map_err(|e| format!("read {}: {e}", p.display()))?;
                zw.write_all(&bytes)
                    .map_err(|e| format!("zip write: {e}"))?;
            }
        }
        Ok(())
    }

    walk(source_dir, source_dir, &mut zw, &opts)?;
    zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    Ok(final_dest)
}

/// Archive the WHOLE directory tree under `source_dir` into a `.zip` at
/// `dest`, verbatim — used for galgame folder imports, where a game is more
/// than its images (executables, scripts, audio, and the on-disk directory
/// layout all matter). The ZIP format is a flat list of entries, so the tree
/// is enumerated; every file AND every directory (including empty ones) is
/// recorded so the extracted game is byte-for-byte the same shape.
///
/// File bytes are streamed with `io::copy` rather than read whole into RAM —
/// a multi-GB game file would otherwise spike memory (see the `MimeDetectNode`
/// note about per-import RAM growth). Stored (no compression): game assets are
/// already compressed, so deflate burns CPU for ~0% gain. OS-metadata junk
/// (`.DS_Store`, `Thumbs.db`, `desktop.ini`) is skipped via the shared
/// `is_ignorable_metadata` predicate. Returns the final destination path after
/// any unique-name disambiguation.
fn zip_dir(source_dir: &std::path::Path, dest: &std::path::Path) -> Result<PathBuf, String> {
    use crate::pipeline::archive::is_ignorable_metadata;

    let final_dest = get_unique_destination(dest);
    let f = std::fs::File::create(&final_dest)
        .map_err(|e| format!("Failed to create zip: {e}"))?;
    let mut zw = zip::ZipWriter::new(f);
    let opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    /// Build the forward-slash ZIP entry name for `path` relative to `root`.
    fn zip_name(root: &std::path::Path, path: &std::path::Path) -> Result<String, String> {
        let rel = path
            .strip_prefix(root)
            .map_err(|e| format!("strip_prefix: {e}"))?;
        Ok(rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/"))
    }

    fn walk(
        root: &std::path::Path,
        dir: &std::path::Path,
        zw: &mut zip::ZipWriter<std::fs::File>,
        opts: &zip::write::SimpleFileOptions,
    ) -> Result<(), String> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .map_err(|e| format!("Failed to read {}: {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if is_ignorable_metadata(&name_str) {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                // Record the directory entry so empty dirs the game expects at
                // runtime (e.g. `save/`) survive the round-trip, then recurse.
                zw.add_directory(zip_name(root, &p)?, *opts)
                    .map_err(|e| format!("zip add_directory: {e}"))?;
                walk(root, &p, zw, opts)?;
            } else if p.is_file() {
                zw.start_file(zip_name(root, &p)?, *opts)
                    .map_err(|e| format!("zip start_file: {e}"))?;
                // Stream the file rather than slurping it — keeps RAM flat for
                // multi-GB game assets.
                let mut src = std::fs::File::open(&p)
                    .map_err(|e| format!("open {}: {e}", p.display()))?;
                std::io::copy(&mut src, zw)
                    .map_err(|e| format!("zip write {}: {e}", p.display()))?;
            }
        }
        Ok(())
    }

    walk(source_dir, source_dir, &mut zw, &opts)?;
    zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    Ok(final_dest)
}

/// Move a file, handling cross-drive moves
/// Returns the final destination path
fn move_file(source: &std::path::Path, dest: &std::path::Path) -> Result<PathBuf, String> {
    let final_dest = get_unique_destination(dest);

    // Try rename first (fast, same filesystem)
    if fs::rename(source, &final_dest).is_ok() {
        return Ok(final_dest);
    }

    // Fall back to copy + delete (cross-drive)
    fs::copy(source, &final_dest)
        .map_err(|e| {
            // Map common filesystem errors to user-friendly codes
            let err_str = e.to_string().to_lowercase();
            if err_str.contains("permission denied") {
                "PERMISSION_DENIED".to_string()
            } else if err_str.contains("disk full") || err_str.contains("no space") {
                "DISK_FULL".to_string()
            } else if err_str.contains("being used") || err_str.contains("locked") {
                "FILE_LOCKED".to_string()
            } else {
                format!("Failed to copy file: {}", e)
            }
        })?;

    fs::remove_file(source)
        .map_err(|e| format!("Failed to remove original: {}", e))?;

    Ok(final_dest)
}

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
    let (filter_sql, filter_binds) = build_filter_sql(
        conditions.as_deref().unwrap_or(&[]),
        "",
    );

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
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, created_at, updated_at FROM files{} {} LIMIT {} OFFSET {}",
        where_clause, order_by, limit, offset
    );
    let mut row_stmt = sqlx::query_as::<_, FileEntry>(&row_query);
    for b in &filter_binds {
        row_stmt = row_stmt.bind(b);
    }
    let files: Vec<FileEntry> = row_stmt
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

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
             INNER JOIN file_tags ft ON t.id = ft.tag_id WHERE ft.file_id = ?"
        )
        .bind(file.id)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

        let authors: Vec<Author> = sqlx::query_as(
            "SELECT a.id, a.name, a.created_at FROM authors a
             INNER JOIN file_authors fa ON a.id = fa.author_id WHERE fa.file_id = ?"
        )
        .bind(file.id)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;

        let storage_kind_ref = file.storage_kind.as_deref().unwrap_or("local");
        let abs_path = crate::path_resolve::to_absolute(
            storage_kind_ref, &file.path, &roots.storage_path, &roots.app_root,
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
async fn hydrate_file_items(
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
            storage_kind_ref, &file.path, &roots.storage_path, &roots.app_root,
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
pub async fn file_list_by_ids(
    app: AppHandle,
    ids: Vec<i64>,
) -> Result<Vec<FileListItem>, String> {
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
        let placeholders = std::iter::repeat("?").take(chunk.len()).collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status,
                    f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.local_cache_path, f.created_at, f.updated_at
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

const DEFAULT_MIN_PREFIX_CHARS: usize = 3;
const DEFAULT_PREFIX_RATIO: f32 = 0.5;

#[derive(Serialize)]
pub struct DuplicateGroup {
    /// The longest shared prefix across all files in this group, in the
    /// original case of the first file. Useful as a group label.
    pub prefix: String,
    /// Hydrated file rows (tags + authors joined) so the frontend can
    /// render the cleanup card without a follow-up IPC.
    pub files: Vec<FileListItem>,
}

/// Case-insensitive longest common prefix. Folds ASCII letters; CJK and
/// other scripts compare as-is (no case folding needed). Returns the
/// matched characters in `a`'s original case so a label can be shown
/// verbatim without re-querying.
fn longest_common_prefix(a: &str, b: &str) -> String {
    a.chars()
        .zip(b.chars())
        .take_while(|(ca, cb)| ca.eq_ignore_ascii_case(cb))
        .map(|(ca, _)| ca)
        .collect()
}

/// Walk a sorted file list and emit groups whose adjacent members share
/// a long-enough prefix. Algorithm:
///   - Window starts at i, prefix = files[i].display_name.
///   - Extend j while LCP(prefix, files[j].display_name) ≥ `min_prefix_chars`
///     AND ≥ `prefix_ratio` × min(len(prefix), len(files[j].display_name)).
///   - The window's prefix shrinks (LCP-style) as more files join.
///   - Groups of size ≥ 2 are yielded; singletons are skipped.
///
/// Used by `file_duplicate_groups`. Pulled out so it can be unit-tested
/// without spinning up an in-memory DB.
fn compute_group_ranges(
    files: &[FileEntry],
    min_prefix_chars: usize,
    prefix_ratio: f32,
) -> Vec<(String, std::ops::Range<usize>)> {
    let mut groups: Vec<(String, std::ops::Range<usize>)> = Vec::new();
    let mut i = 0;
    while i < files.len() {
        let mut j = i + 1;
        let mut prefix = files[i].display_name.clone();
        let mut prefix_chars = prefix.chars().count();
        while j < files.len() {
            let next = &files[j].display_name;
            let common = longest_common_prefix(&prefix, next);
            let common_chars = common.chars().count();
            let next_chars = next.chars().count();
            let shorter = prefix_chars.min(next_chars);
            if common_chars < min_prefix_chars
                || (common_chars as f32) < prefix_ratio * shorter as f32
            {
                break;
            }
            prefix = common;
            prefix_chars = common_chars;
            j += 1;
        }
        if j - i >= 2 {
            groups.push((prefix, i..j));
        }
        i = j.max(i + 1);
    }
    groups
}

/// Find groups of files whose `display_name`s share a long prefix. The
/// frontend's `/cleanup` page calls this once on mount and renders each
/// group as a collapsible card with per-row delete. Defaults match the
/// UI's hidden-by-default sensitivity controls. `category_id`, when
/// provided, scopes the scan to one category — useful when the user
/// wants to dedupe comics without novel-title noise (or vice versa).
#[tauri::command]
pub async fn file_duplicate_groups(
    app: AppHandle,
    min_prefix_chars: Option<usize>,
    prefix_ratio: Option<f32>,
    category_id: Option<i64>,
) -> Result<Vec<DuplicateGroup>, String> {
    let min_prefix_chars = min_prefix_chars.unwrap_or(DEFAULT_MIN_PREFIX_CHARS);
    let prefix_ratio = prefix_ratio.unwrap_or(DEFAULT_PREFIX_RATIO);
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let mut files: Vec<FileEntry> = if let Some(cid) = category_id {
        sqlx::query_as(
            "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, created_at, updated_at FROM files WHERE category_id = ?"
        )
        .bind(cid)
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query_as(
            "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, created_at, updated_at FROM files"
        )
        .fetch_all(&pool)
        .await
    }
    .map_err(|e| e.to_string())?;

    files.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });

    let ranges = compute_group_ranges(&files, min_prefix_chars, prefix_ratio);
    if ranges.is_empty() {
        return Ok(Vec::new());
    }

    // Drain files into per-group vecs without cloning. The iterator
    // walks once; we skip files outside any group and take files inside.
    let mut iter = files.into_iter();
    let mut consumed = 0usize;
    let mut result: Vec<DuplicateGroup> = Vec::with_capacity(ranges.len());
    for (prefix, range) in ranges {
        while consumed < range.start {
            iter.next();
            consumed += 1;
        }
        let mut group_files: Vec<FileEntry> = Vec::with_capacity(range.end - range.start);
        while consumed < range.end {
            // Safe: ranges are within files.len() by construction.
            group_files.push(iter.next().expect("range within bounds"));
            consumed += 1;
        }
        let items = hydrate_file_items(&pool, group_files).await?;
        result.push(DuplicateGroup {
            prefix,
            files: items,
        });
    }

    Ok(result)
}

// ── Re-analyze novels with no tags ────────────────────────────────────────────

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
pub async fn file_reanalyze_missing_tags(
    app: AppHandle,
) -> Result<ReanalyzeResponse, String> {
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
    let categories: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM categories")
            .fetch_all(&pool)
            .await
            .map_err(|e| e.to_string())?;
    let category_names: Vec<String> = categories.iter().map(|(_, n)| n.clone()).collect();
    let mut existing_tags: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM tags")
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
                kind, &c.path, &roots.storage_path, &roots.app_root,
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
                let Some(text) =
                    crate::pipeline::nodes::decode_to_utf8(&bytes)
                else {
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
                let Some(sample) =
                    crate::pipeline::nodes::sample_from_text(&text, 5, 1000)
                else {
                    failed += 1;
                    errors.push(ReanalyzeError {
                        file_id: c.id,
                        display_name: c.display_name.clone(),
                        message: "no content after decoding (zero chars)"
                            .to_string(),
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
            if let Some((new_cat_id, _)) =
                categories.iter().find(|(_, n)| n == new_cat_name)
            {
                if Some(*new_cat_id) != c.category_id {
                    if let Err(e) = file_move_category(
                        app.clone(),
                        c.id,
                        Some(*new_cat_id),
                    )
                    .await
                    {
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
            if let Some((tid, _)) =
                existing_tags.iter().find(|(_, n)| n == trimmed)
            {
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
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO file_tags (file_id, tag_id) VALUES (?, ?)",
            )
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
                Some(cache) => {
                    crate::path_resolve::cache_to_absolute(cache, &roots.storage_path)
                }
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
        let node_error: Option<String> = ctx
            .outcomes
            .iter()
            .find_map(|o| match &o.status {
                crate::pipeline::NodeStatus::Err(msg) => {
                    Some(format!("{}: {}", o.name, msg))
                }
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

        if let Err(e) = sqlx::query(
            "INSERT OR REPLACE INTO covers (file_id, data, mime_type) VALUES (?, ?, ?)",
        )
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

#[tauri::command]
pub async fn file_get(
    app: AppHandle,
    id: i64,
) -> Result<FileWithDetails, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let file: FileEntry = sqlx::query_as(
        "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, created_at, updated_at FROM files WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("File not found")?;

    let category: Option<Category> = if let Some(cat_id) = file.category_id {
        sqlx::query_as("SELECT id, name, description, icon, is_default, folder_name, schema_slug, view_config, created_at FROM categories WHERE id = ?")
            .bind(cat_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?
    } else {
        None
    };

    let tags: Vec<Tag> = sqlx::query_as(
        "SELECT t.id, t.name, t.color, t.created_at FROM tags t
         INNER JOIN file_tags ft ON t.id = ft.tag_id WHERE ft.file_id = ?"
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let authors: Vec<Author> = sqlx::query_as(
        "SELECT a.id, a.name, a.created_at FROM authors a
         INNER JOIN file_authors fa ON a.id = fa.author_id WHERE fa.file_id = ?"
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let metadata: Vec<Metadata> = sqlx::query_as(
        "SELECT id, file_id, key, value, data_type FROM metadata WHERE file_id = ?"
    )
    .bind(id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    // Resolve stored relative paths → absolute at the IPC boundary.
    let roots = crate::commands::settings::load_path_roots(&pool).await?;
    let storage_kind_ref = file.storage_kind.as_deref().unwrap_or("local");
    let abs_path = crate::path_resolve::to_absolute(
        storage_kind_ref, &file.path, &roots.storage_path, &roots.app_root,
    )
    .to_string_lossy()
    .to_string();
    let abs_cache = file.local_cache_path.as_ref().map(|p| {
        crate::path_resolve::cache_to_absolute(p, &roots.storage_path)
            .to_string_lossy()
            .to_string()
    });

    Ok(FileWithDetails {
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
        created_at: file.created_at,
        updated_at: file.updated_at,
        category,
        tags,
        authors,
        metadata,
    })
}

#[tauri::command]
pub async fn file_create(
    app: AppHandle,
    path: String,
    display_name: String,
    category_id: Option<i64>,
    tag_ids: Option<Vec<i64>>,
    author_ids: Option<Vec<i64>>,
    metadata: Option<Vec<MetadataInput>>,
    progress: Option<String>,
    cover_data: Option<Vec<u8>>,
    cover_mime_type: Option<String>,
    staged_cover_path: Option<String>,
) -> Result<FileCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Inline `cover_data` (user uploaded one in the form) wins. When absent,
    // fall back to whichever cover the pipeline staged for this import path
    // — those bytes never crossed IPC, so this `take` is the only consumer.
    let (cover_data, cover_mime_type) = if cover_data.is_some() {
        (cover_data, cover_mime_type)
    } else if let Some(staged_path) = staged_cover_path.as_deref() {
        let cache = app.state::<crate::commands::processing::PreparedCoverCache>();
        match cache.take(staged_path) {
            Some((bytes, mime)) => (Some(bytes), Some(mime)),
            None => (None, cover_mime_type),
        }
    } else {
        (None, cover_mime_type)
    };

    let validated_name = validate_display_name(&display_name)?;

    // Check for existing file with same path
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM files WHERE path = ?")
        .bind(&path)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("FILE_ALREADY_EXISTS".to_string());
    }

    // Get storage_path from settings
    let storage_path_result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let storage_path = match storage_path_result {
        Some((p,)) if !p.is_empty() => PathBuf::from(&p),
        _ => return Err("STORAGE_PATH_NOT_CONFIGURED".to_string()),
    };

    // Verify storage path exists
    if !storage_path.exists() {
        return Err("STORAGE_PATH_NOT_FOUND".to_string());
    }

    // Check source file exists
    let source_path = PathBuf::from(&path);
    if !source_path.exists() {
        return Err("SOURCE_FILE_NOT_FOUND".to_string());
    }
    // Source kind drives the move/copy vs zip-on-commit branch below.
    let source_is_dir = source_path.is_dir();

    // Canonicalize paths for comparison
    let source_canonical = source_path.canonicalize()
        .map_err(|e| format!("Failed to resolve source path: {}", e))?;
    let storage_canonical = storage_path.canonicalize()
        .map_err(|e| format!("Failed to resolve storage path: {}", e))?;

    // Check source is not inside storage_path
    if source_canonical.starts_with(&storage_canonical) {
        return Err("SOURCE_ALREADY_IN_STORAGE".to_string());
    }

    // Determine destination folder. Imports must carry a category — the
    // legacy `_uncategorized` fallback was retired once the migration
    // helper moved every existing null row into the `novel` category.
    let cat_id = category_id.ok_or_else(|| "CATEGORY_REQUIRED".to_string())?;
    let cat_result: Option<(Option<String>, String, String)> = sqlx::query_as(
        "SELECT folder_name, name, schema_slug FROM categories WHERE id = ?",
    )
    .bind(cat_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let (folder_name, schema_slug) = match cat_result {
        Some((Some(folder), _, slug)) => (folder, crate::schema::SchemaSlug::from_str(&slug)),
        Some((None, name, slug)) => (
            sanitize_folder_name(&name),
            crate::schema::SchemaSlug::from_str(&slug),
        ),
        None => return Err("CATEGORY_NOT_FOUND".to_string()),
    };

    // Create destination folder if needed
    let dest_folder = storage_canonical.join(&folder_name);
    if !dest_folder.exists() {
        fs::create_dir_all(&dest_folder)
            .map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    // Get source filename and extension
    let source_filename = source_path.file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;
    let ext_lower = source_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());
    let should_clean_name = matches!(ext_lower.as_deref(), Some("txt") | Some("epub"));

    // Resolve author names up-front (needed for clean filename)
    let resolved_author_names: Vec<String> = match author_ids.as_ref() {
        Some(ids) if !ids.is_empty() => {
            let placeholders = (0..ids.len()).map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT name FROM authors WHERE id IN ({})",
                placeholders
            );
            let mut q = sqlx::query_scalar::<_, String>(&sql);
            for id in ids {
                q = q.bind(id);
            }
            q.fetch_all(&pool).await.map_err(|e| e.to_string())?
        }
        _ => Vec::new(),
    };

    // Compute destination filename
    let dest_filename = if source_is_dir {
        // Image-folder import: package the directory into a `.zip` named
        // after the (sanitized) display_name. Source folder has no
        // extension, so we append `.zip` unconditionally.
        let stem = sanitize_filename(&validated_name);
        let stem = if stem.trim().is_empty() {
            sanitize_filename(source_filename)
        } else {
            stem
        };
        format!("{}.zip", stem)
    } else if should_clean_name {
        let ext_with_dot = ext_lower
            .as_ref()
            .map(|e| format!(".{}", e))
            .unwrap_or_default();
        let clean = build_novel_filename(
            &validated_name,
            progress.as_deref(),
            &resolved_author_names,
            &ext_with_dot,
        );
        sanitize_filename(&clean)
    } else {
        source_filename.to_string()
    };

    let dest_path = get_unique_destination(&dest_folder.join(&dest_filename));

    // Check import mode setting
    let import_mode: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'import_mode'"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let use_copy = import_mode
        .map(|(v,)| v == "copy")
        .unwrap_or(false);

    // Stage the destination WITHOUT destroying the source yet. The row writes
    // below run in one transaction; if any fail we roll back and delete the
    // staged file, and because the source is still intact (the move-mode
    // source delete is deferred until after commit) that cleanup can never
    // lose data. Previously the source was moved/removed before the INSERT, so
    // a DB failure orphaned the bytes (move mode) or left a row with partial
    // children (a mid-loop child-insert failure).
    let final_path = if source_is_dir {
        // Both comic and galgame folder imports zip on commit. Comics keep
        // only image files (`zip_image_dir`); galgames archive the whole
        // directory tree verbatim (`zip_dir`) since a game is more than its
        // images. Other schemas don't accept directory inputs (validated at
        // import time), so this binary split is exhaustive in practice.
        if schema_slug == crate::schema::SchemaSlug::Galgame {
            zip_dir(&source_canonical, &dest_path)?
        } else {
            zip_image_dir(&source_canonical, &dest_path)?
        }
    } else {
        // Copy (not move) so the source survives a rolled-back transaction;
        // the original is removed below only after the commit succeeds.
        copy_file(&source_canonical, &dest_path)?
    };
    let final_path_str = final_path.to_string_lossy().to_string();
    // Store the destination path RELATIVE to storage_path so the user can
    // rename / move the storage folder without rewriting every row. The
    // resolver in `path_resolve::to_absolute` rebuilds the full path at
    // read time using whatever `storage_path` is currently set to.
    let storage_path_str = storage_path.to_string_lossy();
    let stored_path = crate::path_resolve::to_relative_local(&final_path_str, &storage_path_str);

    // Compress the cover (if any) BEFORE opening the transaction so a
    // compression failure aborts before any DB write and the transaction
    // stays short. Route through the shared compressor: user-uploaded
    // replacements arrive uncompressed, and even pipeline-staged bytes
    // (already JPEG'd by `CoverCompressNode`) round-trip cheaply through it,
    // avoiding the 1 MB+ blobs the helper exists to prevent.
    let _ = cover_mime_type;
    let compressed_cover = match cover_data {
        Some(data) => Some(
            crate::commands::cover::compress_cover_bytes(&data)
                .map_err(|e| {
                    // Clean up the staged destination file on compression failure
                    let _ = std::fs::remove_file(&final_path);
                    format!("Failed to compress cover: {e}")
                })?,
        ),
        None => None,
    };

    // All row writes (parent + tags + authors + metadata + cover) go through
    // one transaction so a mid-sequence failure rolls back to nothing rather
    // than leaving a row with partial children.
    let insert_result: Result<i64, String> = async move {
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

        let result = sqlx::query(
            "INSERT INTO files (path, display_name, category_id, in_storage, original_path, file_status, progress) VALUES (?, ?, ?, 1, ?, 'available', ?)"
        )
        .bind(&stored_path)
        .bind(&validated_name)
        .bind(category_id)
        .bind(&path)  // original_path is the source path
        .bind(&progress)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        let file_id = result.last_insert_rowid();

        if let Some(tags) = tag_ids {
            for tag_id in tags {
                sqlx::query("INSERT INTO file_tags (file_id, tag_id) VALUES (?, ?)")
                    .bind(file_id)
                    .bind(tag_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }

        if let Some(authors) = author_ids {
            for author_id in authors {
                sqlx::query("INSERT INTO file_authors (file_id, author_id) VALUES (?, ?)")
                    .bind(file_id)
                    .bind(author_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }

        if let Some(meta) = metadata {
            for m in meta {
                sqlx::query("INSERT INTO metadata (file_id, key, value, data_type) VALUES (?, ?, ?, ?)")
                    .bind(file_id)
                    .bind(&m.key)
                    .bind(&m.value)
                    .bind(m.data_type.unwrap_or_else(|| "text".to_string()))
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }

        if let Some(compressed) = &compressed_cover {
            sqlx::query(
                "INSERT OR REPLACE INTO covers (file_id, data, mime_type) VALUES (?, ?, ?)"
            )
            .bind(file_id)
            .bind(compressed)
            .bind("image/jpeg")
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to save cover: {e}"))?;
        }

        tx.commit().await.map_err(|e| e.to_string())?;
        Ok(file_id)
    }
    .await;

    let file_id = match insert_result {
        Ok(id) => id,
        Err(e) => {
            // DB rolled back — remove the staged destination so it doesn't
            // leak as an orphan. The source is untouched (move-mode delete is
            // deferred below), so retrying from the original is always safe.
            let _ = fs::remove_file(&final_path);
            return Err(e);
        }
    };

    // DB committed. In move mode, remove the original source now that the
    // import is durably recorded. Best-effort: a failed cleanup leaves a
    // harmless duplicate at the source, never data loss.
    if !use_copy {
        if source_is_dir {
            let _ = fs::remove_dir_all(&source_canonical);
        } else {
            let _ = fs::remove_file(&source_canonical);
        }
    }

    Ok(FileCreateResponse { id: file_id })
}

/// Rename a file on disk to match current metadata, updating DB atomically.
/// Only renames .txt files. No-op for other extensions.
/// Uses a transaction: DB update first, then fs rename; rollback on rename failure.
pub async fn rename_file_to_match_metadata(
    pool: &sqlx::SqlitePool,
    file_id: i64,
) -> Result<(), String> {
    let file_info: Option<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT path, display_name, progress, COALESCE(storage_kind, 'local') FROM files WHERE id = ?"
    )
    .bind(file_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((stored_path, display_name, progress, storage_kind)) = file_info else {
        return Ok(());
    };

    // The stored path is relative to storage_path (post-v11). Resolve to
    // an absolute path for the on-disk rename, then strip the prefix
    // again on the way back into the DB.
    let roots = crate::commands::settings::load_path_roots(pool).await?;
    let current_path = crate::path_resolve::to_absolute(
        &storage_kind,
        &stored_path,
        &roots.storage_path,
        &roots.app_root,
    );

    let ext_lower = current_path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    // Only rename text files
    if !matches!(ext_lower.as_deref(), Some("txt")) {
        return Ok(());
    }
    let ext_with_dot = ext_lower
        .as_ref()
        .map(|e| format!(".{}", e))
        .unwrap_or_default();

    let author_rows: Vec<(String,)> = sqlx::query_as(
        "SELECT a.name FROM authors a JOIN file_authors fa ON a.id = fa.author_id WHERE fa.file_id = ?"
    )
    .bind(file_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let author_names_vec: Vec<String> = author_rows.into_iter().map(|(n,)| n).collect();

    let clean = build_novel_filename(
        &display_name,
        progress.as_deref(),
        &author_names_vec,
        &ext_with_dot,
    );
    let sanitized = sanitize_filename(&clean);

    let Some(parent) = current_path.parent() else {
        return Ok(());
    };

    let new_path = get_unique_destination(&parent.join(&sanitized));
    if new_path == current_path {
        return Ok(());
    }
    let new_path_str = new_path.to_string_lossy().to_string();
    let new_stored = crate::path_resolve::to_relative_local(&new_path_str, &roots.storage_path);

    // Transaction: update DB first, then rename file
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("UPDATE files SET path = ? WHERE id = ?")
        .bind(&new_stored)
        .bind(file_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    match fs::rename(&current_path, &new_path) {
        Ok(_) => {
            tx.commit().await.map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            let _ = tx.rollback().await;
            Err(format!("Failed to rename file: {}", e))
        }
    }
}

#[derive(Serialize)]
pub struct FileCreateResponse {
    pub id: i64,
}

/// Physically move a file into the target category's folder and update
/// `files.path`. No category-equality or in_storage guards — the caller
/// decides whether the move should be attempted. Returns `Ok(true)` when
/// the file was actually moved, `Ok(false)` when it was already in the
/// destination folder.
async fn relocate_file_to_category_folder(
    pool: &sqlx::SqlitePool,
    file_id: i64,
    stored_path: &str,
    target_category_id: i64,
) -> Result<bool, String> {
    let storage_row: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let storage_path = match storage_row {
        Some((p,)) if !p.is_empty() => PathBuf::from(&p),
        _ => return Err("STORAGE_PATH_NOT_CONFIGURED".to_string()),
    };
    let storage_canonical = storage_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve storage path: {}", e))?;
    let storage_canonical_str = storage_canonical.to_string_lossy().to_string();

    let cat_row: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT folder_name, name FROM categories WHERE id = ?",
    )
    .bind(target_category_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;
    let folder_name = match cat_row {
        Some((Some(folder), _)) => folder,
        Some((None, name)) => sanitize_folder_name(&name),
        None => return Err("CATEGORY_NOT_FOUND".to_string()),
    };

    let dest_folder = storage_canonical.join(&folder_name);
    if !dest_folder.exists() {
        fs::create_dir_all(&dest_folder)
            .map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    // Caller passes the stored (relative) path. Resolve to an absolute
    // filesystem path for the on-disk operations; the relativizer at the
    // bottom strips the prefix again before UPDATE.
    let current_pb = crate::path_resolve::to_absolute(
        "local",
        stored_path,
        &storage_canonical_str,
        "",
    );
    if let Some(parent) = current_pb.parent() {
        if parent == dest_folder {
            return Ok(false);
        }
    }

    let filename = current_pb
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;

    let dest_path = dest_folder.join(filename);
    let final_path = move_file(&current_pb, &dest_path)?;
    let final_path_str = final_path.to_string_lossy().to_string();
    let new_stored = crate::path_resolve::to_relative_local(&final_path_str, &storage_canonical_str);

    sqlx::query("UPDATE files SET path = ? WHERE id = ?")
        .bind(&new_stored)
        .bind(file_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(true)
}

/// Move a file to the folder of a different category when category_id changes.
/// Updates files.path in the DB to match. No-op when the file isn't in storage,
/// the category hasn't actually changed, or the file is already in the target
/// folder. Caller is responsible for updating files.category_id.
async fn move_file_to_category_folder(
    pool: &sqlx::SqlitePool,
    file_id: i64,
    new_category_id: Option<i64>,
) -> Result<(), String> {
    let file_info: Option<(String, bool, Option<i64>)> = sqlx::query_as(
        "SELECT path, in_storage, category_id FROM files WHERE id = ?",
    )
    .bind(file_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((current_path, in_storage, current_category)) = file_info else {
        return Ok(());
    };

    if !in_storage || current_category == new_category_id {
        return Ok(());
    }

    let cat_id = new_category_id.ok_or_else(|| "CATEGORY_REQUIRED".to_string())?;
    relocate_file_to_category_folder(pool, file_id, &current_path, cat_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn file_update(
    app: AppHandle,
    id: i64,
    display_name: Option<String>,
    category_id: Option<i64>,
    progress: Option<String>,
) -> Result<FileUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // If the category changed, physically move the file first so that the
    // subsequent rename_file_to_match_metadata operates on the new location.
    if let Some(new_cat) = category_id {
        move_file_to_category_folder(&pool, id, Some(new_cat)).await?;
    }

    match (display_name, category_id, progress) {
        (Some(name), Some(cat_id), Some(prog)) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, category_id = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(cat_id)
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (Some(name), Some(cat_id), None) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(cat_id)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (Some(name), None, Some(prog)) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (Some(name), None, None) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, Some(cat_id), Some(prog)) => {
            sqlx::query(
                "UPDATE files SET category_id = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(cat_id)
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, Some(cat_id), None) => {
            sqlx::query(
                "UPDATE files SET category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(cat_id)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, None, Some(prog)) => {
            sqlx::query(
                "UPDATE files SET progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, None, None) => {}
    }

    // Rename file on disk to match updated metadata (atomic with DB)
    let _ = rename_file_to_match_metadata(&pool, id).await;

    Ok(FileUpdateResponse { success: true })
}

#[derive(Serialize)]
pub struct FileUpdateResponse {
    pub success: bool,
}

/// Recursively enumerate every non-hidden file under `path`. Used by the
/// import flow's "Choose folder…" option to expand a directory into the
/// flat list of paths that `file_prepare_import` expects.
///
/// Hidden files/dirs (dotfiles) are skipped. Symlinks are followed by the
/// default `std::fs::read_dir` + `is_dir`/`is_file` calls — acceptable for
/// the common case of a user picking a media folder.
///
/// True iff every non-hidden direct child of `dir` is an image file
/// AND `dir` contains no subdirectories. Hidden entries are ignored.
/// Empty directories return false (no content to import).
fn folder_is_image_leaf(dir: &std::path::Path) -> std::io::Result<bool> {
    use crate::pipeline::archive::{is_ignorable_metadata, is_image_filename};
    let mut saw_image = false;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // OS-metadata junk (dotfiles, Thumbs.db, desktop.ini) is transparent:
        // a folder of pages plus a stray Thumbs.db is still a comic leaf.
        if is_ignorable_metadata(&name_str) {
            continue;
        }
        let p = entry.path();
        if p.is_dir() {
            return Ok(false);
        }
        if p.is_file() {
            if !is_image_filename(&name_str) {
                return Ok(false);
            }
            saw_image = true;
        }
    }
    Ok(saw_image)
}

/// Recursively enumerate non-hidden files under `dir`. Image-leaf
/// subdirectories collapse to a single directory entry (the comic
/// pipeline zips them on commit). See `list_files_in_folder` for the
/// rationale behind the leaf-only collapse rule.
fn folder_walk(dir: &std::path::Path, out: &mut Vec<String>) -> std::io::Result<()> {
    use crate::pipeline::archive::is_ignorable_metadata;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // Skip OS-metadata junk so it isn't emitted as a bogus standalone
        // import path (matches the leaf detector's transparency rule).
        if is_ignorable_metadata(&name_str) {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            if folder_is_image_leaf(&path)? {
                if let Some(s) = path.to_str() {
                    out.push(s.to_string());
                }
            } else {
                folder_walk(&path, out)?;
            }
        } else if path.is_file() {
            if let Some(s) = path.to_str() {
                out.push(s.to_string());
            }
        }
    }
    Ok(())
}

/// Scan a single folder root using the import-aware walker. Result is
/// sorted so repeated scans produce stable ordering. If the root itself
/// is an image leaf, the root path is the only entry returned.
///
/// `schema` decides folder semantics. Galgame treats the picked folder as
/// ONE unit (a game is an opaque tree of executables/scripts/assets, not a
/// collapsible set of leaves) — it returns `[root]` and never walks or
/// image-leaf-tests, so `file_create` zips the whole directory on commit.
/// Comic/novel keep the image-leaf collapse + recursive walk.
fn scan_folder_root(
    root: &std::path::Path,
    schema: crate::schema::SchemaSlug,
) -> std::io::Result<Vec<String>> {
    if schema == crate::schema::SchemaSlug::Galgame {
        // The picked folder is the game. Don't descend — there's no
        // filesystem signal that would make walking it correct, and the
        // commit step archives the whole tree.
        return Ok(root.to_str().map(|s| vec![s.to_string()]).unwrap_or_default());
    }
    let mut files = Vec::new();
    if folder_is_image_leaf(root)? {
        if let Some(s) = root.to_str() {
            files.push(s.to_string());
        }
    } else {
        folder_walk(root, &mut files)?;
    }
    files.sort();
    Ok(files)
}

/// Image-folder leaf collapse: a directory whose non-hidden direct
/// children are all image files (and which has no subdirectories) is
/// emitted as a single path. `file_prepare_import` routes such dir
/// paths through the comic pipeline; `file_create` zips them on commit.
/// The walker descends through every other directory, so a
/// `library/[author]/[work]/*.jpg` tree resolves to one comic per
/// `[work]` folder. Multi-level structures like
/// `vol/chapter-1/*.jpg, vol/chapter-2/*.jpg` are split into per-chapter
/// comics — there is no filesystem-only signal that distinguishes
/// sibling chapters of one comic from sibling comics of one author, so
/// this leaf-only rule errs on the side of finer-grained imports.
/// Result is sorted so repeated folder picks produce stable ordering.
#[tauri::command]
pub async fn list_files_in_folder(
    path: String,
    schema_slug: Option<String>,
) -> Result<Vec<String>, String> {
    let root = std::path::Path::new(&path);
    if !root.exists() {
        return Err("PATH_NOT_FOUND".to_string());
    }
    if !root.is_dir() {
        return Err("NOT_A_DIRECTORY".to_string());
    }
    let schema = crate::schema::SchemaSlug::from_str(schema_slug.as_deref().unwrap_or("novel"));
    scan_folder_root(root, schema).map_err(|e| format!("Failed to scan folder: {e}"))
}

#[derive(Serialize)]
pub struct DropExpansion {
    /// Resolved file paths: standalone files passed through, plus the
    /// recursive contents of every dropped folder (with image-leaf
    /// collapse, matching `list_files_in_folder`).
    pub files: Vec<String>,
    /// Maps each enumerated path to the folder root the user dropped.
    /// Standalone-file drops are absent from this map — they take the
    /// same code path as `FilePicker.handlePickFiles`.
    pub path_folder_roots: std::collections::HashMap<String, String>,
    /// Folder roots that contained no importable entries. Reported so
    /// the UI can surface them, mirroring `FilePicker.handlePickFolder`.
    pub empty_folders: Vec<String>,
}

/// Resolve OS-level drop paths into the same shape `FilePicker` produces.
/// Handles a mixed batch where some paths are files and others are
/// folders — files pass through untouched; folders are walked with the
/// same image-leaf rules as the explicit folder picker. Missing paths
/// are skipped silently (a stale Finder drag can race a filesystem move
/// and a hard error would block the rest of the batch).
#[tauri::command]
pub async fn expand_drop_paths(
    paths: Vec<String>,
    schema_slug: Option<String>,
) -> Result<DropExpansion, String> {
    let mut files = Vec::new();
    let mut path_folder_roots = std::collections::HashMap::new();
    let mut empty_folders = Vec::new();
    let schema = crate::schema::SchemaSlug::from_str(schema_slug.as_deref().unwrap_or("novel"));

    for raw in paths {
        let p = std::path::Path::new(&raw);
        if !p.exists() {
            continue;
        }
        if p.is_file() {
            files.push(raw);
        } else if p.is_dir() {
            let scanned = scan_folder_root(p, schema)
                .map_err(|e| format!("Failed to scan folder {raw}: {e}"))?;
            if scanned.is_empty() {
                empty_folders.push(raw);
                continue;
            }
            for f in scanned {
                path_folder_roots.insert(f.clone(), raw.clone());
                files.push(f);
            }
        }
    }

    Ok(DropExpansion {
        files,
        path_folder_roots,
        empty_folders,
    })
}

/// Post-commit cleanup for folder imports. Called by the frontend after
/// every per-file `file_create` in the batch succeeds.
///
/// Behavior:
/// - No-op when `had_folder_imports` is false (pure-archive folder picks
///   keep their picked root untouched, matching pre-feature behavior).
/// - No-op when `import_mode` is `'copy'` (copy semantics keep originals).
/// - Refuses to touch anything inside `storage_path` (defense in depth).
/// - Walks `folder_root` bottom-up and removes empty subdirectories.
///   If after the walk the root itself is empty, removes it. If
///   non-empty (the user had stray non-image files), leaves it alone
///   and logs to stderr — the import already succeeded; cleanup is
///   best-effort and never fails the call.
#[tauri::command]
pub async fn import_finalize(
    app: AppHandle,
    folder_root: String,
    had_folder_imports: bool,
) -> Result<(), String> {
    if !had_folder_imports {
        return Ok(());
    }

    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let import_mode: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'import_mode'",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;
    if import_mode.map(|(v,)| v == "copy").unwrap_or(false) {
        return Ok(());
    }

    let root = PathBuf::from(&folder_root);
    if !root.exists() {
        // file_create already removed every leaf; nothing to do.
        return Ok(());
    }
    if !root.is_dir() {
        return Err("FOLDER_ROOT_NOT_A_DIRECTORY".to_string());
    }

    // Defense in depth: never recurse into anything under storage_path.
    let storage_path: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;
    if let Some((sp,)) = storage_path {
        if !sp.is_empty() {
            let storage_canonical = std::path::Path::new(&sp)
                .canonicalize()
                .map_err(|e| format!("Failed to resolve storage path: {e}"))?;
            let root_canonical = root
                .canonicalize()
                .map_err(|e| format!("Failed to resolve folder root: {e}"))?;
            if root_canonical.starts_with(&storage_canonical) {
                return Err("FOLDER_ROOT_INSIDE_STORAGE".to_string());
            }
        }
    }

    /// True iff `dir` recursively contains no real content — only
    /// OS-metadata junk. Dotfiles (`.DS_Store`, `.localized`, …) plus
    /// Windows Explorer junk (`Thumbs.db`, `desktop.ini`) are transparent:
    /// the OS seeds them everywhere it's been opened, and they would
    /// otherwise block cleanup of folders that are otherwise empty after
    /// `file_create` removed the leaf source dirs. Mirrors the
    /// `is_ignorable_metadata` convention used by `folder_is_image_leaf`
    /// and `folder_walk`.
    fn has_only_ignorable_content(dir: &std::path::Path) -> std::io::Result<bool> {
        use crate::pipeline::archive::is_ignorable_metadata;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if is_ignorable_metadata(&name_str) {
                continue;
            }
            let p = entry.path();
            if p.is_dir() {
                if !has_only_ignorable_content(&p)? {
                    return Ok(false);
                }
            } else {
                return Ok(false);
            }
        }
        Ok(true)
    }

    match has_only_ignorable_content(&root) {
        Ok(true) => {
            // `remove_dir_all` nukes the dir tree including the hidden
            // metadata we treated as transparent above.
            if let Err(e) = std::fs::remove_dir_all(&root) {
                eprintln!(
                    "import_finalize: remove_dir_all failed for {}: {e}",
                    root.display()
                );
            }
            Ok(())
        }
        Ok(false) => {
            eprintln!(
                "import_finalize: {} not removed (real files remain after leaf cleanup)",
                root.display()
            );
            Ok(())
        }
        Err(e) => {
            eprintln!("import_finalize: cleanup failed for {}: {e}", root.display());
            Ok(())
        }
    }
}

/// Move a file at an arbitrary path on disk to the system Trash — used
/// for the "Delete" choice on the import duplicate dialog, where the
/// file is NOT yet in the DB (so `file_delete`, which keys off id,
/// doesn't apply).
///
/// Uses the `trash` crate (which delegates to macOS NSWorkspace's
/// recycle / Linux XDG trash / Windows IFileOperation) instead of
/// `std::fs::remove_file`. Two reasons:
///   1. **TCC**: on macOS, user-picked source files often sit in
///      `~/Downloads` / `~/Desktop` / `~/Documents` — directories where
///      the file picker grants read scope only. `fs::remove_file` needs
///      directory write permission and fails with `PermissionDenied`.
///      The Trash API is treated as a user-intent operation by the OS
///      and works without that directory write entitlement.
///   2. **Reversibility**: the user can recover the file from Trash if
///      they change their mind. Better than a permanent delete on an
///      import-dialog click.
#[tauri::command]
pub async fn file_delete_source(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        // Nothing to remove — treat as success so the UI flow doesn't error.
        return Ok(());
    }
    trash::delete(p).map_err(|e| {
        let err_str = e.to_string().to_lowercase();
        if err_str.contains("permission") {
            "PERMISSION_DENIED".to_string()
        } else if err_str.contains("being used") || err_str.contains("locked") {
            "FILE_LOCKED".to_string()
        } else {
            format!("Failed to move source file to Trash: {e}")
        }
    })
}

#[tauri::command]
pub async fn file_delete(
    app: AppHandle,
    id: i64,
) -> Result<FileDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Get file info before deleting. `storage_kind` tells us whether the
    // row's `path` is a local filesystem path or a Baidu Pan path.
    let file_info: Option<(String, bool, String)> = sqlx::query_as(
        "SELECT path, in_storage, storage_kind FROM files WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((stored_path, in_storage, storage_kind)) = file_info else {
        // Already gone — treat as success so a double-click doesn't error.
        return Ok(FileDeleteResponse { success: true });
    };

    // Resolve stored relative path → absolute for the external op.
    // Baidu's API and fs::remove both want the full path.
    let roots = super::settings::load_path_roots(&pool).await?;
    let path = crate::path_resolve::to_absolute(
        &storage_kind, &stored_path, &roots.storage_path, &roots.app_root,
    )
    .to_string_lossy()
    .to_string();

    // Strict ordering: external resource first, DB row second. A failure
    // here aborts before the row is touched so the user can retry against
    // the same id. Matches the bulk-delete worker's policy and prevents
    // orphans on Baidu Pan / in the storage folder.
    if storage_kind == "remote" {
        super::remote::delete_on_remote_for_file(&pool, id, &path).await?;
    } else if in_storage {
        if let Err(e) = fs::remove_file(&path) {
            // "Already missing on disk" satisfies the invariant — the
            // external state is what we wanted. Anything else (permission
            // denied, file locked, IO error) propagates.
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(format!("Failed to remove local file: {e}"));
            }
        }
    }

    sqlx::query("DELETE FROM files WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(FileDeleteResponse { success: true })
}

#[derive(Serialize)]
pub struct FileDeleteResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn file_move_category(
    app: AppHandle,
    id: i64,
    category_id: Option<i64>,
) -> Result<FileMoveCategoryResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Get current file info
    let file_info: Option<(String, bool, Option<i64>)> = sqlx::query_as(
        "SELECT path, in_storage, category_id FROM files WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let (current_path, in_storage, _current_category) = file_info
        .ok_or("FILE_NOT_FOUND".to_string())?;

    // Verify file is in storage
    if !in_storage {
        return Err("FILE_NOT_IN_STORAGE".to_string());
    }

    // Get storage_path
    let storage_path_result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let storage_path = match storage_path_result {
        Some((p,)) if !p.is_empty() => PathBuf::from(&p),
        _ => return Err("STORAGE_PATH_NOT_CONFIGURED".to_string()),
    };

    // Verify storage path exists
    if !storage_path.exists() {
        return Err("STORAGE_PATH_NOT_FOUND".to_string());
    }

    // Canonicalize storage path
    let storage_canonical = storage_path.canonicalize()
        .map_err(|e| format!("Failed to resolve storage path: {}", e))?;

    // Determine new folder. Files must carry a category — the legacy
    // `_uncategorized` fallback was retired with the Debug-section
    // migration helper.
    let cat_id = category_id.ok_or_else(|| "CATEGORY_REQUIRED".to_string())?;
    let cat_result: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT folder_name, name FROM categories WHERE id = ?",
    )
    .bind(cat_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;
    let folder_name = match cat_result {
        Some((Some(folder), _)) => folder,
        Some((None, name)) => sanitize_folder_name(&name),
        None => return Err("CATEGORY_NOT_FOUND".to_string()),
    };

    // Create destination folder if needed
    let dest_folder = storage_canonical.join(&folder_name);
    if !dest_folder.exists() {
        fs::create_dir_all(&dest_folder)
            .map_err(|e| format!("Failed to create folder: {}", e))?;
    }

    // Resolve the stored relative path to an absolute filesystem path
    // for the move operation; we strip the prefix again before UPDATE.
    let storage_canonical_str = storage_canonical.to_string_lossy().to_string();
    let current_path_buf = crate::path_resolve::to_absolute(
        "local",
        &current_path,
        &storage_canonical_str,
        "",
    );
    let filename = current_path_buf.file_name()
        .and_then(|n| n.to_str())
        .ok_or("Invalid filename")?;

    // Check if already in the correct folder
    if let Some(parent) = current_path_buf.parent()
        && parent == dest_folder
    {
        // Already in correct folder, just update database
        sqlx::query("UPDATE files SET category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(category_id)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;

        return Ok(FileMoveCategoryResponse {
            success: true,
            new_path: current_path_buf.to_string_lossy().to_string(),
        });
    }

    // Move the file
    let dest_path = dest_folder.join(filename);
    let final_path = move_file(&current_path_buf, &dest_path)?;
    let final_path_str = final_path.to_string_lossy().to_string();
    let new_stored = crate::path_resolve::to_relative_local(&final_path_str, &storage_canonical_str);

    // Update database
    sqlx::query("UPDATE files SET path = ?, category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(&new_stored)
        .bind(category_id)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    // Return the absolute path — TS callers expect to use it directly
    // for follow-up disk ops (open, etc.). Internal storage stays
    // relative; this is just the IPC-boundary shape.
    Ok(FileMoveCategoryResponse { success: true, new_path: final_path_str })
}

#[derive(Serialize)]
pub struct FileMoveCategoryResponse {
    pub success: bool,
    pub new_path: String,
}

/// Minimum query length, in Unicode characters, for an FTS5 trigram lookup.
/// Trigram only indexes 3-character windows, so anything shorter has no
/// rows in the index and must use a `LIKE '%q%'` fallback instead.
const TRIGRAM_MIN_CHARS: usize = 3;

/// What kind of SQL filter to apply for a typed search query. Returned by
/// [`prepare_search_filter`] so `file_search` can pick the matching SQL.
enum SearchFilter {
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
fn prepare_search_filter(raw: &str) -> Option<SearchFilter> {
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
    let (filter_sql, filter_binds) = build_filter_sql(
        conditions.as_deref().unwrap_or(&[]),
        "f",
    );

    // If the user's query sanitizes to nothing, return an empty result set
    // with total = 0. The frontend routes empty queries to `file_list`, so
    // this path really only covers the all-punctuation / all-whitespace
    // edge case.
    let Some(filter) = prepare_search_filter(&query) else {
        return Ok(FileListResponse { files: Vec::new(), total: 0 });
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
                "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.local_cache_path, f.created_at, f.updated_at \
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
                "SELECT f.id, f.path, f.display_name, f.category_id, f.file_status, f.in_storage, f.original_path, f.progress, f.storage_kind, f.remote_provider, f.local_cache_path, f.created_at, f.updated_at \
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
    let total: (i64,) = count_stmt.fetch_one(&pool).await.map_err(|e| e.to_string())?;

    let items = hydrate_file_items(&pool, files).await?;

    Ok(FileListResponse {
        files: items,
        total: total.0,
    })
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
                "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, created_at, updated_at FROM files WHERE id IN ({})",
                placeholders
            );
            sqlx::query_as(&query)
                .fetch_all(&pool)
                .await
                .map_err(|e| e.to_string())?
        }
        None => {
            sqlx::query_as("SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, created_at, updated_at FROM files")
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
            kind, &file.path, &roots.storage_path, &roots.app_root,
        );
        let exists = if kind == "remote" {
            file.local_cache_path
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(|p| {
                    crate::path_resolve::cache_to_absolute(p, &roots.storage_path).exists()
                })
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

    let existing: Option<(String, bool, String)> = sqlx::query_as(
        "SELECT path, in_storage, storage_kind FROM files WHERE id = ?",
    )
    .bind(existing_file_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    if let Some((existing_stored, in_storage, storage_kind)) = existing {
        // Resolve relative → absolute before the external op. Same shape
        // as file_delete; see comments there.
        let roots = super::settings::load_path_roots(&pool).await?;
        let existing_path = crate::path_resolve::to_absolute(
            &storage_kind, &existing_stored, &roots.storage_path, &roots.app_root,
        )
        .to_string_lossy()
        .to_string();

        // Strict ordering, same as `file_delete`: prove the prior copy is
        // gone before we drop the DB row. Without this, a remote duplicate
        // resolved via "Replace" used to leave the Baidu file orphaned
        // (in_storage=false skipped the local branch, the DB row went
        // away, the cloud copy stayed).
        if storage_kind == "remote" {
            super::remote::delete_on_remote_for_file(&pool, existing_file_id, &existing_path).await?;
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
        cover_data,
        cover_mime_type,
        staged_cover_path,
    )
    .await
}

/// One grouping of files that share an author or a series-name prefix.
/// Frontend renders these as stacked cards; clicking one drills down to
/// a flat FileList filtered to `file_ids`. `cover_file_id` is whichever
/// member sorts first by display_name — gives the card a stable preview.
/// `schema_slug` tells the frontend which renderer to use for the cover
/// preview (comic → stored cover bytes; novel → procedural NovelCover).
#[derive(Debug, Serialize)]
pub struct Collection {
    pub mode: String,
    pub key: String,
    pub title: String,
    pub file_ids: Vec<i64>,
    pub cover_file_id: Option<i64>,
    pub schema_slug: String,
}

/// Strip a trailing volume / chapter / issue marker from a display name so
/// that "Berserk vol.1", "Berserk vol.2", "Berserk vol.3" all collapse to
/// "Berserk". The rule: find the first ASCII digit in the name, then trim
/// surrounding volume-marker characters (whitespace, `-`, `.`, `v`/`V`,
/// CJK 第/卷/话/章/集) off the right edge of the prefix. Names without a
/// digit pass through unchanged so non-serialized titles stay singletons.
/// Returns the original name if the derived key is shorter than 2 chars —
/// otherwise spurious matches like "X1"/"X2" would collide.
fn series_key(name: &str) -> String {
    let trimmed = name.trim();
    let end = trimmed
        .char_indices()
        .find(|(_, c)| c.is_ascii_digit())
        .map(|(i, _)| i)
        .unwrap_or(trimmed.len());
    if end == trimmed.len() {
        return trimmed.to_string();
    }
    let head = trimmed[..end].trim_end_matches(|c: char| {
        c.is_whitespace()
            || matches!(c, '-' | '_' | '.' | 'v' | 'V' | '#' | '(' | '[')
            || matches!(c, '第' | '卷' | '话' | '章' | '集')
    });
    if head.chars().count() < 2 {
        return trimmed.to_string();
    }
    head.to_string()
}

/// Build collections grouped by `mode` for files of the given schema:
/// - `"author"`: one collection per author that has ≥ 2 matching files.
/// - `"name_prefix"`: one collection per `series_key`-derived prefix
///   shared by ≥ 2 matching files.
///
/// `schema_slug` filters to one schema family ('novel' covers every
/// novel-schema category; 'comic' covers every comic-schema category —
/// the schema-slug column does the routing). Singletons are filtered out so the UI
/// doesn't show a wall of one-item "collections". When `category_id` is
/// `Some`, only that category's files participate; when `None`, every
/// category with the matching schema slug is included. Results are
/// sorted by member count descending, then title ascending, so the
/// densest series surface first.
#[tauri::command]
pub async fn collection_list(
    app: AppHandle,
    mode: String,
    schema_slug: String,
    category_id: Option<i64>,
) -> Result<Vec<Collection>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let mut query = String::from(
        "SELECT f.id, f.display_name
         FROM files f
         INNER JOIN categories c ON c.id = f.category_id
         WHERE c.schema_slug = ?",
    );
    if category_id.is_some() {
        query.push_str(" AND f.category_id = ?");
    }
    let mut q = sqlx::query_as::<_, (i64, String)>(&query);
    q = q.bind(&schema_slug);
    if let Some(id) = category_id {
        q = q.bind(id);
    }
    let files: Vec<(i64, String)> = q
        .fetch_all(&pool)
        .await
        .map_err(|e| format!("Failed to load files for collections: {e}"))?;

    match mode.as_str() {
        "author" => build_author_collections(&pool, &files, &schema_slug, category_id).await,
        "name_prefix" => Ok(build_name_prefix_collections(&files, &schema_slug)),
        other => Err(format!("Unknown collection mode: {other}")),
    }
}

async fn build_author_collections(
    pool: &sqlx::SqlitePool,
    files: &[(i64, String)],
    schema_slug: &str,
    category_id: Option<i64>,
) -> Result<Vec<Collection>, String> {
    if files.is_empty() {
        return Ok(Vec::new());
    }

    // Fetch (file_id, author_id, author_name) rows for every in-scope
    // file. A single query is cheaper than N per-file lookups and keeps
    // the function O(rows) instead of O(files × authors).
    let mut query = String::from(
        "SELECT fa.file_id, a.id, a.name
         FROM file_authors fa
         INNER JOIN authors a ON a.id = fa.author_id
         INNER JOIN files f ON f.id = fa.file_id
         INNER JOIN categories c ON c.id = f.category_id
         WHERE c.schema_slug = ?",
    );
    if category_id.is_some() {
        query.push_str(" AND f.category_id = ?");
    }
    let mut q = sqlx::query_as::<_, (i64, i64, String)>(&query);
    q = q.bind(schema_slug);
    if let Some(id) = category_id {
        q = q.bind(id);
    }
    let rows: Vec<(i64, i64, String)> = q
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load author rows: {e}"))?;

    let name_by_id: std::collections::HashMap<i64, String> = files
        .iter()
        .map(|(id, name)| (*id, name.clone()))
        .collect();

    let mut groups: std::collections::HashMap<i64, (String, Vec<(i64, String)>)> =
        std::collections::HashMap::new();
    for (file_id, author_id, author_name) in rows {
        let Some(display_name) = name_by_id.get(&file_id) else {
            continue;
        };
        groups
            .entry(author_id)
            .or_insert_with(|| (author_name, Vec::new()))
            .1
            .push((file_id, display_name.clone()));
    }

    Ok(finalize_collections(
        "author",
        schema_slug,
        groups.into_iter().map(|(k, v)| (k.to_string(), v)),
    ))
}

fn build_name_prefix_collections(files: &[(i64, String)], schema_slug: &str) -> Vec<Collection> {
    let mut groups: std::collections::HashMap<String, (String, Vec<(i64, String)>)> =
        std::collections::HashMap::new();
    for (id, name) in files {
        let key = series_key(name);
        groups
            .entry(key.clone())
            .or_insert_with(|| (key, Vec::new()))
            .1
            .push((*id, name.clone()));
    }
    finalize_collections("name_prefix", schema_slug, groups.into_iter())
}

fn finalize_collections<I>(
    mode: &'static str,
    schema_slug: &str,
    groups: I,
) -> Vec<Collection>
where
    I: Iterator<Item = (String, (String, Vec<(i64, String)>))>,
{
    let mut out: Vec<Collection> = groups
        .filter_map(|(key, (title, mut members))| {
            if members.len() < 2 {
                return None;
            }
            // Stable preview cover: pick the alphabetically-first member.
            members.sort_by(|a, b| a.1.cmp(&b.1));
            let cover_file_id = members.first().map(|(id, _)| *id);
            let file_ids: Vec<i64> = members.iter().map(|(id, _)| *id).collect();
            Some(Collection {
                mode: mode.to_string(),
                key,
                title,
                file_ids,
                cover_file_id,
                schema_slug: schema_slug.to_string(),
            })
        })
        .collect();
    out.sort_by(|a, b| b.file_ids.len().cmp(&a.file_ids.len()).then(a.title.cmp(&b.title)));
    out
}

#[cfg(test)]
mod filename_tests {
    use super::*;

    #[test]
    fn test_build_novel_filename_full() {
        let result = build_novel_filename(
            "三体",
            Some("完结"),
            &["刘慈欣".to_string()],
            ".txt",
        );
        assert_eq!(result, "三体 完结 刘慈欣.txt");
    }

    #[test]
    fn test_build_novel_filename_no_progress() {
        let result = build_novel_filename(
            "三体",
            None,
            &["刘慈欣".to_string()],
            ".txt",
        );
        assert_eq!(result, "三体 刘慈欣.txt");
    }

    #[test]
    fn test_build_novel_filename_no_authors() {
        let result = build_novel_filename(
            "三体",
            Some("完结"),
            &[],
            ".txt",
        );
        assert_eq!(result, "三体 完结.txt");
    }

    #[test]
    fn test_build_novel_filename_multiple_authors() {
        let result = build_novel_filename(
            "三体",
            None,
            &["A".to_string(), "B".to_string()],
            ".txt",
        );
        assert_eq!(result, "三体 A, B.txt");
    }

    #[test]
    fn test_build_novel_filename_empty_progress() {
        let result = build_novel_filename(
            "三体",
            Some(""),
            &["刘慈欣".to_string()],
            ".txt",
        );
        assert_eq!(result, "三体 刘慈欣.txt");
    }

    #[test]
    fn test_sanitize_filename_invalid_chars() {
        assert_eq!(
            sanitize_filename("a/b\\c:d*e?f\"g<h>i|j.txt"),
            "abcdefghij.txt"
        );
    }

    #[test]
    fn test_sanitize_filename_preserves_valid() {
        assert_eq!(
            sanitize_filename("三体 完结 刘慈欣.txt"),
            "三体 完结 刘慈欣.txt"
        );
    }

    fn fts_expr(raw: &str) -> Option<String> {
        match prepare_search_filter(raw)? {
            SearchFilter::Fts(s) => Some(s),
            SearchFilter::Like(_) => None,
        }
    }

    fn like_pattern(raw: &str) -> Option<String> {
        match prepare_search_filter(raw)? {
            SearchFilter::Like(s) => Some(s),
            SearchFilter::Fts(_) => None,
        }
    }

    #[test]
    fn search_filter_single_token_quoted_for_fts() {
        // Trigram tokenizer matches substrings inside indexed text, so the
        // quoted whole-token form is enough — no `*` prefix marker needed.
        assert_eq!(fts_expr("三体老师").as_deref(), Some("\"三体老师\""));
    }

    #[test]
    fn search_filter_multiple_tokens_anded_with_quotes() {
        assert_eq!(
            fts_expr("三体老师 刘慈欣").as_deref(),
            Some("\"三体老师\" \"刘慈欣\"")
        );
    }

    #[test]
    fn search_filter_strips_fts5_operators() {
        assert_eq!(
            fts_expr("hello(world):today").as_deref(),
            Some("\"hello\" \"world\" \"today\"")
        );
    }

    #[test]
    fn search_filter_short_query_falls_back_to_like() {
        // Below the trigram window — must use LIKE with the raw pattern.
        assert_eq!(like_pattern("体").as_deref(), Some("%体%"));
        assert_eq!(like_pattern("三体").as_deref(), Some("%三体%"));
    }

    #[test]
    fn search_filter_short_query_escapes_like_wildcards() {
        // SQL wildcards in user input must be escaped so a literal `%` or
        // `_` can't expand the match. We escape with `\` and bind `ESCAPE '\\'`.
        assert_eq!(like_pattern("a%").as_deref(), Some("%a\\%%"));
        assert_eq!(like_pattern("a_").as_deref(), Some("%a\\_%"));
        assert_eq!(like_pattern("\\a").as_deref(), Some("%\\\\a%"));
    }

    #[test]
    fn search_filter_empty_or_punctuation_returns_none() {
        assert!(prepare_search_filter("").is_none());
        assert!(prepare_search_filter("   ").is_none());
        assert!(prepare_search_filter("\"\"()").is_none());
    }
}

#[cfg(test)]
mod reverse_index_tests {
    use crate::commands::test_helpers::setup_db;

    // Note: tests for `list_files_by_tag_impl` and `list_files_by_author_impl`
    // were removed when those helpers were deleted during the tag/author
    // route revamp (the routes now query through the general `file_list`
    // path with seeded conditions). The smoke test stays so the module
    // doesn't become empty.

    #[tokio::test]
    async fn setup_db_smoke_test() {
        let pool = setup_db().await;
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

}