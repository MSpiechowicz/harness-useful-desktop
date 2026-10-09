//! Harness Desktop: the Tauri shell. It hosts the UI and exposes `harness_business` through
//! IPC commands; all domain logic lives in the business crate.

mod access;
mod commands;
mod dialogs;
mod error;
mod logging;
pub mod state;
#[cfg(test)]
mod test_support;
mod window;

use std::process::ExitCode;

use tauri::Manager;

use crate::state::AppState;

/// Builds and runs the app until the last window closes.
pub fn run() -> ExitCode {
    let result = tauri::Builder::default()
        // Must be registered first so a second launch exits before anything else starts.
        // Its argv is ignored: nothing from another process is forwarded to the webview.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            window::focus_main_window(app);
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(logging::plugin())
        .setup(|app| {
            app.manage(AppState::from_app(app.handle())?);
            window::create_main_window(app)?;

            let version = app.package_info().version.to_string();
            tracing::info!(%version, "Harness Desktop started");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::prefs_get,
            commands::app::prefs_set,
            commands::harness::harness_list,
            commands::harness::harness_detect,
            commands::harness::harness_pick_binary,
            commands::harness::harness_clear_binary,
            commands::harness::config_view,
            commands::harness::extensions_view,
            commands::harness::commands_catalog,
            commands::projects::project_pick,
            commands::projects::project_open,
            commands::projects::project_close,
            commands::projects::project_recent,
            commands::projects::project_forget,
            commands::system::reveal_path,
            commands::system::open_external,
        ])
        .run(tauri::generate_context!());

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // The logger may not be running yet, so report on stderr as well.
            eprintln!("Harness Desktop failed: {error}");
            tracing::error!(%error, "application failed");

            ExitCode::FAILURE
        }
    }
}
