use super::*;
pub(super) fn get_sqlite_pool(
    instances: &DbInstances,
    db_url: &str,
) -> Result<sqlx::SqlitePool, String> {
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
pub(super) fn order_by_clause(sort_by: Option<&str>, sort_desc: bool, alias: &str) -> String {
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
    pub value: Option<Value>,
}

fn condition_value_as_str(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str)
}

fn condition_value_as_bool(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(v) => Some(*v),
        Value::String(s) => match s.as_str() {
            "true" | "1" => Some(true),
            "false" | "0" => Some(false),
            _ => None,
        },
        Value::Number(n) => n.as_i64().and_then(|v| match v {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }),
        _ => None,
    }
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
pub(super) fn build_filter_sql(
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
                    if let Some(v) = condition_value_as_str(c.value.as_ref()) {
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
                    if let Some(v) = condition_value_as_str(c.value.as_ref()) {
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
            "favorite" => {
                if c.op == "is" {
                    if let Some(v) = condition_value_as_bool(c.value.as_ref()) {
                        sql.push_str(&format!(
                            " AND {p}is_favorite = {}",
                            if v { 1 } else { 0 },
                            p = prefix
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    (sql, binds)
}
