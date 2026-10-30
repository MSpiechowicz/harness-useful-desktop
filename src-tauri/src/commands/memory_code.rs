//! The code index of Harness Memory (`memory_code_index_get`, `memory_code_index_set`,
//! `memory_code_builds_listen`, `memory_code_prepare`), through the installed `harness-memory code …`. The switch needs the
//! `code-index` feature of the installed server: `notFound` when Harness Memory is not installed,
//! `unsupported` when it predates it. See `harness_business::ecosystem::memory_code`.

use harness_business::Result;
use harness_business::ecosystem::{
    FeaturesSupport, code_switch_is_on, feature_server, get_code_index, prepare_code_index, set_code_index,
};
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

/// Builds the code index of `project` ahead of the first agent call, so the agent does not wait
/// for it (`code build --project P --json`). The UI calls it when a project opens and does not wait
/// for the answer: a build of a huge repository takes minutes, and its progress comes through
/// `memory_code_builds_listen`. It does nothing when the code index is off, the installed Harness
/// Memory lacks `code-prepare` or is not installed, the project path does not resolve in a few
/// seconds, the project is already waiting for or running its build, too many projects wait, or off
/// Linux. Builds run one at a time: a project opened while another builds waits its turn. A failed build is logged and never an error, because nobody is waiting to read it.
#[tauri::command]
pub async fn memory_code_prepare(state: State<'_, AppState>, project: String) -> CommandResult<()> {
    // The build on open runs only where the worker's memory cap holds: Windows has none and macOS
    // does not reliably enforce it. Elsewhere the index is built on the first code tool call of an
    // agent.
    if !cfg!(target_os = "linux") {
        return Ok(());
    }

    if !code_switch_is_on(&state.env) {
        return Ok(());
    }

    // `/r`, `/r/.` and a symlink to it are one repository: the server is given the real path.
    let Some(project) = state.code_prepares.resolve(project).await else {
        return Ok(());
    };

    let Some(queued) = state.code_prepares.enqueue(project.clone()) else {
        return Ok(());
    };

    let _running = queued.turn().await;

    // The switch may have been turned off while this build waited.
    if !code_switch_is_on(&state.env) {
        return Ok(());
    }

    let Ok(server) = prepare_server(&state).await else {
        return Ok(());
    };

    if let Err(error) = prepare_code_index(&state.env, &state.runner, &server, &project).await {
        tracing::warn!(%error, "code index could not be prepared");
    }

    Ok(())
}

async fn prepare_server(state: &AppState) -> Result<std::path::PathBuf> {
    feature_server(
        &state.env,
        &state.runner,
        &state.memory_features,
        FeaturesSupport::code_prepare,
        "preparing the code index",
    )
    .await
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
