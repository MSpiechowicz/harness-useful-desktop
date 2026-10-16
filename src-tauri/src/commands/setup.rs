//! The "Recommended setup" onboarding step: what is set up (`setup_status`), the official
//! installers of the dashboard (`setup_install_dashboard`) and of the memory server
//! (`setup_install_memory`), and the dashboard's and the memory server's MCP servers in a harness's
//! user MCP file (`setup_register_mcp`, `setup_register_memory`).
//!
//! The webview sends no command line and no path: the installer is fixed per OS and the MCP file
//! follows from the harness id (its user agent directory, the active OMP profile's when one is
//! set). The UI confirms each action with the user first.

use harness_business::BusinessError;
use harness_business::ecosystem::{
    Companion, mcp_target_paths, register_dashboard_mcp, resolve_dashboard, run_installer, setup_status as status,
};
use harness_business::model::{DashboardInstallResult, HarnessId, InstallLogLine, McpState, RecommendedSetup};
use harness_business::process::{ProcessSpawner, TokioSpawner};
use tauri::State;
use tauri::async_runtime::block_on;
use tauri::ipc::Channel;

use super::blocking;
use super::harness::{find, find_in, installed_binary, summaries};
use crate::error::CommandResult;
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
/// install at a time). Once `ok`, recalls use the installed server, and while project memory is on
/// it is registered in every detected harness (as at launch).
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
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use harness_business::model::IpcErrorCode;
    use harness_business::process::{ExecSpec, SpawnedProcess};

    use super::*;
    use harness_business::ecosystem::{MEMORY_RECORD_FILE, MemoryRecord, MemoryRegistration, RecordedFile};
    use serde_json::json;

    use crate::test_support::{
        TempDir, cache_detection, collecting_channel, home_state, omp_mcp_file, place_companion, place_memory_server,
    };

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
        place_companion(home, "harness-dashboard");
    }

    #[test]
    fn an_unconfirmed_install_does_not_run_the_installer() {
        let dir = TempDir::new("setup-unconfirmed");
        let state = state(&dir);

        for companion in [Companion::Dashboard, Companion::Memory] {
            let spawner = RefusingSpawner::default();
            let (channel, _lines) = collecting_channel();

            let error = block_on(install(&state, &spawner, companion, false, &channel)).expect_err("unconfirmed");

            assert_eq!(error.code(), IpcErrorCode::InvalidInput);
            assert_eq!(spawner.0.load(Ordering::Relaxed), 0, "{companion:?}");
        }
    }

    #[test]
    fn a_second_install_is_refused_while_one_runs() {
        let dir = TempDir::new("setup-concurrent");
        let state = state(&dir);

        for (companion, lock) in [
            (Companion::Dashboard, &state.dashboard_install),
            (Companion::Memory, &state.memory_install),
        ] {
            let spawner = RefusingSpawner::default();
            let (channel, _lines) = collecting_channel();

            let running = lock.try_lock().expect("no install yet");
            let error = block_on(install(&state, &spawner, companion, true, &channel)).expect_err("already installing");
            drop(running);

            assert_eq!(error.code(), IpcErrorCode::InvalidInput);
            assert_eq!(spawner.0.load(Ordering::Relaxed), 0, "{companion:?}");

            // Once the first one is done, the next install runs.
            block_on(install(&state, &spawner, companion, true, &channel)).expect_err("the spawner refuses");
            assert_eq!(spawner.0.load(Ordering::Relaxed), 1, "{companion:?}");
        }
    }

    #[test]
    fn the_dashboard_and_the_memory_server_install_independently() {
        let dir = TempDir::new("setup-independent");
        let state = state(&dir);
        let spawner = RefusingSpawner::default();
        let (channel, _lines) = collecting_channel();

        let dashboard = state.dashboard_install.try_lock().expect("no dashboard install yet");
        block_on(install(&state, &spawner, Companion::Memory, true, &channel)).expect_err("the spawner refuses");
        drop(dashboard);

        assert_eq!(spawner.0.load(Ordering::Relaxed), 1, "the memory installer was started");
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

    fn memory_enabled(state: &AppState) -> bool {
        state.prefs.with(|store| store.load()).memory_enabled
    }

    fn set_memory_enabled(state: &AppState, enabled: bool) {
        state
            .prefs
            .with(|store| store.update(|prefs| prefs.memory_enabled = enabled))
            .expect("pref stored");
    }

    #[test]
    fn memory_registration_needs_the_memory_server_installed() {
        let dir = TempDir::new("setup-memory-missing");
        let state = state(&dir);
        set_memory_enabled(&state, false);
        // A binary that does not report its version does not count either.
        place_companion(&dir.path().join("home"), "harness-memory");

        let error = block_on(register_memory(&state, HarnessId::Omp)).expect_err("not installed");

        assert_eq!(error.code(), IpcErrorCode::InvalidInput);
        assert!(!omp_mcp_file(&state).exists());
        assert!(!memory_enabled(&state), "the pref is left alone");
    }

    #[test]
    fn memory_registration_needs_the_harness_installed() {
        let dir = TempDir::new("setup-memory-harness");
        let state = state(&dir);
        cache_detection(&state, HarnessId::Omp, None);
        if place_memory_server(&dir.path().join("home")).is_none() {
            return;
        }

        let error = block_on(register_memory(&state, HarnessId::Omp)).expect_err("omp not installed");

        assert_eq!(error.code(), IpcErrorCode::NotFound);
        assert!(!omp_mcp_file(&state).exists());
    }

    #[test]
    fn memory_registration_adds_the_entry_turns_memory_on_and_records_the_file() {
        let dir = TempDir::new("setup-memory-register");
        let state = state(&dir);
        let Some(server) = place_memory_server(&dir.path().join("home")) else {
            return;
        };
        set_memory_enabled(&state, false);

        // The app registered it here before and the user removed it: an explicit request adds it back.
        let path = omp_mcp_file(&state);
        std::fs::create_dir_all(path.parent().expect("agent dir")).expect("agent dir");
        std::fs::write(&path, r#"{"mcpServers":{"other":{"command":"x"}}}"#).expect("write mcp.json");
        let mut record = MemoryRecord::load(&state.config_dir);
        record.note(&state.env, HarnessId::Omp, &path, MemoryRegistration::Added);
        record.save().expect("record saved");

        let registered = block_on(register_memory(&state, HarnessId::Omp)).expect("registered");

        assert_eq!(
            registered,
            McpState::Configured {
                file: "~/.omp/agent/mcp.json".to_owned(),
                name: "harness-memory".to_owned()
            }
        );
        let document: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("mcp.json")).expect("valid JSON");
        assert_eq!(
            document["mcpServers"],
            json!({
                "other": { "command": "x" },
                "harness-memory": { "command": server.to_string_lossy(), "args": ["mcp"] }
            })
        );
        assert!(memory_enabled(&state), "project memory was turned on");
        assert!(state.config_dir.join(MEMORY_RECORD_FILE).exists());
        assert_eq!(
            MemoryRecord::load(&state.config_dir).registered(),
            [RecordedFile {
                harness: HarnessId::Omp,
                file: "~/.omp/agent/mcp.json".to_owned()
            }]
        );

        // A second request changes nothing.
        let before = std::fs::read(&path).expect("mcp.json");
        assert!(matches!(
            block_on(register_memory(&state, HarnessId::Omp)).expect("registered"),
            McpState::Configured { .. }
        ));
        assert_eq!(std::fs::read(&path).expect("mcp.json"), before);
    }

    #[test]
    fn memory_registration_into_a_file_it_cannot_change_is_refused() {
        let dir = TempDir::new("setup-memory-refused");
        let state = state(&dir);
        if place_memory_server(&dir.path().join("home")).is_none() {
            return;
        }

        let path = omp_mcp_file(&state);
        std::fs::create_dir_all(path.parent().expect("agent dir")).expect("agent dir");
        std::fs::write(&path, "{ not json").expect("write mcp.json");

        let registered = block_on(register_memory(&state, HarnessId::Omp)).expect("answered");

        assert!(
            matches!(registered, McpState::Refused { ref file, .. } if file == "~/.omp/agent/mcp.json"),
            "{registered:?}"
        );
        assert_eq!(std::fs::read_to_string(&path).expect("mcp.json"), "{ not json");
        assert!(!state.config_dir.join(MEMORY_RECORD_FILE).exists(), "nothing recorded");
    }
}
