//! IPC commands. Each is a thin adapter over `harness_business`; names are the C0–C4 contract in
//! `ui/src/lib/ipc/commands.ts` and must also be listed in `build.rs` and `capabilities/main.json`.

pub mod agent;
pub mod app;
pub mod desktop_mcp;
pub mod drops;
pub mod ecosystem;
pub mod git;
pub mod harness;
pub mod hooks;
pub mod memory;
pub mod memory_notes;
pub mod memory_notes_import;
pub mod memory_notes_template;
pub mod memory_remote;
pub mod memory_requests;
pub mod memory_scripts;
pub mod plugins;
pub mod projects;
pub mod review;
pub mod routines;
pub mod sessions;
pub mod setup;
pub mod sound;
pub mod sync;
pub mod system;
pub mod updates;

use std::path::PathBuf;

use harness_business::project::Projects;

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// Runs blocking work (file IO, process spawns) off the async runtime's worker threads.
async fn blocking<T, F>(task: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CommandResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| CommandError::internal(format!("background task failed: {error}")))?
}

/// The canonical directory of `project`, which must be the active project, a recent one, the
/// routines workspace or the folder of one global routine in it, where global routines run
/// (`forbidden` otherwise, with the generic message).
async fn authorized_dir(state: &AppState, project: String) -> CommandResult<PathBuf> {
    let routines = state.routines.clone();
    let asked = project.clone();

    if let Some(folder) = blocking(move || Ok(routines.workspace_folder(&asked))).await? {
        return Ok(folder);
    }

    let prefs = state.prefs.clone();
    let env = state.env.clone();

    let authorized =
        blocking(move || Ok(prefs.with(|store| Projects::new(store, &env).authorize(Some(&project)))?)).await?;

    authorized.ok_or_else(|| CommandError::internal("an authorized project has no path"))
}

/// The saved UI language, for the text the Rust side shows (native dialog titles).
async fn saved_language(state: &AppState) -> CommandResult<String> {
    let prefs = state.prefs.clone();

    blocking(move || Ok(prefs.with(|store| store.load()).language)).await
}
