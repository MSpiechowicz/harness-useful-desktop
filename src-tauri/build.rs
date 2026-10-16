//! Build script: generates the Tauri context and the app ACL manifest.
//!
//! Listing the commands in an [`AppManifest`] makes Tauri generate `allow-<command>` /
//! `deny-<command>` permissions for them and enforce the ACL on app commands, so every command
//! must be granted explicitly in `capabilities/`.

use tauri_build::{AppManifest, Attributes};

/// Every `#[tauri::command]` in `src/commands`; keep in sync with `generate_handler!` in `lib.rs`.
const COMMANDS: &[&str] = &[
    "app_info",
    "drop_listen",
    "attachment_save",
    "prefs_get",
    "prefs_set",
    "memory_set_enabled",
    "harness_list",
    "harness_detect",
    "harness_pick_binary",
    "harness_clear_binary",
    "config_view",
    "extensions_view",
    "commands_catalog",
    "project_pick",
    "project_open",
    "project_close",
    "project_recent",
    "project_forget",
    "reveal_path",
    "open_external",
    "agent_start",
    "agent_send",
    "agent_ui_respond",
    "agent_bash",
    "agent_stop",
    "agent_list",
    "agent_move_project",
    "sessions_list",
    "sessions_folder",
    "session_tree",
    "session_messages",
    "session_files_touched",
    "session_delete",
    "git_status",
    "git_branches",
    "git_diff",
    "git_file_lines",
    "git_discard_all",
    "git_discard_files",
    "review_compose",
    "ecosystem_status",
    "ecosystem_releases",
    "dashboard_availability",
    "dashboard_query",
    "extension_action",
    "setup_status",
    "setup_install_dashboard",
    "setup_install_memory",
    "setup_register_mcp",
    "setup_register_memory",
    "sync_export_preview",
    "sync_export",
    "sync_export_save",
    "sync_import_preview",
    "sync_import_apply",
    "update_check",
    "update_install",
    "app_relaunch",
    "launch_sound",
];

fn main() {
    let attributes = Attributes::new().app_manifest(AppManifest::new().commands(COMMANDS));

    if let Err(error) = tauri_build::try_build(attributes) {
        // Build scripts report failures by panicking; `{error:#}` keeps the whole cause chain.
        panic!("tauri build script failed: {error:#}");
    }
}
