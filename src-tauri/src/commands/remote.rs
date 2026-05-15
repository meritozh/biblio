//! Tauri commands backing biblio's remote storage (Baidu Netdisk) flow.
//!
//! Config is persisted in `app_settings` as individual rows rather than a
//! dedicated table — keeps the settings surface uniform with the rest of
//! the app (LLM config, storage path, etc). Uses implicit grant OAuth:
//! the frontend obtains the access_token directly via the authorize URL
//! redirect and passes it here for storage.

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

use crate::providers::baidu_netdisk::{
    BaiduError, UploadResult, build_authorize_url, delete_file, upload_file,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConfig {
    pub enabled: bool,
    pub app_key: String,
    pub access_token: String,
    pub access_token_expires_at: i64,
    pub app_root: String,
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            app_key: String::new(),
            access_token: String::new(),
            access_token_expires_at: 0,
            app_root: "/apps/biblio".to_string(),
        }
    }
}

fn get_sqlite_pool(instances: &DbInstances, db_url: &str) -> Result<sqlx::SqlitePool, String> {
    let instances_lock = instances.0.try_read().map_err(|e| e.to_string())?;
    let db_pool = instances_lock.get(db_url).ok_or("Database not found")?;
    match db_pool {
        DbPool::Sqlite(pool) => Ok(pool.clone()),
    }
}

async fn read_setting(pool: &sqlx::SqlitePool, key: &str) -> Option<String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM app_settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
    row.map(|r| r.0)
}

async fn write_setting(
    pool: &sqlx::SqlitePool,
    key: &str,
    value: &str,
) -> Result<(), String> {
    sqlx::query("INSERT OR REPLACE INTO app_settings (key, value) VALUES (?, ?)")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn load_config(pool: &sqlx::SqlitePool) -> RemoteConfig {
    let enabled = read_setting(pool, "remote_enabled")
        .await
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let app_key = read_setting(pool, "remote_app_key")
        .await
        .unwrap_or_default();
    let access_token = read_setting(pool, "remote_access_token")
        .await
        .unwrap_or_default();
    let access_token_expires_at = read_setting(pool, "remote_access_token_expires_at")
        .await
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0);
    let app_root = read_setting(pool, "remote_app_root")
        .await
        .unwrap_or_else(|| "/apps/biblio".to_string());

    RemoteConfig {
        enabled,
        app_key,
        access_token,
        access_token_expires_at,
        app_root,
    }
}

async fn save_config(pool: &sqlx::SqlitePool, cfg: &RemoteConfig) -> Result<(), String> {
    write_setting(pool, "remote_enabled", &cfg.enabled.to_string()).await?;
    write_setting(pool, "remote_app_key", &cfg.app_key).await?;
    write_setting(pool, "remote_access_token", &cfg.access_token).await?;
    write_setting(
        pool,
        "remote_access_token_expires_at",
        &cfg.access_token_expires_at.to_string(),
    )
    .await?;
    write_setting(pool, "remote_app_root", &cfg.app_root).await?;
    Ok(())
}

pub async fn ensure_access_token(pool: &sqlx::SqlitePool) -> Result<String, String> {
    let cfg = load_config(pool).await;
    if !cfg.enabled || cfg.access_token.is_empty() {
        return Err("REMOTE_NOT_AUTHENTICATED".into());
    }

    let now = chrono::Utc::now().timestamp();
    if cfg.access_token_expires_at > now + 60 {
        return Ok(cfg.access_token);
    }

    Err("ACCESS_TOKEN_EXPIRED".into())
}

#[tauri::command]
pub async fn remote_config_get(app: tauri::AppHandle) -> Result<RemoteConfig, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    Ok(load_config(&pool).await)
}

#[tauri::command]
pub async fn remote_get_authorize_url(app_key: String) -> Result<String, String> {
    build_authorize_url(&app_key).map_err(|e: BaiduError| e.0)
}

