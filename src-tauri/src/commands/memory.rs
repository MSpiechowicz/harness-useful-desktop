//! The "Project memory" switch (`memory_set_enabled`): stores the `memoryEnabled` pref and applies
//! it to the harnesses' user MCP files right away (see `memory_setup`).

use std::path::Path;

use harness_business::model::AppPrefs;
use tauri::State;

use super::blocking;
use crate::error::CommandResult;
use crate::memory_setup::{bundled_server, install_and_register, unregister_all};
use crate::state::AppState;

/// Turns project memory on (installs the server and registers it in every detected harness, also
/// where the user removed it before) or off (removes the app's own entries). Returns the stored
/// prefs. A failure to change an MCP file is logged; the pref is stored regardless.
#[tauri::command]
pub async fn memory_set_enabled(state: State<'_, AppState>, enabled: bool) -> CommandResult<AppPrefs> {
    set_enabled(&state, enabled, bundled_server().as_deref()).await
}

/// `server` is the bundled server; `None` when the executable's folder is unknown, and then only the
/// pref is stored.
async fn set_enabled(state: &AppState, enabled: bool, server: Option<&Path>) -> CommandResult<AppPrefs> {
    // Held from storing the pref to applying it, so quick toggles and the launch run apply in order.
    let _running = state.memory_setup.lock().await;
    let prefs = state.prefs.clone();

    let (was_enabled, saved) = blocking(move || {
        Ok(prefs.with(|store| {
            let was_enabled = store.load().memory_enabled;

            store
                .update(|prefs| prefs.memory_enabled = enabled)
                .map(|saved| (was_enabled, saved))
        })?)
    })
    .await?;

    let Some(server) = server else {
        return Ok(saved);
    };

    if enabled {
        install_and_register(state, server, !was_enabled).await;
    } else {
        unregister_all(state).await;
    }

    Ok(saved)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use harness_business::app_dirs::memory_bin_dir;
    use harness_business::ecosystem::{MEMORY_RECORD_FILE, memory_binary_name};
    use harness_business::model::HarnessId;
    use serde_json::{Value, json};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::memory_setup::register_on_launch;
    use crate::test_support::{TempDir, cache_detection, home_state, omp_mcp_file};

    /// A state with only OMP detected as installed and a bundled server to install.
    fn state(dir: &TempDir) -> (AppState, PathBuf) {
        let state = home_state(dir);
        cache_detection(&state, HarnessId::Omp, Some(&dir.path().join("missing-omp")));
        cache_detection(&state, HarnessId::Pi, None);

        let server = dir.path().join("bundle").join(memory_binary_name());
        std::fs::create_dir_all(server.parent().expect("bundle dir")).expect("create bundle");
        std::fs::write(&server, "memory server").expect("write server");

        (state, server)
    }

    fn mcp_servers(state: &AppState) -> Value {
        let text = std::fs::read_to_string(omp_mcp_file(state)).expect("mcp.json");
        serde_json::from_str::<Value>(&text).expect("valid JSON")["mcpServers"].clone()
    }

    #[test]
    fn switching_off_removes_the_entry_and_switching_on_adds_it_back() {
        let dir = TempDir::new("memory-switch");
        let (state, server) = state(&dir);

        block_on(register_on_launch(&state, &server));
        let installed = memory_bin_dir(&state.env).expect("bin dir").join(memory_binary_name());
        let entry = json!({ "command": installed.to_string_lossy(), "args": ["mcp"] });
        assert_eq!(mcp_servers(&state), json!({ "harness-memory": entry }));

        let off = block_on(set_enabled(&state, false, Some(&server))).expect("switched off");
        assert!(!off.memory_enabled);
        assert_eq!(mcp_servers(&state), json!({}));
        assert!(state.config_dir.join(MEMORY_RECORD_FILE).exists(), "the record is kept");

        // Registering again while it is recorded does not add it back.
        block_on(install_and_register(&state, &server, false));
        assert_eq!(mcp_servers(&state), json!({}));

        let on = block_on(set_enabled(&state, true, Some(&server))).expect("switched on");
        assert!(on.memory_enabled);
        assert_eq!(mcp_servers(&state), json!({ "harness-memory": entry }));
    }

    #[test]
    fn a_launch_run_that_gets_the_lock_after_a_switch_off_adds_nothing() {
        let dir = TempDir::new("memory-late-launch");
        let (state, server) = state(&dir);

        // The launch run started while the pref was on; the switch went off before it got the lock.
        block_on(set_enabled(&state, false, Some(&server))).expect("switched off");
        let outcomes = block_on(register_on_launch(&state, &server));

        assert!(outcomes.is_empty());
        assert!(!omp_mcp_file(&state).exists());
        assert!(!state.config_dir.join(MEMORY_RECORD_FILE).exists());
    }

    #[test]
    fn switching_on_forgets_the_record_even_when_the_install_fails() {
        let dir = TempDir::new("memory-on-failed");
        let (state, server) = state(&dir);

        block_on(register_on_launch(&state, &server));
        block_on(set_enabled(&state, false, Some(&server))).expect("switched off");
        assert!(state.config_dir.join(MEMORY_RECORD_FILE).exists());

        let missing = dir.path().join("gone").join(memory_binary_name());
        let on = block_on(set_enabled(&state, true, Some(&missing))).expect("switched on");

        assert!(on.memory_enabled);
        assert!(!state.config_dir.join(MEMORY_RECORD_FILE).exists());
        assert_eq!(mcp_servers(&state), json!({}));
    }

    #[test]
    fn a_build_without_the_server_only_stores_the_pref() {
        let dir = TempDir::new("memory-gated");
        let (state, _) = state(&dir);

        let off = block_on(set_enabled(&state, false, None)).expect("switched off");

        assert!(!off.memory_enabled);
        assert!(!state.prefs.with(|store| store.load()).memory_enabled);
        assert!(!omp_mcp_file(&state).exists());
    }
}
