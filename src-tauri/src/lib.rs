mod batch;
mod commands;
mod db;
mod filename;
mod queue;
mod state;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};
use zdm_core::DownloadEngine;

use db::Db;
use state::{AppState, Settings};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
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

            // Closing the window hides it to the tray instead of quitting —
            // downloads keep running in the background. "Quit" on the tray
            // menu is the only thing that actually ends the process.
            if let Some(window) = app.get_webview_window("main") {
                let window_for_close = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_for_close.hide();
                    }
                });
            }

            let show_item = MenuItem::with_id(app, "show", "Show ZDM", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().expect("app icon is embedded via tauri.conf.json"))
                .menu(&tray_menu)
                .tooltip("ZDM Download Manager")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_downloads,
            commands::list_queues,
            commands::get_settings,
            commands::update_settings,
            commands::choose_directory,
            commands::check_conflict,
            commands::add_download,
            commands::add_batch,
            commands::pause_download,
            commands::resume_download,
            commands::retry_download,
            commands::cancel_download,
            commands::remove_download,
            commands::delete_queue,
            commands::reorder_downloads,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
