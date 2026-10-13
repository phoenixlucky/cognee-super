mod commands;
pub mod config;
mod error;
pub mod secrets;
mod state;

pub fn run() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tauri::Manager;

    let shutting_down = Arc::new(AtomicBool::new(false));
    let shutdown_once = shutting_down.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::save_settings,
            commands::set_provider_secret,
            commands::clear_provider_secret,
            commands::get_settings_summary,
            commands::get_recent_logs,
            commands::prepare_runtime,
            commands::cancel_runtime,
            commands::start_api,
            commands::start_mcp,
            commands::stop_api,
            commands::stop_mcp,
            commands::stop_all,
            commands::list_mcp_tools,
            commands::list_datasets,
            commands::create_dataset,
            commands::delete_dataset,
            commands::list_data,
            commands::upload_source,
            commands::cognify_dataset,
            commands::get_dataset_progress,
            commands::download_raw,
            commands::preview_raw,
            commands::delete_data,
            commands::get_graph,
            commands::list_sessions,
            commands::get_session
        ])
        .on_window_event(move |window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if !shutdown_once.swap(true, Ordering::SeqCst) {
                    let app = window.app_handle().clone();
                    tauri::async_runtime::spawn(async move {
                        let state = app.state::<state::AppState>();
                        let _ = commands::stop_all(state).await;
                        app.exit(0);
                    });
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run Cognee Desktop");
}
