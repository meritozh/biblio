use crate::commands::validation::validate_author_name;
use serde::Serialize;
use tauri::AppHandle;
use tauri::{Emitter, Manager};
use tauri_plugin_sql::{DbPool, DbInstances};

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

#[derive(serde::Serialize, Clone)]
struct AuthorChangeEvent {
    id: i64,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AuthorWithUsage {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    /// Wire-format alias: TS reads `usageCount`. See `TagWithUsage` for
    /// the same pattern; we avoid `rename_all = "camelCase"` so other
    /// snake_case fields (created_at) stay aligned with the base
    /// `Author` type.
    #[serde(rename = "usageCount")]
    pub usage_count: i64,
}

#[tauri::command]
pub async fn author_list(
    app: AppHandle,
    include_usage: Option<bool>,
    limit: Option<i64>,
    offset: Option<i64>,
    name_query: Option<String>,
) -> Result<AuthorListResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // `LIMIT -1 OFFSET 0` returns every row in SQLite — lets the unpaginated
    // callers (edit dialog's author picker etc.) reuse the same SQL.
    let limit_val: i64 = limit.unwrap_or(-1);
    let offset_val: i64 = offset.unwrap_or(0);
    let like_pattern = name_query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s.to_lowercase()));

    let authors: Vec<AuthorWithUsage> = match (include_usage.unwrap_or(false), like_pattern.as_deref()) {
        (true, Some(pat)) => sqlx::query_as(
            "SELECT a.id, a.name, a.created_at, COUNT(fa.file_id) as usage_count
             FROM authors a
             LEFT JOIN file_authors fa ON a.id = fa.author_id
             WHERE LOWER(a.name) LIKE ?
             GROUP BY a.id
             ORDER BY a.name
             LIMIT ? OFFSET ?",
        )
        .bind(pat)
        .bind(limit_val)
        .bind(offset_val)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
        (true, None) => sqlx::query_as(
            "SELECT a.id, a.name, a.created_at, COUNT(fa.file_id) as usage_count
             FROM authors a
             LEFT JOIN file_authors fa ON a.id = fa.author_id
             GROUP BY a.id
             ORDER BY a.name
             LIMIT ? OFFSET ?",
        )
        .bind(limit_val)
        .bind(offset_val)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
        (false, Some(pat)) => sqlx::query_as(
            "SELECT a.id, a.name, a.created_at, 0 as usage_count
             FROM authors a
             WHERE LOWER(a.name) LIKE ?
             ORDER BY a.name
             LIMIT ? OFFSET ?",
        )
        .bind(pat)
        .bind(limit_val)
        .bind(offset_val)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
        (false, None) => sqlx::query_as(
            "SELECT a.id, a.name, a.created_at, 0 as usage_count
             FROM authors a
             ORDER BY a.name
             LIMIT ? OFFSET ?",
        )
        .bind(limit_val)
        .bind(offset_val)
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?,
    };

    Ok(AuthorListResponse { authors })
}

#[derive(Serialize)]
pub struct AuthorListResponse {
    pub authors: Vec<AuthorWithUsage>,
}

#[tauri::command]
pub async fn author_count(app: AppHandle, name_query: Option<String>) -> Result<i64, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let like_pattern = name_query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", s.to_lowercase()));
    let (total,): (i64,) = if let Some(pat) = like_pattern {
        sqlx::query_as("SELECT COUNT(*) FROM authors WHERE LOWER(name) LIKE ?")
            .bind(pat)
            .fetch_one(&pool)
            .await
            .map_err(|e| e.to_string())?
    } else {
        sqlx::query_as("SELECT COUNT(*) FROM authors")
            .fetch_one(&pool)
            .await
            .map_err(|e| e.to_string())?
    };
    Ok(total)
}

#[tauri::command]
pub async fn author_create(
    app: AppHandle,
    name: String,
) -> Result<AuthorCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let validated_name = validate_author_name(&name)?;

    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM authors WHERE name = ?")
        .bind(&validated_name)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("AUTHOR_EXISTS".to_string());
    }

    let result = sqlx::query("INSERT INTO authors (name) VALUES (?)")
        .bind(&validated_name)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(AuthorCreateResponse {
        id: result.last_insert_rowid(),
    })
}

