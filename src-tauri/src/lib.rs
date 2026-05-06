mod commands;
mod database;
mod pipeline;
mod providers;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Shared cancellation flag for processing pipeline. The inner Arc lets the
/// pipeline runner hold a private handle without also keeping an
/// `AppHandle` alive; `cancel_processing` and the runner share one flag.
pub struct ProcessingCancelled(pub Arc<AtomicBool>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let migrations = database::get_migrations();

    tauri::Builder::default()
        .manage(ProcessingCancelled(Arc::new(AtomicBool::new(false))))
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:biblio.db", migrations)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            commands::file::file_list,
            commands::file::file_get,
            commands::file::file_create,
            commands::file::file_update,
            commands::file::file_delete,
            commands::file::file_delete_source,
            commands::file::list_files_in_folder,
            commands::file::import_finalize,
            commands::file::file_move_category,
            commands::file::file_search,
            commands::file::file_check_status,
            commands::file::file_replace,
            commands::file::file_list_by_tag,
            commands::file::file_list_by_author,
            commands::category::category_list,
            commands::category::category_get,
            commands::category::category_create,
            commands::category::category_update,
            commands::category::category_delete,
            commands::tag::tag_list,
            commands::tag::tag_create,
            commands::tag::tag_update,
            commands::tag::tag_delete,
            commands::tag::tag_assign,
            commands::tag::tag_unassign,
            commands::metadata::metadata_get,
            commands::metadata::metadata_set,
            commands::metadata::metadata_delete,
            commands::author::author_list,
            commands::author::author_create,
            commands::author::author_update,
            commands::author::author_delete,
            commands::author::author_assign,
            commands::author::author_unassign,
            commands::author::author_set,
            commands::cover::cover_set,
            commands::cover::cover_get,
            commands::cover::cover_delete,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::settings::storage_get_path,
            commands::settings::storage_check_access,
            database::recovery::db_check_integrity,
            database::recovery::db_create_backup,
            database::recovery::db_optimize,
            database::recovery::db_get_stats,
            commands::processing::file_prepare_import,
            commands::processing::cancel_processing,
            commands::llm::llm_config_get,
            commands::llm::llm_config_set,
            commands::llm::llm_test_connection,
            commands::prompts::prompt_list,
            commands::prompts::prompt_create,
            commands::prompts::prompt_update,
            commands::prompts::prompt_delete,
            commands::prompts::prompt_set_default,
            commands::remote::remote_config_get,
            commands::remote::remote_login,
            commands::remote::remote_logout,
            commands::remote::remote_get_authorize_url,
            commands::remote::file_upload_to_remote,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}