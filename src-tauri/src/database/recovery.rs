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

pub struct DatabaseRecovery;

#[derive(Debug, serde::Serialize)]
pub enum RecoveryStatus {
    Healthy,
    Corrupted,
    Recovered,
    RecoveryFailed,
}

#[derive(Debug, serde::Serialize)]
pub struct RecoveryResult {
    pub status: RecoveryStatus,
    pub backup_created: bool,
    pub message: String,
}

impl DatabaseRecovery {
    pub fn get_database_path(app: &AppHandle) -> Option<PathBuf> {
        app.path().app_data_dir().ok().map(|p| p.join("database.sqlite"))
    }

    pub fn get_backup_path(app: &AppHandle) -> Option<PathBuf> {
        Self::get_database_path(app).map(|p| p.with_extension("sqlite.backup"))
    }

    pub async fn check_integrity(pool: &sqlx::SqlitePool) -> Result<bool, String> {
        let result: (String,) = sqlx::query_as("PRAGMA integrity_check")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(result.0 == "ok")
    }

    pub fn create_backup(app: &AppHandle) -> Result<PathBuf, String> {
        let db_path = Self::get_database_path(app).ok_or("Could not find database path")?;
        let backup_path = Self::get_backup_path(app).ok_or("Could not determine backup path")?;

        if !db_path.exists() {
            return Err("Database file does not exist".to_string());
        }

        fs::copy(&db_path, &backup_path)
            .map_err(|e| format!("Failed to create backup: {}", e))?;

        Ok(backup_path)
    }

    pub fn restore_from_backup(app: &AppHandle) -> Result<(), String> {
        let db_path = Self::get_database_path(app).ok_or("Could not find database path")?;
        let backup_path = Self::get_backup_path(app).ok_or("Could not determine backup path")?;

        if !backup_path.exists() {
            return Err("Backup file does not exist".to_string());
        }

        if db_path.exists() {
            let corrupted_path = db_path.with_extension("sqlite.corrupted");
            fs::rename(&db_path, &corrupted_path)
                .map_err(|e| format!("Failed to rename corrupted database: {}", e))?;
        }

        fs::copy(&backup_path, &db_path)
            .map_err(|e| format!("Failed to restore backup: {}", e))?;

        Ok(())
    }

    pub async fn vacuum_database(pool: &sqlx::SqlitePool) -> Result<(), String> {
        sqlx::query("VACUUM")
            .execute(pool)
            .await
            .map_err(|e| format!("VACUUM failed: {}", e))?;

        Ok(())
    }

    pub async fn optimize_database(pool: &sqlx::SqlitePool) -> Result<(), String> {
        sqlx::query("PRAGMA optimize")
            .execute(pool)
            .await
            .map_err(|e| format!("OPTIMIZE failed: {}", e))?;

        Ok(())
    }

    pub fn get_database_size(app: &AppHandle) -> Result<u64, String> {
        let db_path = Self::get_database_path(app).ok_or("Could not find database path")?;

        let metadata = fs::metadata(&db_path)
            .map_err(|e| format!("Failed to get database metadata: {}", e))?;

        Ok(metadata.len())
    }

    pub fn list_backups(app: &AppHandle) -> Result<Vec<PathBuf>, String> {
        let data_dir = app.path().app_data_dir()
            .map_err(|e| format!("Could not get app data directory: {}", e))?;

        if !data_dir.exists() {
            return Ok(Vec::new());
        }

        let backups: Vec<PathBuf> = fs::read_dir(&data_dir)
            .map_err(|e| format!("Failed to read directory: {}", e))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .map(|ext| ext == "backup" || ext == "corrupted")
                    .unwrap_or(false)
            })
            .collect();

        Ok(backups)
    }

    pub fn cleanup_old_backups(app: &AppHandle, keep_count: usize) -> Result<usize, String> {
        let backups = Self::list_backups(app)?;
        let mut removed = 0;

        if backups.len() > keep_count {
            let mut sorted_backups: Vec<_> = backups.into_iter().collect();
            sorted_backups.sort_by_key(|p| {
                fs::metadata(p)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            });

            let to_remove = sorted_backups.len() - keep_count;
            for path in sorted_backups.into_iter().take(to_remove) {
                if fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }

        Ok(removed)
    }
}

#[tauri::command]
pub async fn db_check_integrity(app: AppHandle) -> Result<RecoveryResult, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let is_healthy = DatabaseRecovery::check_integrity(&pool).await?;

    Ok(RecoveryResult {
        status: if is_healthy { RecoveryStatus::Healthy } else { RecoveryStatus::Corrupted },
        backup_created: false,
        message: if is_healthy { "Database is healthy".to_string() } else { "Database corruption detected".to_string() },
    })
}

#[tauri::command]
pub async fn db_create_backup(app: AppHandle) -> Result<RecoveryResult, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let is_healthy = DatabaseRecovery::check_integrity(&pool).await?;

    if !is_healthy {
        return Ok(RecoveryResult {
            status: RecoveryStatus::Corrupted,
            backup_created: false,
            message: "Cannot create backup from corrupted database".to_string(),
        });
    }

    match DatabaseRecovery::create_backup(&app) {
        Ok(_) => Ok(RecoveryResult {
            status: RecoveryStatus::Healthy,
            backup_created: true,
            message: "Backup created successfully".to_string(),
        }),
        Err(e) => Ok(RecoveryResult {
            status: RecoveryStatus::Healthy,
            backup_created: false,
            message: e,
        }),
    }
}

#[tauri::command]
pub async fn db_optimize(app: AppHandle) -> Result<RecoveryResult, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    DatabaseRecovery::optimize_database(&pool).await?;
    DatabaseRecovery::vacuum_database(&pool).await?;

    Ok(RecoveryResult {
        status: RecoveryStatus::Healthy,
        backup_created: false,
        message: "Database optimized successfully".to_string(),
    })
}

#[tauri::command]
pub async fn db_get_stats(app: AppHandle) -> Result<DatabaseStats, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    let size = DatabaseRecovery::get_database_size(&app).unwrap_or(0);

    let file_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files")
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let tag_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tags")
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    let category_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM categories")
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(DatabaseStats {
        size_bytes: size,
        file_count: file_count.0,
        tag_count: tag_count.0,
        category_count: category_count.0,
    })
}

#[derive(Debug, serde::Serialize)]
pub struct DatabaseStats {
    pub size_bytes: u64,
    pub file_count: i64,
    pub tag_count: i64,
    pub category_count: i64,
}