#[derive(Serialize)]
pub struct AuthorCreateResponse {
    pub id: i64,
}

#[tauri::command]
pub async fn author_update(
    app: AppHandle,
    id: i64,
    name: String,
) -> Result<AuthorUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let validated_name = validate_author_name(&name)?;

    // Check if another author with the same name exists
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM authors WHERE name = ? AND id != ?")
        .bind(&validated_name)
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("AUTHOR_EXISTS".to_string());
    }

    sqlx::query("UPDATE authors SET name = ? WHERE id = ?")
        .bind(&validated_name)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("author-updated", AuthorChangeEvent { id });

    Ok(AuthorUpdateResponse { success: true })
}

#[derive(Serialize)]
pub struct AuthorUpdateResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn author_delete(
    app: AppHandle,
    id: i64,
) -> Result<AuthorDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let affected: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM file_authors WHERE author_id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM file_authors WHERE author_id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM authors WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let _ = app.emit("author-deleted", AuthorChangeEvent { id });

    Ok(AuthorDeleteResponse {
        success: true,
        affected_files: affected.0,
    })
}

#[derive(Serialize)]
pub struct AuthorDeleteResponse {
    pub success: bool,
    pub affected_files: i64,
}

/// Bulk-delete authors with no `file_authors` row referencing them. Used
/// by the `/cleanup` page. Emits one `author-deleted` event with `id: 0`
/// as a bulk sentinel; see `tag_delete_unused` for the rationale.
#[tauri::command]
pub async fn author_delete_unused(
    app: AppHandle,
) -> Result<AuthorDeleteUnusedResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let result = sqlx::query(
        "DELETE FROM authors WHERE NOT EXISTS (SELECT 1 FROM file_authors WHERE author_id = authors.id)",
    )
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let deleted = result.rows_affected() as i64;
    if deleted > 0 {
        let _ = app.emit("author-deleted", AuthorChangeEvent { id: 0 });
    }

    Ok(AuthorDeleteUnusedResponse { deleted })
}

#[derive(Serialize)]
pub struct AuthorDeleteUnusedResponse {
    pub deleted: i64,
}

/// Merge `from_id` into `to_id`: every file linked to the source author
/// is relinked to the target (files already linked to both keep only
/// the target), then the source row is deleted. Used by `/cleanup`'s
/// author tools. Single transaction; emits the bulk `author-deleted`
/// sentinel so list views refresh everywhere.
#[tauri::command]
pub async fn author_merge(
    app: AppHandle,
    from_id: i64,
    to_id: i64,
) -> Result<AuthorMergeResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let moved = merge_authors(&pool, from_id, to_id).await?;
    let _ = app.emit("author-deleted", AuthorChangeEvent { id: 0 });
    Ok(AuthorMergeResponse { moved })
}

