use super::*;
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
            "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, is_favorite, created_at, updated_at FROM files WHERE category_id = ?"
        )
        .bind(cid)
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query_as(
            "SELECT id, path, display_name, category_id, file_status, in_storage, original_path, progress, storage_kind, remote_provider, local_cache_path, is_favorite, created_at, updated_at FROM files"
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
