pub mod file;
pub mod category;
pub mod tag;
pub mod metadata;
pub mod validation;
pub mod author;
pub mod cache;
pub mod cover;
pub mod settings;
pub mod processing;
pub mod llm;
pub mod prompts;
pub mod remote;
pub mod vndb;

#[cfg(test)]
pub mod test_helpers;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub is_default: bool,
    pub folder_name: Option<String>,
    /// Drives form sections, card layout, and prompt resolution. See
    /// `crate::schema::SchemaSlug` for the parsed view; the frontend
    /// mirrors the registry in `src/lib/categorySchema.ts`.
    pub schema_slug: String,
    /// User-tuned view defaults: which view mode the file list opens in,
    /// the default sort, the default filter conditions, the default
    /// storage destination. Opaque JSON so the schema doesn't need a new
    /// column every time the UI grows a new tunable — the frontend owns
    /// the shape (`CategoryViewConfig`). `None` means "use the schema
    /// defaults from the frontend REGISTRY".
    pub view_config: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct FileEntry {
    pub id: i64,
    pub path: String,
    pub display_name: String,
    pub category_id: Option<i64>,
    pub file_status: String,
    pub in_storage: bool,
    pub original_path: Option<String>,
    pub progress: Option<String>,
    pub storage_kind: Option<String>,
    pub remote_provider: Option<String>,
    pub local_cache_path: Option<String>,
    pub is_favorite: bool,
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
pub struct Author {
    pub id: i64,
    pub name: String,
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
    pub in_storage: bool,
    pub original_path: Option<String>,
    pub progress: Option<String>,
    pub storage_kind: Option<String>,
    pub remote_provider: Option<String>,
    pub local_cache_path: Option<String>,
    pub is_favorite: bool,
    pub created_at: String,
    pub updated_at: String,
    pub category: Option<Category>,
    pub tags: Vec<Tag>,
    pub authors: Vec<Author>,
    pub metadata: Vec<Metadata>,
}

#[derive(Debug, Serialize)]
pub struct FileListItem {
    pub id: i64,
    pub path: String,
    pub display_name: String,
    pub category_id: Option<i64>,
    pub file_status: String,
    pub in_storage: bool,
    pub original_path: Option<String>,
    pub progress: Option<String>,
    pub storage_kind: Option<String>,
    pub remote_provider: Option<String>,
    pub local_cache_path: Option<String>,
    pub is_favorite: bool,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<Tag>,
    pub authors: Vec<Author>,
}

#[derive(Debug, Deserialize)]
pub struct MetadataInput {
    pub key: String,
    pub value: String,
    pub data_type: Option<String>,
}

/// Wire-format DTO: the frontend sends `metadataFilters: [{ key, value }]`
/// on `file_search`, and the backend currently ignores the filter set
/// (the `_metadata_filters` parameter is underscored). Keep the type so
/// the IPC contract doesn't drift — removing it would error the
/// frontend's `metadata_filters` payload at deserialization time.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MetadataFilter {
    pub key: String,
    pub value: String,
}
