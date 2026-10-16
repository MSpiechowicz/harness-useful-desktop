//! The team database of Harness Memory (`memory_remote_get`, `memory_remote_set`,
//! `memory_remote_test`), kept in the server's own `config.toml`.
//!
//! The URL and the token the webview sends are secrets: they are validated, written to that file or
//! handed to `harness-memory check` through its environment, and never logged or returned. The app
//! itself does not connect to the database.

use harness_business::ecosystem::{MemoryRemoteChange, memory_remote_get as get, memory_remote_set as set};
use harness_business::model::{MemoryNotesTarget, MemoryRemote, MemoryRemoteKind, MemoryRemoteTest};
use tauri::State;
use tauri::async_runtime::block_on;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

/// What the config file holds: the redacted URL, whether a token is saved, the note routing, and
/// what is wrong with the file, if anything.
#[tauri::command]
pub async fn memory_remote_get(state: State<'_, AppState>) -> CommandResult<MemoryRemote> {
    let env = state.env.clone();

    blocking(move || Ok(get(&env))).await
}

/// Saves the team database (`enabled`), or removes it from the config file. A null `url` or `token`
/// keeps the saved one, an empty `token` removes it. `invalidInput` for a URL or token Harness Memory
/// would refuse, and for a file the app must not overwrite (see `problem` in the result of
/// [`memory_remote_get`]).
#[tauri::command]
pub async fn memory_remote_set(
    state: State<'_, AppState>,
    enabled: bool,
    kind: Option<MemoryRemoteKind>,
    url: Option<String>,
    token: Option<String>,
    repository_notes: MemoryNotesTarget,
    global_notes: MemoryNotesTarget,
) -> CommandResult<MemoryRemote> {
    let env = state.env.clone();
    let change = MemoryRemoteChange {
        enabled,
        kind,
        url,
        token,
        repository_notes,
        global_notes,
    };

    blocking(move || Ok(set(&env, change)?)).await
}

/// Runs `harness-memory check` (20 s limit) against the given URL and token, or the saved ones where
/// they are null. `ok` false with the reason when the server is not installed, the input is not
/// valid, or the database cannot be reached.
#[tauri::command]
pub async fn memory_remote_test(
    state: State<'_, AppState>,
    kind: MemoryRemoteKind,
    url: Option<String>,
    token: Option<String>,
) -> CommandResult<MemoryRemoteTest> {
    let env = state.env.clone();
    let runner = state.runner;

    blocking(move || {
        Ok(block_on(harness_business::ecosystem::memory_remote_test(
            &env, &runner, kind, url, token,
        )))
    })
    .await
}
