mod batch;
mod commands;
mod db;
mod queue;
mod state;

use tauri::Manager;
use zdm_core::DownloadEngine;

use db::Db;
use state::{AppState, Settings};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
            }

            let default_dir = app
                .path()
                .download_dir()
                .unwrap_or_else(|_| std::env::temp_dir())
                .to_string_lossy()
                .into_owned();

            let app_data_dir = app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir());
            std::fs::create_dir_all(&app_data_dir)?;
            let db = Db::open(&app_data_dir.join("zdm.sqlite3")).expect("failed to open local database");

            let initial_downloads = db.load_downloads();
            let initial_queues = db.load_queues();
            let settings = db.load_settings().unwrap_or_else(|| Settings::with_default_dir(default_dir));

            let (engine, events_rx) = DownloadEngine::new();
            app.manage(AppState::new(engine, db, initial_downloads, initial_queues, settings));
            queue::spawn_event_forwarder(app.handle().clone(), events_rx);

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                queue::resume_interrupted_downloads(&app_handle).await;
                queue::try_promote_queue(&app_handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_downloads,
            commands::list_queues,
            commands::get_settings,
            commands::update_settings,
            commands::choose_directory,
            commands::add_download,
            commands::add_batch,
            commands::pause_download,
            commands::resume_download,
            commands::retry_download,
            commands::cancel_download,
            commands::remove_download,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
