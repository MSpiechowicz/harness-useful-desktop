//! The "Recommended setup" onboarding step: what is set up (`setup_status`), the official
//! installers of the dashboard (`setup_install_dashboard`) and of the memory server
//! (`setup_install_memory`), and the dashboard's and the memory server's MCP servers in a harness's
//! user MCP file (`setup_register_mcp`, `setup_register_memory`).
//!
//! The webview sends no command line and no path: the installer is fixed per OS and the MCP file
//! follows from the harness id (its user agent directory, the active OMP profile's when one is
//! set, or OpenCode's global config file). The UI confirms each action with the user first.

use harness_business::BusinessError;
use harness_business::ecosystem::{
    Companion, mcp_target, register_dashboard_mcp, resolve_dashboard, run_installer, setup_status as status,
};
use harness_business::model::{DashboardInstallResult, HarnessId, InstallLogLine, McpState, RecommendedSetup};
use harness_business::process::{ProcessSpawner, TokioSpawner};
use tauri::State;
use tauri::async_runtime::block_on;
use tauri::ipc::Channel;

use super::blocking;
use super::harness::{find, find_in, installed_binary, summaries};
use crate::error::CommandResult;
use crate::hooks::after_member_install;
use crate::memory_setup::{after_install, register_in};
use crate::state::AppState;

/// The dashboard and memory binaries, and per harness its skills and the dashboard's MCP entry. Read-only;
/// detects the harnesses first when they have not been yet.
#[tauri::command]
pub async fn setup_status(state: State<'_, AppState>) -> CommandResult<RecommendedSetup> {
    let summaries = summaries(&state).await?;
    let env = state.env.clone();
    let runner = state.runner;
    let registry = state.registry.clone();

    // Reads the plugin registries and MCP files besides probing the binaries' versions.
    blocking(move || Ok(block_on(status(&env, &runner, &registry, &summaries)))).await
}

/// Runs the dashboard's installer, streaming its output to `on_log`. `confirmed` must be true (the
/// UI asks the user first; `invalidInput` otherwise), and only one install runs at a time
/// (`invalidInput` while one does). Success is `ok` in the result, never the exit code.
#[tauri::command]
pub async fn setup_install_dashboard(
    state: State<'_, AppState>,
    confirmed: bool,
    on_log: Channel<InstallLogLine>,
) -> CommandResult<DashboardInstallResult> {
    install(&state, &TokioSpawner, Companion::Dashboard, confirmed, &on_log).await
}

/// Runs the memory server's installer like [`setup_install_dashboard`] (its own lock: one memory
/// install at a time). Once `ok`, while project memory is on, the installed server is registered
/// in every detected harness (as at launch).
#[tauri::command]
pub async fn setup_install_memory(
    state: State<'_, AppState>,
    confirmed: bool,
    on_log: Channel<InstallLogLine>,
) -> CommandResult<DashboardInstallResult> {
    let result = install(&state, &TokioSpawner, Companion::Memory, confirmed, &on_log).await?;

    if result.ok {
        after_install(&state).await;
    }

    Ok(result)
}

async fn install(
    state: &AppState,
    spawner: &dyn ProcessSpawner,
    companion: Companion,
    confirmed: bool,
    on_log: &Channel<InstallLogLine>,
) -> CommandResult<DashboardInstallResult> {
    if !confirmed {
        return Err(BusinessError::InvalidInput("the user has not confirmed the install".to_owned()).into());
    }

    let lock = match companion {
        Companion::Dashboard => &state.dashboard_install,
        Companion::Memory => &state.memory_install,
    };
    let Ok(_installing) = lock.try_lock() else {
        return Err(BusinessError::InvalidInput(format!("{} is already being installed", companion.binary())).into());
    };

    tracing::info!(?companion, "installer started");

    let mut lines = 0_usize;
    let result = run_installer(&state.env, companion, spawner, |line| {
        lines += 1;
        // A closed channel only means the page went away; the install goes on.
        on_log.send(line).ok();
    })
    .await?;

    tracing::info!(
        ok = result.ok,
        exit_code = ?result.exit_code,
        timed_out = result.timed_out,
        truncated = result.truncated,
        lines,
        ?companion,
        "installer finished"
    );

    // Its Harness family hooks come back, also ones the user deleted before.
    if result.ok {
        after_member_install(&state.hooks, companion).await;
    }

    Ok(result)
}

/// Adds the dashboard's MCP server to the user MCP file of `harness` unless it is there, and
/// returns the file's state (`refused`, nothing written, when the file cannot be changed safely).
/// `notFound` when the harness or the dashboard is not installed; `unsupported` when the harness
/// has no user MCP file.
#[tauri::command]
pub async fn setup_register_mcp(state: State<'_, AppState>, harness: HarnessId) -> CommandResult<McpState> {
    register(&state, harness).await
}

async fn register(state: &AppState, harness: HarnessId) -> CommandResult<McpState> {
    find(state, harness)?;
    installed_binary(state, harness).await?;

    let env = state.env.clone();
    let registry = state.registry.clone();

    let registered = blocking(move || {
        if resolve_dashboard(&env).is_none() {
            return Err(BusinessError::NotFound("harness-dashboard is not installed".to_owned()).into());
        }

        let found = find_in(&registry, harness)?;
        let Some(file) = mcp_target(&env, found) else {
            return Err(BusinessError::Unsupported(format!("{harness:?} has no user MCP file")).into());
        };

        Ok(register_dashboard_mcp(&env, &file))
    })
    .await?;

    tracing::info!(
        ?harness,
        outcome = outcome(&registered),
        "dashboard MCP registration finished"
    );

    Ok(registered)
}

/// Adds the memory server's MCP entry to the user MCP file of `harness` (also where the user removed
/// it before) and returns the file's state (`refused`, nothing written, when the file cannot be
/// changed safely). Turns project memory on when it is off. `notFound` when the harness is not
/// installed; `invalidInput` when Harness Memory is not installed; `unsupported` when the harness
/// has no user MCP file.
#[tauri::command]
pub async fn setup_register_memory(state: State<'_, AppState>, harness: HarnessId) -> CommandResult<McpState> {
    register_memory(&state, harness).await
}

async fn register_memory(state: &AppState, harness: HarnessId) -> CommandResult<McpState> {
    find(state, harness)?;
    installed_binary(state, harness).await?;

    let registered = register_in(state, harness).await?;

    tracing::info!(
        ?harness,
        outcome = outcome(&registered),
        "memory MCP registration finished"
    );

    Ok(registered)
}

/// The kind of `state`, for the log (without the file).
fn outcome(state: &McpState) -> &'static str {
    match state {
        McpState::Configured { .. } => "configured",
        McpState::Missing { .. } => "missing",
        McpState::Disabled { .. } => "disabled",
        McpState::Refused { .. } => "refused",
        McpState::ManualEdit { .. } => "manualEdit",
    }
}

#[cfg(test)]
mod tests;
