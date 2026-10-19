//! The team database of Harness Memory (`memory_remote_get`, `memory_remote_set`,
//! `memory_remote_test`), kept in the server's own `config.toml`. Whether the `scripts` routing
//! and `trust_team_scripts` may be saved comes from the installed server's tools (`script_list`,
//! and a `script_run` without `sha`), probed once per binary; whether `hooks` and
//! `trust_team_hooks` may be saved from `harness-memory hooks config`, whether `routines` and
//! `trust_team_routines` from `harness-memory routines config`, whether Amazon DocumentDB may be
//! saved and tested from `harness-memory kinds`, whether a note scope or the scripts may be routed
//! `remoteLocal` from `harness-memory features`. A save applies the team hooks and
//! team routines settings at once: sharing turned off removes the team copies before it returns.
//!
//! The URL and the token the webview sends are secrets: they are validated, written to that file or
//! handed to `harness-memory check` through its environment, and never logged or returned. The app
//! itself does not connect to the database.

use harness_business::ecosystem::{KindsSupport, MemoryRemoteChange, MemorySupport};
use harness_business::model::{MemoryNotesRouting, MemoryRemote, MemoryRemoteKind, MemoryRemoteTest};
use tauri::State;
use tauri::async_runtime::block_on;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

/// What the config file holds: the redacted URL, whether a token is saved, the note and script
/// routing, whether the server keeps scripts, and what is wrong with the file, if anything.
#[tauri::command]
pub async fn memory_remote_get(state: State<'_, AppState>) -> CommandResult<MemoryRemote> {
    get(&state).await
}

pub(crate) async fn get(state: &AppState) -> CommandResult<MemoryRemote> {
    let env = state.env.clone();
    let support = support(state).await;

    blocking(move || Ok(harness_business::ecosystem::memory_remote_get(&env, support))).await
}

/// What the installed server supports: scripts (its tools), team hooks (`hooks config`), team
/// routines (`routines config`), database kinds (`kinds`) and features (`features`).
async fn support(state: &AppState) -> MemorySupport {
    MemorySupport {
        scripts: state.memory_scripts.support(&state.env).await,
        hooks: state.hooks.team_support().await,
        routines: state.routines.team().support(&state.env).await,
        kinds: state.memory_kinds.support(&state.env, &state.runner).await,
        features: state.memory_features.support(&state.env, &state.runner).await,
    }
}

/// Saves the team database (`enabled`), or removes it from the config file. A null `url` or `token`
/// keeps the saved one, an empty `token` removes it. `invalidInput` for a URL or token Harness Memory
/// would refuse, and for a file the app must not overwrite (see `problem` in the result of
/// [`memory_remote_get`]). `scripts` is saved only as `scripts = "local"` or `"remote+local"`, and
/// only while the installed server keeps scripts. `trust_team_scripts` is saved only as `true`, and only while the
/// installed server asks before running a script. `shareHooks` (`hooks = "remote"`) and
/// `trustTeamHooks` (only while hooks are shared) are saved only while the installed server keeps
/// team hooks; null keeps the saved choice. `shareRoutines` (`routines = "remote"`) and
/// `trustTeamRoutines` (only while routines are shared) likewise, while the server keeps team
/// routines. When the server cannot be asked, the saved keys are kept as they are. `kind`
/// `documentdb` is saved as `database_kind = "documentdb"` and refused (`unsupported`) unless the
/// installed server lists it in `harness-memory kinds`. `mongodb` at a host the server would take
/// for Amazon DocumentDB is saved as `database_kind = "mongodb"` while the server lists it.
/// `repositoryNotes` or `globalNotes` `remoteLocal` is refused (`unsupported`) unless the installed
/// server lists `notes-remote-local`, `notes-hide` and `push-json` in `harness-memory features`,
/// `scripts` `remoteLocal` unless it lists `scripts-remote-local`, `scripts-hide` and
/// `scripts-push-json`.
#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "IPC arguments arrive one per name; grouping them would change the command's contract"
)]
pub async fn memory_remote_set(
    state: State<'_, AppState>,
    enabled: bool,
    kind: Option<MemoryRemoteKind>,
    url: Option<String>,
    token: Option<String>,
    repository_notes: MemoryNotesRouting,
    global_notes: MemoryNotesRouting,
    scripts: MemoryNotesRouting,
    trust_team_scripts: bool,
    share_hooks: Option<bool>,
    trust_team_hooks: Option<bool>,
    share_routines: Option<bool>,
    trust_team_routines: Option<bool>,
) -> CommandResult<MemoryRemote> {
    let change = MemoryRemoteChange {
        enabled,
        kind,
        url,
        token,
        repository_notes,
        global_notes,
        scripts,
        trust_team_scripts,
        share_hooks,
        trust_team_hooks,
        share_routines,
        trust_team_routines,
    };

    set(&state, change).await
}

/// Saves `change` (see [`memory_remote_set`]), then applies the team hooks and team routines
/// settings: sharing turned off removes the team copies before this returns.
pub(crate) async fn set(state: &AppState, change: MemoryRemoteChange) -> CommandResult<MemoryRemote> {
    let env = state.env.clone();
    let support = support(state).await;

    let saved = blocking(move || Ok(harness_business::ecosystem::memory_remote_set(&env, change, support)?)).await?;

    if state.hooks.team_reprobe().await {
        state.hooks.refresh_team_soon();
    }

    if crate::team_sync::reprobe(&*state.routines).await {
        state.routines.team().refresh_soon();
    }

    Ok(saved)
}

/// Runs `harness-memory check` (20 s limit) against the given URL and token, or the saved ones where
/// they are null. `ok` false with the reason when the server is not installed, the input is not
/// valid, or the database cannot be reached, and for `documentdb` when the installed server does not
/// list it in `harness-memory kinds` (asked for `documentdb` only).
#[tauri::command]
pub async fn memory_remote_test(
    state: State<'_, AppState>,
    kind: MemoryRemoteKind,
    url: Option<String>,
    token: Option<String>,
) -> CommandResult<MemoryRemoteTest> {
    let env = state.env.clone();
    let runner = state.runner;

    // Only Amazon DocumentDB depends on the server's kinds: other tests do not ask.
    let kinds = match kind {
        MemoryRemoteKind::Documentdb => state.memory_kinds.support(&state.env, &state.runner).await,
        _ => KindsSupport::Unknown,
    };

    blocking(move || {
        Ok(block_on(harness_business::ecosystem::memory_remote_test(
            &env, &runner, kind, kinds, url, token,
        )))
    })
    .await
}