/// The merge itself, pool-scoped for testability.
async fn merge_authors(
    pool: &sqlx::SqlitePool,
    from_id: i64,
    to_id: i64,
) -> Result<i64, String> {
    if from_id == to_id {
        return Err("MERGE_SAME_AUTHOR".to_string());
    }
    // Both rows must exist — merging into a ghost target would silently
    // erase the source's identity along with its links.
    let found: Vec<(i64,)> = sqlx::query_as("SELECT id FROM authors WHERE id IN (?, ?)")
        .bind(from_id)
        .bind(to_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    if found.len() != 2 {
        return Err("AUTHOR_NOT_FOUND".to_string());
    }

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    // OR IGNORE: a file linked to both authors would collide on the
    // (file_id, author_id) PK; those links stay and get dropped by the
    // DELETE below instead.
    let moved = sqlx::query("UPDATE OR IGNORE file_authors SET author_id = ? WHERE author_id = ?")
        .bind(to_id)
        .bind(from_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected() as i64;
    sqlx::query("DELETE FROM file_authors WHERE author_id = ?")
        .bind(from_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM authors WHERE id = ?")
        .bind(from_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(moved)
}

#[derive(Serialize)]
pub struct AuthorMergeResponse {
    /// Files newly linked to the target author (files already linked
    /// to both don't count — they kept their existing target link).
    pub moved: i64,
}

#[tauri::command]
pub async fn author_assign(
    app: AppHandle,
    file_id: i64,
    author_ids: Vec<i64>,
) -> Result<AuthorAssignResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    for author_id in &author_ids {
        sqlx::query("INSERT OR IGNORE INTO file_authors (file_id, author_id) VALUES (?, ?)")
            .bind(file_id)
            .bind(author_id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    let _ = super::file::rename_file_to_match_metadata(&pool, file_id).await;

    Ok(AuthorAssignResponse { success: true })
}

#[derive(Serialize)]
pub struct AuthorAssignResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn author_unassign(
    app: AppHandle,
    file_id: i64,
    author_ids: Vec<i64>,
) -> Result<AuthorUnassignResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    for author_id in &author_ids {
        sqlx::query("DELETE FROM file_authors WHERE file_id = ? AND author_id = ?")
            .bind(file_id)
            .bind(author_id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    let _ = super::file::rename_file_to_match_metadata(&pool, file_id).await;

    Ok(AuthorUnassignResponse { success: true })
}

#[derive(Serialize)]
pub struct AuthorUnassignResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn author_set(
    app: AppHandle,
    file_id: i64,
    author_ids: Vec<i64>,
) -> Result<AuthorSetResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Replace the full set of authors atomically: a mid-loop failure must not
    // leave the file with a partially-applied author set.
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // First remove all existing author assignments for this file
    sqlx::query("DELETE FROM file_authors WHERE file_id = ?")
        .bind(file_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // Then add the new author assignments
    for author_id in &author_ids {
        sqlx::query("INSERT INTO file_authors (file_id, author_id) VALUES (?, ?)")
            .bind(file_id)
            .bind(author_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    let _ = super::file::rename_file_to_match_metadata(&pool, file_id).await;

    Ok(AuthorSetResponse { success: true })
}

#[derive(Serialize)]
pub struct AuthorSetResponse {
    pub success: bool,
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal authors + file_authors schema for merge tests.
    async fn seeded_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE authors (\
                id INTEGER PRIMARY KEY AUTOINCREMENT,\
                name TEXT NOT NULL UNIQUE\
            );\
            CREATE TABLE file_authors (\
                file_id INTEGER NOT NULL,\
                author_id INTEGER NOT NULL,\
                PRIMARY KEY (file_id, author_id)\
            );\
            INSERT INTO authors (id, name) VALUES (1, 'Alpha'), (2, 'Beta');\
            INSERT INTO file_authors (file_id, author_id) VALUES \
                (10, 1), (20, 1), (20, 2), (30, 2);",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Seed layout: file 10 has only the source author, file 20 has
        // both, file 30 has only the target.
        pool
    }

    #[tokio::test]
    async fn merge_relinks_files_and_deletes_source() {
        let pool = seeded_pool().await;
        let moved = merge_authors(&pool, 1, 2).await.unwrap();
        // file 10 newly links to target; file 20 already had it.
        assert_eq!(moved, 1);

        let links: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT file_id, author_id FROM file_authors ORDER BY file_id, author_id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(links, vec![(10, 2), (20, 2), (30, 2)]);

        let authors: Vec<(i64,)> = sqlx::query_as("SELECT id FROM authors ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(authors, vec![(2,)]);
    }

    #[tokio::test]
    async fn merge_rejects_same_author_and_missing_rows() {
        let pool = seeded_pool().await;
        assert_eq!(merge_authors(&pool, 1, 1).await.unwrap_err(), "MERGE_SAME_AUTHOR");
        assert_eq!(merge_authors(&pool, 1, 99).await.unwrap_err(), "AUTHOR_NOT_FOUND");
        assert_eq!(merge_authors(&pool, 99, 2).await.unwrap_err(), "AUTHOR_NOT_FOUND");
        // Nothing was written by the rejected merges.
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM file_authors")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 4);
    }
}
