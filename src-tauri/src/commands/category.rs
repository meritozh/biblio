use crate::commands::*;
use crate::commands::validation::{sanitize_folder_name, validate_category_name};
use crate::schema::SchemaSlug;
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

#[tauri::command]
pub async fn category_create(
    app: AppHandle,
    name: String,
    icon: Option<String>,
    description: Option<String>,
    schema_slug: Option<String>,
    view_config: Option<String>,
) -> Result<CategoryCreateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let validated_name = validate_category_name(&name)?;
    // Whitelist user-provided slug — typos shouldn't silently fall back
    // to 'novel' here (they would at read time, but at write time the
    // user is choosing explicitly so we surface an error).
    let slug = match schema_slug.as_deref() {
        Some(s) if !s.is_empty() => {
            if !SchemaSlug::is_known(s) {
                return Err("INVALID_SCHEMA_SLUG".to_string());
            }
            SchemaSlug::from_str(s).as_str().to_string()
        }
        _ => SchemaSlug::Novel.as_str().to_string(),
    };
    let view_config = normalize_view_config(view_config)?;

    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM categories WHERE name = ?")
        .bind(&validated_name)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("CATEGORY_EXISTS".to_string());
    }

    // Generate unique folder name
    let base_folder = sanitize_folder_name(&validated_name);
    let folder_name = get_unique_folder_name(&pool, &base_folder).await?;

    let result = sqlx::query("INSERT INTO categories (name, description, icon, folder_name, schema_slug, view_config) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&validated_name)
        .bind(&description)
        .bind(&icon)
        .bind(&folder_name)
        .bind(&slug)
        .bind(&view_config)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CategoryCreateResponse {
        id: result.last_insert_rowid(),
    })
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

#[derive(serde::Serialize)]
pub struct CategoryCreateResponse {
    pub id: i64,
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
                        let path = std::path::PathBuf::from(&old_path);
                        if let Some(filename) = path.file_name() {
                            let new_file_path = std::path::PathBuf::from(&storage)
                                .join(&new_folder)
                                .join(filename);
                            sqlx::query("UPDATE files SET path = ? WHERE id = ?")
                                .bind(new_file_path.to_string_lossy().to_string())
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

#[tauri::command]
pub async fn category_delete(
    app: AppHandle,
    id: i64,
) -> Result<CategoryDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let is_default: (bool,) = sqlx::query_as("SELECT is_default FROM categories WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if is_default.0 {
        return Err("CANNOT_DELETE_DEFAULT".to_string());
    }

    let affected: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files WHERE category_id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if affected.0 > 0 {
        return Err("CATEGORY_HAS_FILES".to_string());
    }

    sqlx::query("DELETE FROM categories WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CategoryDeleteResponse {
        success: true,
    })
}

#[derive(serde::Serialize)]
pub struct CategoryDeleteResponse {
    pub success: bool,
}