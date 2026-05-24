use crate::commands::*;
use crate::commands::validation::{sanitize_folder_name, validate_category_name};
use crate::schema::SchemaSlug;
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_sql::{DbPool, DbInstances};

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

#[tauri::command]
pub async fn category_list(app: AppHandle) -> Result<Vec<Category>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    sqlx::query_as("SELECT id, name, description, icon, is_default, folder_name, schema_slug, view_config, created_at FROM categories ORDER BY name")
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn category_get(app: AppHandle, id: i64) -> Result<Category, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    sqlx::query_as("SELECT id, name, description, icon, is_default, folder_name, schema_slug, view_config, created_at FROM categories WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("CATEGORY_NOT_FOUND".to_string())
}

/// Coerce blank-string and whitespace-only payloads to NULL so the DB
/// column stays clean, and reject malformed JSON early. The frontend
/// owns the shape; we just confirm it parses.
fn normalize_view_config(raw: Option<String>) -> Result<Option<String>, String> {
    match raw {
        Some(s) if !s.trim().is_empty() => {
            serde_json::from_str::<serde_json::Value>(&s)
                .map_err(|e| format!("INVALID_VIEW_CONFIG_JSON: {e}"))?;
            Ok(Some(s))
        }
        _ => Ok(None),
    }
}

async fn get_unique_folder_name(pool: &sqlx::SqlitePool, base: &str) -> Result<String, String> {
    let existing: Vec<(String,)> = sqlx::query_as(
        "SELECT folder_name FROM categories WHERE folder_name LIKE ?"
    )
    .bind(format!("{}%", base))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    if !existing.iter().any(|(f,)| f == base) {
        return Ok(base.to_string());
    }

    let mut counter = 1;
    loop {
        let candidate = format!("{}_{}", base, counter);
        if !existing.iter().any(|(f,)| f == &candidate) {
            return Ok(candidate);
        }
        counter += 1;
    }
}

