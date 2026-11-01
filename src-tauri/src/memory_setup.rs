//! Project memory: the installed `harness-memory` MCP server (Harness Memory, found once it reports
//! its version), registered in every detected harness's user MCP file at launch and after its
//! install (`setup_install_memory`) while the `memoryEnabled` pref is on, and unregistered when the
//! user turns it off (`memory_set_enabled`). While the server is not installed nothing is
//! registered, and the UI offers its installer. The setup step also registers it in one harness at
//! the user's request (`setup_register_memory`, [`register_in`]). The registration itself is
//! [`crate::sidecar_setup`]'s, run for [`Memory`].
//!
//! Each launch also retires the copy of the bundled server older builds kept
//! (`migrate_legacy_memory`). Logs name the harness and the outcome, never a file's contents.

use std::path::PathBuf;

use harness_business::ecosystem::{mcp_target_paths, migrate_legacy_memory};
use harness_business::model::{HarnessId, McpState};
use tauri::AppHandle;

use crate::error::CommandResult;
use crate::product::Memory;
use crate::sidecar_setup::{self, Registered};
use crate::state::AppState;

/// Registers the server and retires the old copy in the background; the window does not wait for
/// it.
pub fn spawn_on_launch(app: &AppHandle) {
    sidecar_setup::spawn_on_launch::<Memory>(app);
}

/// After the server's installer succeeded (see [`sidecar_setup::after_install`]).
pub(crate) async fn after_install(state: &AppState) -> Registered {
    sidecar_setup::after_install::<Memory>(state).await
}

/// Registers the installed server in the user MCP file of `harness` because the user asked for it
/// there (see [`sidecar_setup::register_in`]).
pub(crate) async fn register_in(state: &AppState, harness: HarnessId) -> CommandResult<McpState> {
    sidecar_setup::register_in::<Memory>(state, harness).await
}

/// Points the owned entries that run the old copy at the installed `server` and deletes the copy
/// once nothing runs it. Checks every harness's user `mcp.json` files, detected or not (the builds
/// that kept the copy registered it only there). The caller holds `state.memory_setup`.
pub(crate) async fn retire_legacy_copy(state: &AppState, server: Option<PathBuf>) {
    let env = state.env.clone();
    let targets: Vec<(PathBuf, Vec<PathBuf>)> = state
        .registry
        .all()
        .filter_map(|harness| mcp_target_paths(&env, harness))
        .collect();

    let migrated =
        tauri::async_runtime::spawn_blocking(move || migrate_legacy_memory(&env, &targets, server.as_deref())).await;

    match migrated {
        Ok(migration) => {
            tracing::info!(
                repointed = migration.repointed,
                copy = ?migration.copy,
                "project memory: old server copy checked"
            );
        }
        Err(error) => tracing::warn!(%error, "project memory migration task failed"),
    }
}
