use super::*;
/// One grouping of files that share an author or a series-name prefix.
/// Frontend renders these as stacked cards; clicking one drills down to
/// a flat FileList filtered to `file_ids`. `cover_file_id` is whichever
/// member sorts first by display_name — gives the card a stable preview.
/// `schema_slug` tells the frontend which renderer to use for the cover
/// preview (comic uses stored cover bytes; novel uses procedural NovelCover).
#[derive(Serialize)]
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

    let name_by_id: std::collections::HashMap<i64, String> =
        files.iter().map(|(id, name)| (*id, name.clone())).collect();

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

fn finalize_collections<I>(mode: &'static str, schema_slug: &str, groups: I) -> Vec<Collection>
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
    out.sort_by(|a, b| {
        b.file_ids
            .len()
            .cmp(&a.file_ids.len())
            .then(a.title.cmp(&b.title))
    });
    out
}
