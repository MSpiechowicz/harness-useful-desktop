//! Harness Desktop: the Tauri shell. It hosts the UI and exposes `harness_business` through
//! IPC commands; all domain logic lives in the business crate.

mod access;
mod agent_sink;
mod agents;
mod commands;
mod desktop_mcp;
mod dialogs;
mod doc_watch;
mod drops;
mod error;
mod hooks;
mod lifecycle;
mod logging;
mod mcp_cli;
mod memory_held;
mod memory_requests;
mod memory_setup;
mod native_text;
mod notifications;
mod releases;
mod routines;
pub mod state;
mod team_sync;
#[cfg(test)]
mod test_support;
mod tray;
mod updates;
mod window;

use std::process::ExitCode;

use tauri::Manager;

use crate::state::AppState;

/// Builds and runs the app until the last window closes and every agent session stopped.
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
        // Driven from the update commands only; its own JS commands are not granted to the webview.
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Driven from the script request thread only (see `notifications`); no JS command is granted.
        .plugin(notifications::plugin())
        .setup(|app| {
            app.manage(AppState::from_app(app.handle())?);
            app.state::<AppState>().agents.sweep_orphans_in_background();
            window::create_main_window(app)?;
            memory_setup::spawn_on_launch(app.handle());
            desktop_mcp::spawn_on_launch(app.handle());
            hooks::spawn_on_launch(app.handle());
            routines::agent_run::install(app.handle());
            routines::spawn_on_launch(app.handle());
            notifications::focus_on_click(app.handle());
            memory_requests::spawn(app.handle());
            tray::create_tray(app);
            // Opens the audio device now, so the intro's sound starts without delay.
            commands::sound::warm_up();

            let version = app.package_info().version.to_string();
            tracing::info!(%version, "Harness Desktop started");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::prefs_get,
            commands::app::prefs_set,
            commands::memory::memory_set_enabled,
            commands::memory_remote::memory_remote_get,
            commands::memory_remote::memory_remote_set,
            commands::memory_remote::memory_remote_test,
            commands::memory_scripts::memory_scripts_list,
            commands::memory_scripts::memory_script_get,
            commands::memory_scripts::memory_script_delete,
            commands::memory_scripts::memory_script_save,
            commands::memory_scripts::memory_scripts_delete_all,
            commands::memory_scripts::memory_scripts_trust,
            commands::memory_scripts::memory_script_hide,
            commands::memory_scripts::memory_scripts_discard_local,
            commands::memory_requests::memory_script_requests_listen,
            commands::memory_requests::memory_script_decide,
            commands::memory_guards::memory_guards_get,
            commands::memory_guards::memory_guards_update,
            commands::memory_guards::memory_chat_guards_set,
            commands::memory_guards::memory_held_list,
            commands::memory_guards::memory_held_save,
            commands::memory_guards::memory_held_skip,
            commands::memory_guards::memory_held_listen,
            commands::memory_guards::chat_memory_check_again,
            commands::memory_scripts::memory_repos_list,
            commands::memory_notes::memory_notes_list,
            commands::memory_notes::memory_note_get,
            commands::memory_notes::memory_note_delete,
            commands::memory_notes::memory_note_save,
            commands::memory_notes::memory_notes_delete_all,
            commands::memory_notes::memory_note_hide,
            commands::memory_notes::memory_notes_discard_local,
            commands::memory_notes_copy::memory_notes_copy_preview,
            commands::memory_notes_copy::memory_notes_copy_rows,
            commands::memory_notes_copy::memory_notes_copy_apply,
            commands::memory_notes_import::memory_notes_import_preview,
            commands::memory_notes_import::memory_notes_import_apply,
            commands::memory_notes_template::memory_notes_import_template,
            commands::harness::harness_list,
            commands::harness::harness_detect,
            commands::harness::harness_pick_binary,
            commands::harness::harness_clear_binary,
            commands::harness::config_view,
            commands::harness::extensions_view,
            commands::harness::commands_catalog,
            commands::projects::project_pick,
            commands::projects::dialog_pick_folder,
            commands::projects::project_open,
            commands::projects::project_close,
            commands::projects::project_recent,
            commands::projects::project_forget,
            commands::projects::project_scopes,
            commands::system::reveal_path,
            commands::system::open_external,
            commands::agent::agent_start,
            commands::agent::agent_send,
            commands::agent::agent_ui_respond,
            commands::agent::agent_bash,
            commands::agent::agent_stop,
            commands::agent::agent_move_project,
            commands::agent::agent_list,
            commands::sessions::sessions_list,
            commands::sessions::harness_session_totals,
            commands::sessions::sessions_folder,
            commands::sessions::session_tree,
            commands::sessions::session_messages,
            commands::sessions::session_files_touched,
            commands::sessions::session_delete,
            commands::git::git_status,
            commands::git::git_branches,
            commands::git::git_diff,
            commands::git::git_file_lines,
            commands::git::git_discard_all,
            commands::git::git_discard_files,
            commands::review::review_compose,
            commands::ecosystem::ecosystem_status,
            commands::ecosystem::ecosystem_releases,
            commands::ecosystem::dashboard_availability,
            commands::ecosystem::dashboard_query,
            commands::plugins::extension_action,
            commands::setup::setup_status,
            commands::setup::setup_install_dashboard,
            commands::setup::setup_install_memory,
            commands::setup::setup_install_opencode_skills,
            commands::setup::setup_install_codex_skills,
            commands::skills_migration::skills_migrate_names,
            commands::setup::setup_register_mcp,
            commands::setup::setup_register_memory,
            commands::desktop_mcp::setup_register_desktop_mcp,
            commands::hooks::hooks_list,
            commands::hooks::hook_save,
            commands::hooks::hook_delete,
            commands::hooks::hooks_delete_all,
            commands::hooks::hook_approve,
            commands::hooks::hook_reject,
            commands::hooks::hooks_family,
            commands::hooks::hooks_family_apply,
            commands::hooks::hooks_listen,
            commands::hooks::team::hooks_share,
            commands::hooks::team::hooks_team_remove,
            commands::hooks::team::hooks_team_refresh,
            commands::hooks::team::hooks_team_copy,
            commands::hooks::team::hooks_team_enable,
            commands::routines::routines_list,
            commands::routines::routine_get,
            commands::routines::routine_save,
            commands::routines::routine_delete,
            commands::routines::routines_delete_all,
            commands::routines::routine_approve,
            commands::routines::routine_reject,
            commands::routines::routines_listen,
            commands::routines::runs::routine_run_now,
            commands::routines::runs::routine_run_stop,
            commands::routines::runs::routine_runs,
            commands::routines::runs::routine_schedule_preview,
            commands::routines::chat::routine_run_attach,
            commands::routines::chat::routines_workspace,
            commands::routines::team::routines_share,
            commands::routines::team::routines_team_remove,
            commands::routines::team::routines_team_remove_mine,
            commands::routines::team::routines_team_refresh,
            commands::routines::team::routines_team_copy,
            commands::routines::team::routines_team_enable,
            commands::sync::sync_export_preview,
            commands::sync::sync_export,
            commands::sync::sync_export_save,
            commands::sync::sync_import_preview,
            commands::sync::sync_import_apply,
            commands::updates::update_check,
            commands::updates::update_install,
            commands::updates::app_relaunch,
            commands::sound::launch_sound,
            commands::drops::drop_listen,
            commands::drops::attachment_save,
        ])
        .on_page_load(lifecycle::on_page_load)
        .build(tauri::generate_context!());

    match result {
        Ok(app) => {
            // Exit waits (bounded) until every agent session stopped; see `lifecycle`.
            app.run(lifecycle::on_run_event);
            ExitCode::SUCCESS
        }
        Err(error) => {
            // The logger may not be running yet, so report on stderr as well.
            eprintln!("Harness Desktop failed: {error}");
            tracing::error!(%error, "application failed");

            ExitCode::FAILURE
        }
    }
}
