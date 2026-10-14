mod commands;
pub mod config;
mod error;
pub mod secrets;
mod state;

#[cfg(test)]
mod tray_tests {
    use super::{TrayAction, tray_action};

    #[test]
    fn tray_action_maps_show_quit_and_unknown_ids() {
        assert_eq!(tray_action("show-window"), TrayAction::Show);
        assert_eq!(tray_action("quit"), TrayAction::Quit);
        assert_eq!(tray_action("unexpected"), TrayAction::Ignore);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TrayAction {
    Show,
    Quit,
    Ignore,
}

fn tray_action(id: &str) -> TrayAction {
    match id {
        "show-window" => TrayAction::Show,
        "quit" => TrayAction::Quit,
        _ => TrayAction::Ignore,
    }
}

pub fn run() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use tauri::{
        Manager,
        menu::{Menu, MenuItem},
        tray::TrayIconBuilder,
    };

    let shutting_down = Arc::new(AtomicBool::new(false));
    let shutdown_once = shutting_down.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .setup(|app| {
            let show = MenuItem::with_id(
                app,
                "show-window",
                "打开 Cognee Desktop",
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let icon = app
                .default_window_icon()
                .ok_or_else(|| std::io::Error::other("default app icon is missing"))?
                .clone();
            TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .on_menu_event(|app, event| match tray_action(event.id().as_ref()) {
                    TrayAction::Show => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    TrayAction::Quit => app.exit(0),
                    TrayAction::Ignore => {}
                })
                .build(app)?;
            Ok(())
        })
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
