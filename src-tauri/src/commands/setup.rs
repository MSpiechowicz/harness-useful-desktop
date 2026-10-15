//! The "Recommended setup" onboarding step: what is set up (`setup_status`), the dashboard's
//! official installer (`setup_install_dashboard`) and its MCP server in a harness's user MCP file
//! (`setup_register_mcp`).
//!
//! The webview sends no command line and no path: the installer is fixed per OS and the MCP file
//! follows from the harness id (its user agent directory, the active OMP profile's when one is
//! set). The UI confirms each action with the user first.

use harness_business::BusinessError;
use harness_business::ecosystem::{
    mcp_target_paths, register_dashboard_mcp, resolve_dashboard, run_dashboard_installer, setup_status as status,
};
use harness_business::model::{DashboardInstallResult, HarnessId, InstallLogLine, McpState, RecommendedSetup};
use harness_business::process::{ProcessSpawner, TokioSpawner};
use tauri::State;
use tauri::async_runtime::block_on;
use tauri::ipc::Channel;

use super::blocking;
use super::harness::{find, find_in, installed_binary, summaries};
use crate::error::CommandResult;
use crate::state::AppState;

/// The dashboard binary, and per harness its skills and the dashboard's MCP entry. Read-only;
/// detects the harnesses first when they have not been yet.
#[tauri::command]
pub async fn setup_status(state: State<'_, AppState>) -> CommandResult<RecommendedSetup> {
    let summaries = summaries(&state).await?;
    let env = state.env.clone();
    let runner = state.runner;
    let registry = state.registry.clone();

    // Reads the plugin registries and MCP files besides probing the dashboard's version.
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
    install(&state, &TokioSpawner, confirmed, &on_log).await
}

async fn install(
    state: &AppState,
    spawner: &dyn ProcessSpawner,
    confirmed: bool,
    on_log: &Channel<InstallLogLine>,
) -> CommandResult<DashboardInstallResult> {
    if !confirmed {
        return Err(BusinessError::InvalidInput("the user has not confirmed the install".to_owned()).into());
    }

    let Ok(_installing) = state.dashboard_install.try_lock() else {
        return Err(BusinessError::InvalidInput("the dashboard is already being installed".to_owned()).into());
    };

    tracing::info!("dashboard installer started");

    let mut lines = 0_usize;
    let result = run_dashboard_installer(&state.env, spawner, |line| {
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
        "dashboard installer finished"
    );

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
        let Some((path, also_read)) = mcp_target_paths(&env, found) else {
            return Err(BusinessError::Unsupported(format!("{harness:?} has no user MCP file")).into());
        };

        Ok(register_dashboard_mcp(&env, &path, &also_read))
    })
    .await?;

    tracing::info!(
        ?harness,
        outcome = outcome(&registered),
        "dashboard MCP registration finished"
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
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use harness_business::model::IpcErrorCode;
    use harness_business::process::{ExecSpec, SpawnedProcess};

    use super::*;
    use crate::test_support::{TempDir, cache_detection, collecting_channel, home_state, omp_mcp_file};

    /// Counts spawn attempts and starts nothing.
    #[derive(Default)]
    struct RefusingSpawner(AtomicUsize);

    impl ProcessSpawner for RefusingSpawner {
        fn spawn(&self, _spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
            self.0.fetch_add(1, Ordering::Relaxed);

            Err(BusinessError::Process("spawning is disabled in tests".to_owned()))
        }
    }

    /// A state over `<dir>/home` with OMP detected at a binary that must never run.
    fn state(dir: &TempDir) -> AppState {
        let state = home_state(dir);
        cache_detection(&state, HarnessId::Omp, Some(&dir.path().join("missing-omp")));

        state
    }

    /// Places an executable `harness-dashboard` in `~/.local/bin`, where it is looked up.
    fn install_dashboard(home: &Path) {
        let bin = home.join(".local/bin");
        std::fs::create_dir_all(&bin).expect("create bin");

        let binary = bin.join(format!("harness-dashboard{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&binary, "").expect("write binary");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).expect("chmod binary");
        }
    }

    #[test]
    fn an_unconfirmed_install_does_not_run_the_installer() {
        let dir = TempDir::new("setup-unconfirmed");
        let state = state(&dir);
        let spawner = RefusingSpawner::default();
        let (channel, _lines) = collecting_channel();

        let error = block_on(install(&state, &spawner, false, &channel)).expect_err("unconfirmed");

        assert_eq!(error.code(), IpcErrorCode::InvalidInput);
        assert_eq!(spawner.0.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn a_second_install_is_refused_while_one_runs() {
        let dir = TempDir::new("setup-concurrent");
        let state = state(&dir);
        let spawner = RefusingSpawner::default();
        let (channel, _lines) = collecting_channel();

        let running = state.dashboard_install.try_lock().expect("no install yet");
        let error = block_on(install(&state, &spawner, true, &channel)).expect_err("already installing");
        drop(running);

        assert_eq!(error.code(), IpcErrorCode::InvalidInput);
        assert_eq!(spawner.0.load(Ordering::Relaxed), 0);

        // Once the first one is done, the next install runs.
        block_on(install(&state, &spawner, true, &channel)).expect_err("the spawner refuses");
        assert_eq!(spawner.0.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn mcp_registration_needs_the_harness_installed() {
        let dir = TempDir::new("setup-mcp-harness");
        let state = state(&dir);
        install_dashboard(&dir.path().join("home"));
        cache_detection(&state, HarnessId::Omp, None);

        let error = block_on(register(&state, HarnessId::Omp)).expect_err("omp not installed");

        assert_eq!(error.code(), IpcErrorCode::NotFound);
        assert!(!omp_mcp_file(&state).exists());
    }

    #[test]
    fn mcp_registration_needs_the_dashboard_installed() {
        let dir = TempDir::new("setup-mcp-dashboard");
        let state = state(&dir);

        let error = block_on(register(&state, HarnessId::Omp)).expect_err("no dashboard");

        assert_eq!(error.code(), IpcErrorCode::NotFound);
        assert!(!omp_mcp_file(&state).exists());
    }

    #[test]
    fn mcp_registration_writes_the_dashboard_server_into_the_user_mcp_file() {
        let dir = TempDir::new("setup-mcp-register");
        let state = state(&dir);
        install_dashboard(&dir.path().join("home"));

        let registered = block_on(register(&state, HarnessId::Omp)).expect("registered");

        assert!(matches!(registered, McpState::Configured { .. }), "{registered:?}");
        let written = std::fs::read_to_string(omp_mcp_file(&state)).expect("mcp.json written");
        let document: serde_json::Value = serde_json::from_str(&written).expect("valid JSON");
        assert_eq!(
            document["mcpServers"]["harness-dashboard"],
            serde_json::json!({ "command": "harness-dashboard", "args": ["mcp"] })
        );
    }
}