#[tauri::command]
pub async fn remote_login(
    app: tauri::AppHandle,
    app_key: String,
    access_token: String,
    expires_in_secs: i64,
    app_root: Option<String>,
) -> Result<RemoteConfig, String> {
    if app_key.trim().is_empty() {
        return Err("AppKey is required".into());
    }
    if access_token.trim().is_empty() {
        return Err("Access token is required".into());
    }

    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let mut cfg = load_config(&pool).await;
    cfg.app_key = app_key;
    cfg.access_token = access_token;
    cfg.access_token_expires_at = chrono::Utc::now().timestamp() + expires_in_secs;
    cfg.enabled = true;
    if let Some(root) = app_root.filter(|s| !s.is_empty()) {
        cfg.app_root = root;
    }
    save_config(&pool, &cfg).await?;

    Ok(cfg)
}

#[tauri::command]
pub async fn remote_logout(app: tauri::AppHandle) -> Result<(), String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let mut cfg = load_config(&pool).await;
    cfg.enabled = false;
    cfg.access_token = String::new();
    cfg.access_token_expires_at = 0;
    save_config(&pool, &cfg).await
}

/// Upload helper usable from other command handlers (e.g. `file_create`
/// when `storage_kind == remote`). Handles token refresh transparently.
pub async fn upload_to_remote(
    pool: &sqlx::SqlitePool,
    local_path: &std::path::Path,
    remote_path: &str,
) -> Result<UploadResult, String> {
    let access_token = ensure_access_token(pool).await?;
    upload_file(&access_token, local_path, remote_path)
        .await
        .map_err(|e: BaiduError| e.0)
}

pub async fn delete_on_remote(pool: &sqlx::SqlitePool, remote_path: &str) -> Result<(), String> {
    let access_token = ensure_access_token(pool).await?;
    delete_file(&access_token, remote_path)
        .await
        .map_err(|e: BaiduError| e.0)
}

/// Push a batch of file IDs into the upload worker queue and return immediately.
///
/// Producer-consumer model: the worker drains jobs serially and emits
/// `remote-upload-progress` events per state transition. The user can call
/// this again while previous uploads are in flight; new IDs append to the
/// queue.
#[tauri::command]
pub async fn file_upload_to_remote(
    app: tauri::AppHandle,
    file_ids: Vec<i64>,
) -> Result<(), String> {
    use crate::services::upload_worker::{UploadJob, UploadQueueSender};

    let sender = app.state::<UploadQueueSender>();
    for file_id in file_ids {
        sender
            .0
            .send(UploadJob { file_id })
            .map_err(|e| format!("Upload queue is closed: {e}"))?;
    }
    Ok(())
}

/// Push a batch of file IDs into the download worker queue. The worker
/// pulls each remote file's bytes to `<storage_path>/.cache/` and records
/// the resulting path on the row, leaving the cloud copy in place. Per-file
/// progress arrives as `remote-download-progress` events.
#[tauri::command]
pub async fn file_download_from_remote(
    app: tauri::AppHandle,
    file_ids: Vec<i64>,
) -> Result<(), String> {
    use crate::services::download_worker::{DownloadJob, DownloadQueueSender};

    let sender = app.state::<DownloadQueueSender>();
    for file_id in file_ids {
        sender
            .0
            .send(DownloadJob { file_id })
            .map_err(|e| format!("Download queue is closed: {e}"))?;
    }
    Ok(())
}

/// Push a batch of file IDs into the delete worker queue. Strict mode
/// for remote files (a failed cloud delete aborts the row's removal so the
/// user can retry); fires `remote-delete-progress` events per file.
#[tauri::command]
pub async fn file_delete_via_worker(
    app: tauri::AppHandle,
    file_ids: Vec<i64>,
) -> Result<(), String> {
    use crate::services::delete_worker::{DeleteJob, DeleteQueueSender};

    let sender = app.state::<DeleteQueueSender>();
    for file_id in file_ids {
        sender
            .0
            .send(DeleteJob { file_id })
            .map_err(|e| format!("Delete queue is closed: {e}"))?;
    }
    Ok(())
}
