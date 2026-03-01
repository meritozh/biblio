mod commands;
mod database;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let migrations = database::get_migrations();

    tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:biblio.db", migrations)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::file::file_list,
            commands::file::file_get,
            commands::file::file_create,
            commands::file::file_update,
            commands::file::file_delete,
            commands::file::file_search,
            commands::file::file_check_status,
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
            database::recovery::db_check_integrity,
            database::recovery::db_create_backup,
            database::recovery::db_optimize,
            database::recovery::db_get_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}