mod commands;
pub mod config;
mod error;
pub mod secrets;
mod state;

pub fn run() {
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
            commands::download_raw,
            commands::preview_raw,
            commands::delete_data,
            commands::get_graph,
            commands::list_sessions,
            commands::get_session
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Cognee Desktop");
}
