//! The code index of Harness Memory (`memory_code_index_get`, `memory_code_index_set`,
//! `memory_code_builds_listen`), through the installed `harness-memory code …`. The switch needs the
//! `code-index` feature of the installed server: `notFound` when Harness Memory is not installed,
//! `unsupported` when it predates it. See `harness_business::ecosystem::memory_code`.

use harness_business::Result;
use harness_business::ecosystem::{FeaturesSupport, feature_server, get_code_index, set_code_index};
use harness_business::model::{CodeBuildEvent, CodeIndexState};
use tauri::State;
use tauri::ipc::Channel;

use crate::error::CommandResult;
use crate::hooks::seed_available;
use crate::state::AppState;

/// Whether the code index is on (`code status --json`) and where the switch comes from
/// (`default`, `file` or `env`). `unsupported` for a Harness Memory without `code-index`, `notFound`
/// when it is not installed.
#[tauri::command]
pub async fn memory_code_index_get(state: State<'_, AppState>) -> CommandResult<CodeIndexState> {
    let server = code_server(&state).await?;

    Ok(get_code_index(&state.env, &state.runner, &server).await?)
}

/// Turns the code index on or off (`code enable|disable --json`; off hides the `code_*` tools and
/// keeps the index) and answers the state as the server reports it. Errors as
/// `memory_code_index_get`, `process` with the server's line when it cannot write the choice.
#[tauri::command]
pub async fn memory_code_index_set(state: State<'_, AppState>, on: bool) -> CommandResult<CodeIndexState> {
    let server = code_server(&state).await?;
    let saved = set_code_index(&state.env, &state.runner, &server, on).await?;

    // "Use the code graph first" runs or pauses from now on, and is seeded when it never was.
    state.hooks.refresh_members();
    if saved.enabled {
        seed_available(&state.hooks).await;
    }

    Ok(saved)
}

/// Sends the code graph builds to `on_event` (one event when a build begins, moves into another
/// 10 % step of its files or ends), replacing the previous listener; the builds still in progress
/// are sent at once. `phase` is `building`, `ready`, `error`, `stale` or `cancelled` (the last two
/// mean: dismiss it).
#[tauri::command]
pub async fn memory_code_builds_listen(
    state: State<'_, AppState>,
    on_event: Channel<CodeBuildEvent>,
) -> CommandResult<()> {
    state.code_builds.listen(on_event);

    Ok(())
}

async fn code_server(state: &AppState) -> Result<std::path::PathBuf> {
    feature_server(
        &state.env,
        &state.runner,
        &state.memory_features,
        FeaturesSupport::code_index,
        "the code index",
    )
    .await
}
