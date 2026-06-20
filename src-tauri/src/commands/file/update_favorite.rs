use super::*;
#[tauri::command]
pub async fn file_update(
    app: AppHandle,
    id: i64,
    display_name: Option<String>,
    category_id: Option<i64>,
    progress: Option<String>,
) -> Result<FileUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    // If the category changed, physically move the file first so that the
    // subsequent rename_file_to_match_metadata operates on the new location.
    if let Some(new_cat) = category_id {
        move_file_to_category_folder(&pool, id, Some(new_cat)).await?;
    }

    match (display_name, category_id, progress) {
        (Some(name), Some(cat_id), Some(prog)) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, category_id = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(cat_id)
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (Some(name), Some(cat_id), None) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(cat_id)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (Some(name), None, Some(prog)) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&validated_name)
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (Some(name), None, None) => {
            let validated_name = validate_display_name(&name)?;
            sqlx::query(
                "UPDATE files SET display_name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(&validated_name)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, Some(cat_id), Some(prog)) => {
            sqlx::query(
                "UPDATE files SET category_id = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(cat_id)
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, Some(cat_id), None) => {
            sqlx::query(
                "UPDATE files SET category_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(cat_id)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, None, Some(prog)) => {
            sqlx::query(
                "UPDATE files SET progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(&prog)
            .bind(id)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        (None, None, None) => {}
    }

    // Rename file on disk to match updated metadata (atomic with DB)
    let _ = rename_file_to_match_metadata(&pool, id).await;

    Ok(FileUpdateResponse { success: true })
}

#[derive(Serialize)]
pub struct FileUpdateResponse {
    pub success: bool,
}

#[tauri::command]
pub async fn file_set_favorite(
    app: AppHandle,
    id: i64,
    is_favorite: bool,
) -> Result<FileUpdateResponse, String> {
    let instances = app.state::<DbInstances>();
    let pool = get_sqlite_pool(&instances, "sqlite:biblio.db")?;

    let result = sqlx::query("UPDATE files SET is_favorite = ? WHERE id = ?")
        .bind(is_favorite)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;

    if result.rows_affected() == 0 {
        return Err("File not found".to_string());
    }

    Ok(FileUpdateResponse { success: true })
}
