use crate::Library;
use std::fs;
use std::path::PathBuf;

pub struct DatabaseRecovery;

// `Recovered` and `RecoveryFailed` are reached by the restore flow
// (DatabaseRecovery::restore_from_backup); they're not yet wired into
// a user-facing command but the implementation is ready to expose
// once a "Manage backups" UI surface lands.
#[allow(dead_code)]
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
    pub fn get_database_path(app: &Library) -> Option<PathBuf> {
        // Backups and statistics both use the catalog in the active profile.
        Some(app.config_dir.join("biblio.db"))
    }

    pub fn get_backup_path(app: &Library) -> Option<PathBuf> {
        Self::get_database_path(app).map(|p| p.with_extension("sqlite.backup"))
    }

    pub async fn check_integrity(pool: &sqlx::SqlitePool) -> Result<bool, String> {
        let result: (String,) = sqlx::query_as("PRAGMA integrity_check")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(result.0 == "ok")
    }

    pub async fn create_backup(app: &Library) -> Result<PathBuf, String> {
        let backup_path = Self::get_backup_path(app).ok_or("Could not determine backup path")?;
        let snapshot = tempfile::NamedTempFile::new_in(&app.config_dir).map_err(|e| e.to_string())?;
        // VACUUM INTO snapshots the live connection, including committed WAL pages.
        sqlx::query("VACUUM INTO ?")
            .bind(snapshot.path().to_str().ok_or("Invalid backup path")?)
            .execute(&app.pool).await.map_err(|e| format!("Failed to create backup: {e}"))?;
        snapshot.persist(&backup_path).map_err(|e| format!("Failed to install backup: {e}"))?;
        Ok(backup_path)
    }

    // Not wired into a command yet — see the comment on RecoveryStatus.
    #[allow(dead_code)]
    pub fn restore_from_backup(app: &Library) -> Result<(), String> {
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

    pub fn get_database_size(app: &Library) -> Result<u64, String> {
        let db_path = Self::get_database_path(app).ok_or("Could not find database path")?;

        let metadata = fs::metadata(&db_path)
            .map_err(|e| format!("Failed to get database metadata: {}", e))?;

        Ok(metadata.len())
    }

    #[allow(dead_code)]
    pub fn list_backups(app: &Library) -> Result<Vec<PathBuf>, String> {
        let config_dir = app.config_dir.clone();

        if !config_dir.exists() {
            return Ok(Vec::new());
        }

        let backups: Vec<PathBuf> = fs::read_dir(&config_dir)
            .map_err(|e| format!("Failed to read directory: {}", e))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|name| name.starts_with("biblio.db") && (name.ends_with(".backup") || name.ends_with(".corrupted")))
                    .unwrap_or(false)
            })
            .collect();

        Ok(backups)
    }

    #[allow(dead_code)]
    pub fn cleanup_old_backups(app: &Library, keep_count: usize) -> Result<usize, String> {
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

pub async fn db_check_integrity(app: Library) -> Result<RecoveryResult, String> {
    let pool = app.pool.clone();
    let is_healthy = DatabaseRecovery::check_integrity(&pool).await?;

    Ok(RecoveryResult {
        status: if is_healthy { RecoveryStatus::Healthy } else { RecoveryStatus::Corrupted },
        backup_created: false,
        message: if is_healthy { "Database is healthy".to_string() } else { "Database corruption detected".to_string() },
    })
}

pub async fn db_create_backup(app: Library) -> Result<RecoveryResult, String> {
    let pool = app.pool.clone();
    let is_healthy = DatabaseRecovery::check_integrity(&pool).await?;

    if !is_healthy {
        return Ok(RecoveryResult {
            status: RecoveryStatus::Corrupted,
            backup_created: false,
            message: "Cannot create backup from corrupted database".to_string(),
        });
    }

    // Surface a real failure as Err so structured callers don't see a
    // "Healthy, no backup" result that hides the error in the message.
    DatabaseRecovery::create_backup(&app).await?;

    Ok(RecoveryResult {
        status: RecoveryStatus::Healthy,
        backup_created: true,
        message: "Backup created successfully".to_string(),
    })
}

pub async fn db_optimize(app: Library) -> Result<RecoveryResult, String> {
    let pool = app.pool.clone();
    DatabaseRecovery::optimize_database(&pool).await?;
    DatabaseRecovery::vacuum_database(&pool).await?;

    Ok(RecoveryResult {
        status: RecoveryStatus::Healthy,
        backup_created: false,
        message: "Database optimized successfully".to_string(),
    })
}

pub async fn db_get_stats(app: Library) -> Result<DatabaseStats, String> {
    let pool = app.pool.clone();
    // Propagate a failed size lookup rather than masking it as a 0-byte DB,
    // which would be indistinguishable from a genuinely empty file.
    let size = DatabaseRecovery::get_database_size(&app)?;

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