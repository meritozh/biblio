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
pub async fn settings_get(
    app: AppHandle,
    key: String,
) -> Result<Option<String>, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = ?"
    )
    .bind(&key)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(result.map(|r| r.0))
}

#[tauri::command]
pub async fn settings_set(
    app: AppHandle,
    key: String,
    value: String,
) -> Result<(), String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Special handling for storage_path: block change if files exist in storage
    if key == "storage_path" {
        let files_in_storage: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM files WHERE in_storage = 1"
        )
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

        if files_in_storage.0 > 0 {
            return Err("STORAGE_PATH_CHANGE_BLOCKED".to_string());
        }

        // Validate path is accessible
        if !value.is_empty() {
            let path = std::path::PathBuf::from(&value);
            if !path.exists() {
                return Err("STORAGE_PATH_NOT_FOUND".to_string());
            }
            // Block system directories
            let path_str = path.to_string_lossy();
            let lower_path = path_str.to_lowercase();

            // Unix system directories
            let unix_dangerous = ["/", "/system", "/usr", "/bin", "/etc"];
            for dangerous in unix_dangerous {
                if lower_path == dangerous || lower_path.starts_with(&format!("{}/", dangerous)) {
                    // Allow if it's a user subdirectory like /Users/...
                    if dangerous == "/" && lower_path.starts_with("/users/") {
                        continue;
                    }
                    return Err("STORAGE_PATH_SYSTEM_DIRECTORY".to_string());
                }
            }

            // Windows system directories (check for patterns like C:\Windows, D:\Program Files, etc.)
            let windows_patterns = ["\\windows", "\\program files", "\\program files (x86)"];
            for pattern in windows_patterns {
                if lower_path.contains(pattern) {
                    return Err("STORAGE_PATH_SYSTEM_DIRECTORY".to_string());
                }
            }

            // Try to create a test file to verify write permission
            let test_file = path.join(".biblio_test");
            if std::fs::write(&test_file, b"").is_err() {
                return Err("STORAGE_PATH_NOT_WRITABLE".to_string());
            }
            let _ = std::fs::remove_file(&test_file);
        }
    }

    sqlx::query(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)"
    )
    .bind(&key)
    .bind(&value)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn storage_get_path(app: AppHandle) -> Result<Option<String>, String> {
    settings_get(app, "storage_path".to_string()).await
}

#[tauri::command]
pub async fn storage_check_access(app: AppHandle) -> Result<bool, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM app_settings WHERE key = 'storage_path'"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    let path = match result {
        Some((p,)) if !p.is_empty() => p,
        _ => return Ok(false),
    };

    let path = std::path::PathBuf::from(&path);
    Ok(path.exists() && path.is_dir())
}