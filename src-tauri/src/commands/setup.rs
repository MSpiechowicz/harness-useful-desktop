//! The "Recommended setup" onboarding step: what is set up (`setup_status`), the official
//! installers of the dashboard (`setup_install_dashboard`), of the memory server
//! (`setup_install_memory`) and of the Harness Skills for OpenCode
//! and Codex (`setup_install_opencode_skills`, `setup_install_codex_skills`), and the dashboard's and the memory server's MCP servers in a
//! harness's user MCP file (`setup_register_mcp`, `setup_register_memory`).
//!
//! The webview sends no command line and no path: the installer is fixed per OS and the MCP file
//! follows from the harness id (its user agent directory, the active OMP profile's when one is
//! set, OpenCode's global config file, or Claude Code's user scope, changed through its CLI). The
//! UI confirms each action with the user first.

use harness_business::BusinessError;
use harness_business::ecosystem::{
    Companion, mcp_target, register_dashboard_mcp, register_dashboard_mcp_cli, resolve_dashboard, run_installer,
    setup_status as status,
};
use harness_business::model::{DashboardInstallResult, HarnessId, InstallLogLine, McpState, RecommendedSetup};
use harness_business::process::{ProcessSpawner, TokioSpawner};
use tauri::State;
use tauri::async_runtime::block_on;
use tauri::ipc::Channel;

use super::blocking;
use super::harness::{ensure_detected, find, find_in, installed_binary, summaries};
use crate::error::CommandResult;
use crate::hooks::after_member_install;
use crate::mcp_cli::CliContext;
use crate::memory_embedder::reconcile;
use crate::memory_setup::{after_install, register_in};
use crate::state::AppState;

/// The dashboard and memory binaries, and per harness its skills and its MCP entries. Read-only;
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
        reconcile(&state).await;
    }

    Ok(result)
}

/// Runs the Harness Skills' OpenCode installer like [`setup_install_dashboard`] (its own lock: one
/// skills install at a time). `ok` once their install record holds a version; OpenCode and its
/// running chats load them after a restart. OpenCode is detected first, so its generation is known
/// (see `ensure_detected`).
#[tauri::command]
pub async fn setup_install_opencode_skills(
    state: State<'_, AppState>,
    confirmed: bool,
    on_log: Channel<InstallLogLine>,
) -> CommandResult<DashboardInstallResult> {
    ensure_detected(&state, HarnessId::OpenCode).await?;

    install(&state, &TokioSpawner, Companion::OpenCodeSkills, confirmed, &on_log).await
}

/// Runs the Harness Skills' Codex installer like [`setup_install_opencode_skills`] (its own lock).
/// `ok` once their install record holds a version; Codex and its running chats load them after a
/// restart.
#[tauri::command]
pub async fn setup_install_codex_skills(
    state: State<'_, AppState>,
    confirmed: bool,
    on_log: Channel<InstallLogLine>,
) -> CommandResult<DashboardInstallResult> {
    install(&state, &TokioSpawner, Companion::CodexSkills, confirmed, &on_log).await
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

    // A closed channel only means the page went away; the install goes on.
    install_streaming(state, spawner, companion, |line| {
        on_log.send(line).ok();
    })
    .await
}

/// Runs the installer of `companion` under its lock (`invalidInput` while it is already running),
/// handing each output line to `on_line`. The caller has the user's consent: the command's
/// `confirmed`, or for the launch migration of the Harness Skills names the install that already
/// exists.
pub(crate) async fn install_streaming(
    state: &AppState,
    spawner: &dyn ProcessSpawner,
    companion: Companion,
    mut on_line: impl FnMut(InstallLogLine),
) -> CommandResult<DashboardInstallResult> {
    let lock = match companion {
        Companion::Dashboard => &state.dashboard_install,
        Companion::Memory => &state.memory_install,
        Companion::OpenCodeSkills => &state.opencode_skills_install,
        Companion::CodexSkills => &state.codex_skills_install,
    };
    let Ok(_installing) = lock.try_lock() else {
        return Err(BusinessError::InvalidInput(format!("{} is already being installed", companion.name())).into());
    };

    tracing::info!(?companion, "installer started");

    let mut lines = 0_usize;
    let result = run_installer(&state.env, companion, spawner, |line| {
        lines += 1;
        on_line(line);
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
    let binary = installed_binary(state, harness).await?;

    let env = state.env.clone();
    let registry = state.registry.clone();
    let context = CliContext::of(state);

    let registered = blocking(move || {
        if resolve_dashboard(&env).is_none() {
            return Err(BusinessError::NotFound("harness-dashboard is not installed".to_owned()).into());
        }

        let found = find_in(&registry, harness)?;
        let Some(file) = mcp_target(&env, found) else {
            return Err(BusinessError::Unsupported(format!("{harness:?} has no user MCP file")).into());
        };

        match context.cli_for(harness, &file, Some(&binary)) {
            Some(cli) => Ok(block_on(register_dashboard_mcp_cli(cli, &file))),
            None => Ok(register_dashboard_mcp(&env, &file)),
        }
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
