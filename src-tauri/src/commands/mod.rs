//! IPC commands. Each is a thin adapter over `harness_business`; names are the C0/M1 contract in
//! `ui/src/lib/ipc/commands.ts` and must also be listed in `build.rs` and `capabilities/main.json`.

pub mod agent;
pub mod app;
pub mod harness;
pub mod projects;
pub mod system;

use crate::error::{CommandError, CommandResult};

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
