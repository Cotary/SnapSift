mod commands;
mod db;
mod dedup;
mod embedder;
mod models;
mod organizer;
mod scanner;
mod thumbnail;

use db::Database;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let database =
                Database::new(&app_data_dir).expect("Failed to initialize database");
            app.manage(database);

            // Initialize AI embedder (gracefully handles missing model)
            let resource_dir = app
                .path()
                .resource_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            embedder::init_embedder(&resource_dir);

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_project,
            commands::list_projects,
            commands::get_project_detail,
            commands::add_source_folders,
            commands::remove_source_folder,
            commands::set_target_dir,
            commands::delete_project,
            commands::start_scan,
            commands::scan_target,
            commands::get_project_files,
            commands::organize_files,
            commands::find_duplicates,
            commands::get_duplicate_groups,
            commands::get_ai_status,
            commands::set_ai_engine,
            commands::get_thumbnail,
            commands::delete_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
