use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
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

#[derive(Serialize)]
pub struct CoverGetResponse {
    pub data: String,
    pub mime_type: String,
}

#[tauri::command]
pub async fn cover_set(
    app: AppHandle,
    file_id: i64,
    data: Vec<u8>,
    mime_type: Option<String>,
) -> Result<CoverSetResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let mime = mime_type.unwrap_or_else(|| "image/png".to_string());

    // Use INSERT OR REPLACE to handle both insert and update
    sqlx::query(
        "INSERT OR REPLACE INTO covers (file_id, data, mime_type) VALUES (?, ?, ?)"
    )
    .bind(file_id)
    .bind(&data)
    .bind(&mime)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(CoverSetResponse { success: true })
}

#[derive(Serialize)]
pub struct CoverSetResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn cover_get(
    app: AppHandle,
    file_id: i64,
) -> Result<CoverGetResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let result: Option<(Vec<u8>, String)> = sqlx::query_as(
        "SELECT data, mime_type FROM covers WHERE file_id = ?"
    )
    .bind(file_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| e.to_string())?;

    match result {
        Some((data, mime_type)) => {
            let base64_data = STANDARD.encode(&data);
            Ok(CoverGetResponse {
                data: base64_data,
                mime_type,
            })
        }
        None => Err("COVER_NOT_FOUND".to_string()),
    }
}

#[tauri::command]
pub async fn cover_delete(
    app: AppHandle,
    file_id: i64,
) -> Result<CoverDeleteResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    sqlx::query("DELETE FROM covers WHERE file_id = ?")
        .bind(file_id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CoverDeleteResponse { success: true })
}

#[derive(Serialize)]
pub struct CoverDeleteResponse {
    pub success: bool,
}