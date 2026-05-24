mod commands;
mod database;
mod path_resolve;
mod pipeline;
mod providers;
mod schema;
mod services;

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
        .manage(commands::processing::PreparedCoverCache::new())
        .setup(|app| {
            // Long-running worker queues: spawn early so commands can push
            // jobs to the channels via the senders stashed in app state.
            use tauri::Manager;
            let app_handle = app.handle().clone();
            let upload_tx = services::upload_worker::spawn(app_handle.clone());
            let download_tx = services::download_worker::spawn(app_handle.clone());
            let delete_tx = services::delete_worker::spawn(app_handle.clone());
            let import_tx = services::import_worker::spawn(app_handle);
            app.manage(services::upload_worker::UploadQueueSender(upload_tx));
            app.manage(services::download_worker::DownloadQueueSender(download_tx));
            app.manage(services::delete_worker::DeleteQueueSender(delete_tx));
            app.manage(services::import_worker::ImportQueueSender(import_tx));
            Ok(())
        })
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
            commands::file::expand_drop_paths,
            commands::file::import_finalize,
            commands::file::file_move_category,
            commands::file::file_search,
            commands::file::file_check_status,
            commands::file::file_replace,
            commands::file::file_list_by_ids,
            commands::file::file_duplicate_groups,
            commands::file::file_count_novels_missing_tags,
            commands::file::file_reanalyze_missing_tags,
            commands::file::file_count_for_category_reanalyze,
            commands::file::file_reanalyze_for_category,
            commands::file::file_count_authorless_in_category,
            commands::file::file_assign_author_to_authorless,
            commands::file::file_count_comics_missing_covers,
            commands::file::file_regenerate_missing_covers,
            commands::file::collection_list,
            commands::category::category_list,
            commands::category::category_get,
            commands::category::category_update,
            commands::category::category_merge,
            commands::tag::tag_list,
            commands::tag::tag_count,
            commands::tag::tag_create,
            commands::tag::tag_update,
            commands::tag::tag_delete,
            commands::tag::tag_delete_unused,
            commands::tag::tag_assign,
            commands::tag::tag_unassign,
            commands::metadata::metadata_get,
            commands::metadata::metadata_set,
            commands::metadata::metadata_delete,
            commands::author::author_list,
            commands::author::author_count,
            commands::author::author_create,
            commands::author::author_update,
            commands::author::author_delete,
            commands::author::author_delete_unused,
            commands::author::author_assign,
            commands::author::author_unassign,
            commands::author::author_set,
            commands::cover::cover_set,
            commands::cover::cover_get,
            commands::cover::cover_delete,
            commands::cache::cache_open,
            commands::cache::cache_clear,
            commands::settings::settings_get,
            commands::settings::settings_set,
            commands::settings::storage_get_path,
            commands::settings::storage_check_access,
            database::recovery::db_check_integrity,
            database::recovery::db_create_backup,
            database::recovery::db_optimize,
            database::recovery::db_get_stats,
            commands::processing::enqueue_import,
            commands::processing::cancel_processing,
            commands::processing::prepared_cover_get,
            commands::processing::prepared_cover_clear,
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
            commands::remote::file_download_from_remote,
            commands::remote::file_delete_via_worker,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}