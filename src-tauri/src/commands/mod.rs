//! IPC commands. Each is a thin adapter over `harness_business`; names are the C0/M1 contract in
//! `ui/src/lib/ipc/commands.ts` and must also be listed in `build.rs` and `capabilities/main.json`.

pub mod agent;
pub mod app;
pub mod git;
pub mod harness;
pub mod projects;
pub mod review;
pub mod sessions;
pub mod system;

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

/// The canonical directory of `project`, which must be the active project or a recent one
/// (`forbidden` otherwise, with the generic message).
async fn authorized_dir(state: &AppState, project: String) -> CommandResult<PathBuf> {
    let prefs = state.prefs.clone();
    let env = state.env.clone();

    let authorized =
        blocking(move || Ok(prefs.with(|store| Projects::new(store, &env).authorize(Some(&project)))?)).await?;

    authorized.ok_or_else(|| CommandError::internal("an authorized project has no path"))
}
