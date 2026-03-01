use crate::commands::*;
use crate::commands::validation::validate_category_name;
use sqlx::SqlitePool;
use tauri::State;

#[tauri::command]
pub async fn category_list(pool: State<'_, SqlitePool>) -> Result<Vec<Category>, String> {
    sqlx::query_as("SELECT id, name, icon, is_default, created_at FROM categories ORDER BY name")
        .fetch_all(&*pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn category_get(pool: State<'_, SqlitePool>, id: i64) -> Result<Category, String> {
    sqlx::query_as("SELECT id, name, icon, is_default, created_at FROM categories WHERE id = ?")
        .bind(id)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("CATEGORY_NOT_FOUND".to_string())
}

#[tauri::command]
pub async fn category_create(
    pool: State<'_, SqlitePool>,
    name: String,
    icon: Option<String>,
) -> Result<CategoryCreateResponse, String> {
    let validated_name = validate_category_name(&name)?;

    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM categories WHERE name = ?")
        .bind(&validated_name)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    if existing.is_some() {
        return Err("CATEGORY_EXISTS".to_string());
    }

    let result = sqlx::query("INSERT INTO categories (name, icon) VALUES (?, ?)")
        .bind(&validated_name)
        .bind(&icon)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CategoryCreateResponse {
        id: result.last_insert_rowid(),
    })
}

#[derive(serde::Serialize)]
pub struct CategoryCreateResponse {
    pub id: i64,
}

#[tauri::command]
pub async fn category_update(
    pool: State<'_, SqlitePool>,
    id: i64,
    name: Option<String>,
    icon: Option<String>,
) -> Result<CategoryUpdateResponse, String> {
    if let Some(n) = name {
        let validated_name = validate_category_name(&n)?;
        sqlx::query("UPDATE categories SET name = ? WHERE id = ?")
            .bind(&validated_name)
            .bind(id)
            .execute(&*pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    if let Some(i) = icon {
        sqlx::query("UPDATE categories SET icon = ? WHERE id = ?")
            .bind(&i)
            .bind(id)
            .execute(&*pool)
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
    pool: State<'_, SqlitePool>,
    id: i64,
) -> Result<CategoryDeleteResponse, String> {
    let is_default: (bool,) = sqlx::query_as("SELECT is_default FROM categories WHERE id = ?")
        .bind(id)
        .fetch_one(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    if is_default.0 {
        return Err("CANNOT_DELETE_DEFAULT".to_string());
    }

    let affected: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files WHERE category_id = ?")
        .bind(id)
        .fetch_one(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM categories WHERE id = ?")
        .bind(id)
        .execute(&*pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(CategoryDeleteResponse {
        success: true,
        affected_files: affected.0,
    })
}

#[derive(serde::Serialize)]
pub struct CategoryDeleteResponse {
    pub success: bool,
    pub affected_files: i64,
}