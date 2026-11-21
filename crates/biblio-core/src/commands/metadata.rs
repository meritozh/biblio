use crate::Library;
use crate::commands::*;
use crate::commands::validation::{validate_metadata_key, validate_metadata_value};
use serde::Serialize;

pub async fn metadata_get(
    app: Library,
    file_id: i64,
) -> Result<MetadataGetResponse, String> {
    let pool = app.pool.clone();

    let metadata: Vec<Metadata> = sqlx::query_as(
        "SELECT id, file_id, key, value, data_type FROM metadata WHERE file_id = ?"
    )
    .bind(file_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(MetadataGetResponse { metadata })
}

#[derive(Serialize)]
pub struct MetadataGetResponse {
    pub metadata: Vec<Metadata>,
}

pub async fn metadata_set(
    app: Library,
    file_id: i64,
    key: String,
    value: String,
    data_type: Option<String>,
) -> Result<MetadataSetResponse, String> {
    let pool = app.pool.clone();

    let validated_key = validate_metadata_key(&key)?;
    let validated_value = validate_metadata_value(&value)?;
    let dtype = data_type.unwrap_or_else(|| "text".to_string());

    let result = sqlx::query(
        "INSERT INTO metadata (file_id, key, value, data_type) VALUES (?, ?, ?, ?)
         ON CONFLICT(file_id, key) DO UPDATE SET value = excluded.value, data_type = excluded.data_type"
    )
    .bind(file_id)
    .bind(&validated_key)
    .bind(&validated_value)
    .bind(&dtype)
    .execute(&pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(MetadataSetResponse {
        id: result.last_insert_rowid(),
    })
}

#[derive(Serialize)]
pub struct MetadataSetResponse {
    pub id: i64,
}

pub async fn metadata_delete(
    app: Library,
    file_id: i64,
    key: String,
) -> Result<MetadataDeleteResponse, String> {
    let pool = app.pool.clone();

    sqlx::query("DELETE FROM metadata WHERE file_id = ? AND key = ?")
        .bind(file_id)
        .bind(&key)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(MetadataDeleteResponse { success: true })
}

#[derive(Serialize)]
pub struct MetadataDeleteResponse {
    pub success: bool,
}