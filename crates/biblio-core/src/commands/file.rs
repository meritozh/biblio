use crate::Library;
use crate::commands::validation::{sanitize_folder_name, validate_display_name};
use crate::commands::*;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

pub mod collections;
pub mod create_replace;
pub mod delete_move;
pub mod duplicates;
pub mod folder_import;
pub mod listing;
mod query_filter;
pub mod reanalyze;
pub mod search_lucky_status;
mod storage_files;
pub mod update_favorite;

#[cfg(test)]
mod tests;

pub(crate) use create_replace::rename_file_to_match_metadata;
pub use query_filter::FilterCondition;

use create_replace::{FileCreateResponse, file_create, move_file_to_category_folder};
use delete_move::file_move_category;
use listing::{FileListResponse, hydrate_file_items};
use query_filter::{build_filter_sql, custom_sort_clause, order_by_clause};
use search_lucky_status::{SearchFilter, prepare_search_filter};
use storage_files::{
    build_novel_filename, copy_file, get_unique_destination, move_file, sanitize_filename, zip_dir,
    zip_image_dir,
};
