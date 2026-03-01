pub mod file;
pub mod category;
pub mod tag;
pub mod metadata;
pub mod validation;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub icon: Option<String>,
    pub is_default: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct FileEntry {
    pub id: i64,
    pub path: String,
    pub display_name: String,
    pub category_id: Option<i64>,
    pub file_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Metadata {
    pub id: i64,
    pub file_id: i64,
    pub key: String,
    pub value: String,
    pub data_type: String,
}

#[derive(Debug, Serialize)]
pub struct FileWithDetails {
    pub id: i64,
    pub path: String,
    pub display_name: String,
    pub category_id: Option<i64>,
    pub file_status: String,
    pub created_at: String,
    pub updated_at: String,
    pub category: Option<Category>,
    pub tags: Vec<Tag>,
    pub metadata: Vec<Metadata>,
}

#[derive(Debug, Deserialize)]
pub struct MetadataInput {
    pub key: String,
    pub value: String,
    pub data_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MetadataFilter {
    pub key: String,
    pub value: String,
}