//! Tauri commands backing biblio's remote storage (Baidu Netdisk) flow.
//!
//! Config is persisted in `app_settings` as individual rows rather than a
//! dedicated table — keeps the settings surface uniform with the rest of
//! the app (LLM config, storage path, etc). Refresh tokens rotate on
//! every refresh so every success path writes the new token back before
//! returning.

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_sql::{DbInstances, DbPool};

use crate::providers::baidu_netdisk::{
    AuthMode, BaiduCredentials, BaiduError, UploadResult, delete_file, refresh_access_token,
    upload_file,
};

/// User-facing config returned to the frontend. Tokens are included so
/// the settings UI can show "logged in as ..." / "expires in N minutes";
/// the frontend redacts them before display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConfig {
    pub enabled: bool,
    pub auth_mode: String, // "openlist_proxy" | "self_app"
    pub refresh_token: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub access_token: String,
    /// Unix seconds. Values ≤ now indicate we need a refresh before the
    /// next upload/delete.
    pub access_token_expires_at: i64,
    /// Absolute directory in the user's Baidu Pan where uploads land.
    /// Default `/apps/biblio`; created implicitly on first upload.
    pub app_root: String,
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auth_mode: "openlist_proxy".to_string(),
            refresh_token: String::new(),
            client_id: None,
            client_secret: None,
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
    let auth_mode = read_setting(pool, "remote_auth_mode")
        .await
        .unwrap_or_else(|| "openlist_proxy".to_string());
    let refresh_token = read_setting(pool, "remote_refresh_token")
        .await
        .unwrap_or_default();
    let client_id = read_setting(pool, "remote_client_id").await;
    let client_secret = read_setting(pool, "remote_client_secret").await;
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
        auth_mode,
        refresh_token,
        client_id,
        client_secret,
        access_token,
        access_token_expires_at,
        app_root,
    }
}

async fn save_config(pool: &sqlx::SqlitePool, cfg: &RemoteConfig) -> Result<(), String> {
    write_setting(pool, "remote_enabled", &cfg.enabled.to_string()).await?;
    write_setting(pool, "remote_auth_mode", &cfg.auth_mode).await?;
    write_setting(pool, "remote_refresh_token", &cfg.refresh_token).await?;
    write_setting(
        pool,
        "remote_client_id",
        cfg.client_id.as_deref().unwrap_or(""),
    )
    .await?;
    write_setting(
        pool,
        "remote_client_secret",
        cfg.client_secret.as_deref().unwrap_or(""),
    )
    .await?;
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

fn creds_from_config(cfg: &RemoteConfig) -> Result<BaiduCredentials, String> {
    let mode = match cfg.auth_mode.as_str() {
        "openlist_proxy" => AuthMode::OpenListProxy,
        "self_app" => AuthMode::SelfApp,
        other => return Err(format!("unknown auth_mode: {other}")),
    };
    Ok(BaiduCredentials {
        auth_mode: mode,
        refresh_token: cfg.refresh_token.clone(),
        client_id: cfg.client_id.clone(),
        client_secret: cfg.client_secret.clone(),
    })
}

/// Renew `access_token` if it's missing or within 60s of expiry. Persists
/// the rotated refresh_token back to settings on success.
pub async fn ensure_access_token(pool: &sqlx::SqlitePool) -> Result<String, String> {
    let mut cfg = load_config(pool).await;
    if !cfg.enabled || cfg.refresh_token.is_empty() {
        return Err("Remote storage not configured".into());
    }

    let now = chrono::Utc::now().timestamp();
    if !cfg.access_token.is_empty() && cfg.access_token_expires_at > now + 60 {
        return Ok(cfg.access_token);
    }

    let creds = creds_from_config(&cfg)?;
    let refreshed = refresh_access_token(&creds)
        .await
        .map_err(|e: BaiduError| e.0)?;

    cfg.access_token = refreshed.access_token.clone();
    cfg.refresh_token = refreshed.refresh_token;
    cfg.access_token_expires_at = now + refreshed.expires_in_secs;
    save_config(pool, &cfg).await?;

    Ok(refreshed.access_token)
}

#[tauri::command]
pub async fn remote_config_get(app: tauri::AppHandle) -> Result<RemoteConfig, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;
    Ok(load_config(&pool).await)
}

/// Accept a user-supplied refresh_token (and optionally client_id/secret
/// for SelfApp mode) and validate it by doing one refresh. On success,
/// persist the config + fresh access_token and flip `enabled=true`.
#[tauri::command]
pub async fn remote_login(
    app: tauri::AppHandle,
    auth_mode: String,
    refresh_token: String,
    client_id: Option<String>,
    client_secret: Option<String>,
    app_root: Option<String>,
) -> Result<RemoteConfig, String> {
    if refresh_token.trim().is_empty() {
        return Err("refresh_token is required".into());
    }
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let mut cfg = load_config(&pool).await;
    cfg.auth_mode = auth_mode;
    cfg.refresh_token = refresh_token;
    cfg.client_id = client_id.filter(|s| !s.is_empty());
    cfg.client_secret = client_secret.filter(|s| !s.is_empty());
    if let Some(root) = app_root.filter(|s| !s.is_empty()) {
        cfg.app_root = root;
    }

    let creds = creds_from_config(&cfg)?;
    let refreshed = refresh_access_token(&creds)
        .await
        .map_err(|e: BaiduError| e.0)?;

    cfg.enabled = true;
    cfg.access_token = refreshed.access_token;
    cfg.refresh_token = refreshed.refresh_token;
    cfg.access_token_expires_at = chrono::Utc::now().timestamp() + refreshed.expires_in_secs;
    save_config(&pool, &cfg).await?;

    Ok(cfg)
}

#[tauri::command]
pub async fn remote_logout(app: tauri::AppHandle) -> Result<(), String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // Wipe tokens but keep app_root / auth_mode so re-login later doesn't
    // force the user to re-enter everything.
    let mut cfg = load_config(&pool).await;
    cfg.enabled = false;
    cfg.refresh_token = String::new();
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