#[tauri::command]
pub async fn category_update(
    app: AppHandle,
    id: i64,
    name: Option<String>,
    icon: Option<String>,
    description: Option<String>,
    schema_slug: Option<String>,
    view_config: Option<String>,
    clear_view_config: Option<bool>,
) -> Result<CategoryUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    if let Some(n) = name {
        let validated_name = validate_category_name(&n)?;

        // Check if name is actually changing
        let current: (String, Option<String>) = sqlx::query_as(
            "SELECT name, folder_name FROM categories WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

        if validated_name != current.0 {
            // Get storage path
            let storage_path: Option<(String,)> = sqlx::query_as(
                "SELECT value FROM app_settings WHERE key = 'storage_path'"
            )
            .fetch_optional(&pool)
            .await
            .map_err(|e| e.to_string())?;

            // Generate new folder name
            let base_folder = sanitize_folder_name(&validated_name);
            let new_folder = get_unique_folder_name(&pool, &base_folder).await?;

            // Rename folder if it exists
            if let Some((storage,)) = storage_path {
                if !storage.is_empty() {
                    if let Some(old_folder) = &current.1 {
                        let old_path = std::path::PathBuf::from(&storage).join(old_folder);
                        let new_path = std::path::PathBuf::from(&storage).join(&new_folder);

                        if old_path.exists() {
                            std::fs::rename(&old_path, &new_path)
                                .map_err(|e| format!("Failed to rename folder: {}", e))?;
                        }
                    }

                    // Update file paths
                    let files: Vec<(i64, String)> = sqlx::query_as(
                        "SELECT id, path FROM files WHERE category_id = ?"
                    )
                    .bind(id)
                    .fetch_all(&pool)
                    .await
                    .map_err(|e| e.to_string())?;

                    for (file_id, old_path) in files {
                        // `old_path` is now relative to storage_path
                        // (post-v11). Just take the basename and pin
                        // it under the new folder name — the result is
                        // also relative, no need to join with storage.
                        let path = std::path::PathBuf::from(&old_path);
                        if let Some(filename) = path.file_name() {
                            let new_rel_path = std::path::PathBuf::from(&new_folder)
                                .join(filename);
                            sqlx::query("UPDATE files SET path = ? WHERE id = ?")
                                .bind(new_rel_path.to_string_lossy().to_string())
                                .bind(file_id)
                                .execute(&pool)
                                .await
                                .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }

            // Update category
            sqlx::query("UPDATE categories SET name = ?, folder_name = ? WHERE id = ?")
                .bind(&validated_name)
                .bind(&new_folder)
                .bind(id)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(i) = icon {
        sqlx::query("UPDATE categories SET icon = ? WHERE id = ?")
            .bind(&i)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    if let Some(d) = description {
        sqlx::query("UPDATE categories SET description = ? WHERE id = ?")
            .bind(&d)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    if let Some(s) = schema_slug {
        if !SchemaSlug::is_known(&s) {
            return Err("INVALID_SCHEMA_SLUG".to_string());
        }
        let canonical = SchemaSlug::from_str(&s).as_str();
        sqlx::query("UPDATE categories SET schema_slug = ? WHERE id = ?")
            .bind(canonical)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    // Two ways to clear: explicit `clear_view_config: true`, or send
    // `view_config: ""`. Both collapse to NULL in the column. Sending
    // `view_config: null` is treated as "no change" so callers can omit
    // the field without erasing existing settings.
    if clear_view_config.unwrap_or(false) {
        sqlx::query("UPDATE categories SET view_config = NULL WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    } else if let Some(raw) = view_config {
        let normalized = normalize_view_config(Some(raw))?;
        sqlx::query("UPDATE categories SET view_config = ? WHERE id = ?")
            .bind(&normalized)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(CategoryUpdateResponse { success: true })
}

#[derive(serde::Serialize)]
pub struct CategoryUpdateResponse {
    pub success: bool,
}

#[derive(serde::Serialize)]
pub struct CategoryMergeResponse {
    pub moved: u32,
    /// Basenames the merge refused to move because the same filename
    /// already exists in the target folder (on disk or in DB). The user
    /// resolves these by hand — usually by renaming one side.
    pub skipped_duplicates: Vec<String>,
    /// True when every source file moved cleanly and the source category
    /// row + folder were removed. False when duplicates were skipped, in
    /// which case the source category is intentionally left in place so
    /// the user can resolve the conflicts.
    pub deleted_source: bool,
}

/// Merge every file from `source_id` into `target_id`, on disk and in the
/// DB, then delete the source category when the move is clean.
///
/// Same-schema only: refusing INCOMPATIBLE_SCHEMAS prevents merging e.g.
/// a comic category into a novel one, which would leave files using the
/// wrong metadata layout.
///
/// Duplicates (same basename already at target) are reported back rather
/// than auto-renamed — the same filename across two categories is almost
/// always the same series, and the user should pick which copy wins.
#[tauri::command]
pub async fn category_merge(
    app: AppHandle,
    source_id: i64,
    target_id: i64,
) -> Result<CategoryMergeResponse, String> {
    if source_id == target_id {
        return Err("SAME_CATEGORY".to_string());
    }

    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let source: (String, Option<String>, String) = sqlx::query_as(
        "SELECT name, folder_name, schema_slug FROM categories WHERE id = ?",
    )
    .bind(source_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("SOURCE_NOT_FOUND".to_string())?;

    let target: (String, Option<String>, String) = sqlx::query_as(
        "SELECT name, folder_name, schema_slug FROM categories WHERE id = ?",
    )
    .bind(target_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or("TARGET_NOT_FOUND".to_string())?;

    if source.2 != target.2 {
        return Err("INCOMPATIBLE_SCHEMAS".to_string());
    }

    let storage_path: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'",
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;
    let storage_root = match storage_path {
        Some((p,)) if !p.is_empty() => PathBuf::from(&p),
        _ => return Err("STORAGE_PATH_UNSET".to_string()),
    };
    let storage_root_str = storage_root.to_string_lossy().to_string();

    let source_folder = source
        .1
        .unwrap_or_else(|| sanitize_folder_name(&source.0));
    let target_folder = target
        .1
        .unwrap_or_else(|| sanitize_folder_name(&target.0));

    let target_dir = storage_root.join(&target_folder);
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Failed to create target folder: {}", e))?;
    }

    let files: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, path, storage_kind, local_cache_path FROM files WHERE category_id = ?",
    )
    .bind(source_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut moved: u32 = 0;
    let mut skipped: Vec<String> = Vec::new();

    for (file_id, old_path, storage_kind, old_cache) in files {
        let kind = storage_kind.as_deref().unwrap_or("local");

        // basename drives the target-side path regardless of storage_kind.
        // For local rows, `path` is the canonical location; for remote
        // rows, `local_cache_path` (when present) is what physically
        // lives under storage_path. Pick whichever the user actually has
        // on disk to derive the target name.
        let probe_rel = if kind == "remote" {
            old_cache.clone().unwrap_or_default()
        } else {
            old_path.clone()
        };
        let basename = PathBuf::from(&probe_rel)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        let basename = match basename {
            Some(b) if !b.is_empty() => b,
            _ => {
                // Row with no filename (defensive — shouldn't happen).
                // Just rewrite the DB pointer so the row follows the
                // category, and skip the filesystem move.
                sqlx::query(
                    "UPDATE files SET category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                )
                .bind(target_id)
                .bind(file_id)
                .execute(&pool)
                .await
                .map_err(|e| e.to_string())?;
                moved += 1;
                continue;
            }
        };

        let target_rel = format!("{}/{}", target_folder, basename);
        let target_abs = storage_root.join(&target_folder).join(&basename);

        // Duplicate check: another DB row or a file already on disk at
        // the destination wins, so we refuse to overwrite.
        let dup_in_db: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM files WHERE path = ? AND id != ?",
        )
        .bind(&target_rel)
        .bind(file_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;
        if dup_in_db.is_some() || target_abs.exists() {
            // For local rows we never moved anything; for remote rows the
            // physical file is the cache copy under storage_path, also
            // unmoved here. Either way, leave the source row untouched.
            skipped.push(basename);
            continue;
        }

        if kind == "remote" {
            // Remote row: the canonical `path` lives on a remote provider
            // (not on disk under storage_path), so we only rewrite the
            // local cache. The remote path itself doesn't carry a
            // category folder — `category_id` is the only link.
            if let Some(cache_rel) = old_cache {
                if !cache_rel.is_empty() {
                    let src_abs =
                        crate::path_resolve::cache_to_absolute(&cache_rel, &storage_root_str);
                    if src_abs.exists() {
                        fs::rename(&src_abs, &target_abs).map_err(|e| {
                            format!("Failed to move cache for file {}: {}", file_id, e)
                        })?;
                    }
                    sqlx::query(
                        "UPDATE files SET category_id = ?, local_cache_path = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                    )
                    .bind(target_id)
                    .bind(&target_rel)
                    .bind(file_id)
                    .execute(&pool)
                    .await
                    .map_err(|e| e.to_string())?;
                    moved += 1;
                    continue;
                }
            }
            // No cache to relocate; just re-parent the row.
            sqlx::query(
                "UPDATE files SET category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(target_id)
            .bind(file_id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
            moved += 1;
            continue;
        }

        // Local row: move the file on disk, then update path + category.
        let src_abs =
            crate::path_resolve::to_absolute(kind, &old_path, &storage_root_str, "");
        if src_abs.exists() {
            fs::rename(&src_abs, &target_abs)
                .map_err(|e| format!("Failed to move file {}: {}", file_id, e))?;
        }
        // Always rewrite the DB pointer even if the disk file was already
        // missing — fixes orphaned rows in the same pass.
        sqlx::query(
            "UPDATE files SET category_id = ?, path = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(target_id)
        .bind(&target_rel)
        .bind(file_id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
        moved += 1;
    }

    let deleted_source = if skipped.is_empty() {
        sqlx::query("DELETE FROM categories WHERE id = ?")
            .bind(source_id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        // Best-effort folder cleanup: only succeeds when the directory
        // is empty, which it should be after a clean merge. Hidden files
        // or in-flight writes leave it in place — not fatal.
        let _ = fs::remove_dir(storage_root.join(&source_folder));
        true
    } else {
        false
    };

    Ok(CategoryMergeResponse {
        moved,
        skipped_duplicates: skipped,
        deleted_source,
    })
}