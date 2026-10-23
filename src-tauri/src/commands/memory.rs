//! The "Project memory" switch (`memory_set_enabled`): stores the `memoryEnabled` pref and applies
//! it to the harnesses' user MCP files right away (see `memory_setup`).

use harness_business::ecosystem::memory_server_setup;
use harness_business::model::MemorySwitchResult;
use tauri::State;
use tauri::async_runtime::block_on;

use super::blocking;
use crate::error::CommandResult;
use crate::hooks::seed_available;
use crate::memory_embedder::reconcile;
use crate::memory_setup::{register, unregister_all};
use crate::state::AppState;

/// Turns project memory on (registers the installed server in every detected harness, also where
/// the user removed it before) or off (removes the app's own entries). Returns the stored pref, the
/// server's install state (while it is not installed nothing is registered, and the UI offers its
/// installer) and, when switched off, the entries left in files with comments for the user to
/// remove by hand. Any other failure to change an MCP file is logged; the pref is stored regardless.
#[tauri::command]
pub async fn memory_set_enabled(state: State<'_, AppState>, enabled: bool) -> CommandResult<MemorySwitchResult> {
    set_enabled(&state, enabled).await
}

async fn set_enabled(state: &AppState, enabled: bool) -> CommandResult<MemorySwitchResult> {
    // Held from storing the pref to applying it, so quick toggles and the launch run apply in order.
    let running = state.memory_setup.lock().await;
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

    let manual_removals = if enabled {
        register(state, !was_enabled).await;
        Vec::new()
    } else {
        unregister_all(state).await
    };

    // The version probe only reads, so a slow `--version` does not hold up other memory setup.
    drop(running);

    // Hooks that need Harness Memory run or pause from now on; switched on, its family hooks are
    // seeded when they never were.
    state.hooks.refresh_members();
    if enabled {
        seed_available(&state.hooks).await;
    }

    // Vector search runs only while project memory is on.
    reconcile(state).await;

    let env = state.env.clone();
    let runner = state.runner;
    let server = blocking(move || Ok(block_on(memory_server_setup(&env, &runner)))).await?;

    Ok(MemorySwitchResult {
        memory_enabled: saved.memory_enabled,
        server,
        manual_removals,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use harness_business::ecosystem::MEMORY_RECORD_FILE;
    use harness_business::model::{HarnessId, ManualRemoval};
    use serde_json::{Value, json};
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::memory_setup::{after_install, on_launch};
    use crate::test_support::{
        TempDir, cache_detection, home_state, omp_mcp_file, place_companion, place_memory_server,
    };

    /// A state with only OMP detected as installed.
    fn state(dir: &TempDir) -> AppState {
        let state = home_state(dir);
        cache_detection(&state, HarnessId::Omp, Some(&dir.path().join("missing-omp")));
        cache_detection(&state, HarnessId::Pi, None);
        cache_detection(&state, HarnessId::OpenCode, None);
        cache_detection(&state, HarnessId::Claude, None);
        cache_detection(&state, HarnessId::Codex, None);

        state
    }

    /// Installs `harness-memory` in the state's home and returns the entry that runs it; `None` (the
    /// test skips) when `fake_agent` is not built.
    fn install(dir: &TempDir) -> Option<(PathBuf, Value)> {
        let server = place_memory_server(&dir.path().join("home"))?;

        Some(entry(server))
    }

    fn entry(server: PathBuf) -> (PathBuf, Value) {
        let entry = json!({ "command": server.to_string_lossy(), "args": ["mcp"] });

        (server, entry)
    }

    fn mcp_servers(state: &AppState) -> Value {
        let text = std::fs::read_to_string(omp_mcp_file(state)).expect("mcp.json");
        serde_json::from_str::<Value>(&text).expect("valid JSON")["mcpServers"].clone()
    }

    #[test]
    fn switching_off_removes_the_entry_and_switching_on_adds_it_back() {
        let dir = TempDir::new("memory-switch");
        let state = state(&dir);
        let Some((_, entry)) = install(&dir) else {
            return;
        };

        block_on(on_launch(&state));
        assert_eq!(mcp_servers(&state), json!({ "harness-memory": entry }));

        let off = block_on(set_enabled(&state, false)).expect("switched off");
        assert!(!off.memory_enabled);
        assert!(off.server.installed);
        assert_eq!(off.manual_removals, []);
        assert_eq!(mcp_servers(&state), json!({}));
        assert!(state.config_dir.join(MEMORY_RECORD_FILE).exists(), "the record is kept");

        // Registering again while it is recorded does not add it back.
        block_on(register(&state, false));
        assert_eq!(mcp_servers(&state), json!({}));

        let on = block_on(set_enabled(&state, true)).expect("switched on");
        assert!(on.memory_enabled);
        assert_eq!(mcp_servers(&state), json!({ "harness-memory": entry }));
    }

    /// A state with only OpenCode detected as installed, its global config file holding `text`.
    fn opencode_state(dir: &TempDir, text: &str) -> (AppState, PathBuf) {
        let state = home_state(dir);
        cache_detection(&state, HarnessId::Omp, None);
        cache_detection(&state, HarnessId::Pi, None);
        cache_detection(&state, HarnessId::OpenCode, Some(&dir.path().join("missing-opencode")));
        cache_detection(&state, HarnessId::Claude, None);
        cache_detection(&state, HarnessId::Codex, None);

        let file = dir.path().join("home/.config/opencode/opencode.jsonc");
        std::fs::create_dir_all(file.parent().expect("config dir")).expect("config dir");
        std::fs::write(&file, text).expect("write opencode.jsonc");

        (state, file)
    }

    /// An OpenCode config whose `harness-memory` entry is the app's own; `comment` goes above it.
    fn opencode_config(comment: &str) -> String {
        format!(
            "{{\n  \"mcp\": {{\n    {comment}\n    \"harness-memory\": {{ \"type\": \"local\", \"command\": \
             [\"/opt/bin/harness-memory\", \"mcp\"], \"enabled\": true }}\n  }}\n}}\n"
        )
    }

    #[test]
    fn switching_off_reports_the_entry_in_a_file_with_comments_and_leaves_the_file_alone() {
        let dir = TempDir::new("memory-off-commented");
        let text = opencode_config("// keep this note");
        let (state, file) = opencode_state(&dir, &text);

        let off = block_on(set_enabled(&state, false)).expect("switched off");

        assert!(!off.memory_enabled);
        assert_eq!(
            off.manual_removals,
            [ManualRemoval {
                harness: HarnessId::OpenCode,
                file: "~/.config/opencode/opencode.jsonc".to_owned(),
                name: "harness-memory".to_owned(),
            }]
        );
        assert_eq!(std::fs::read_to_string(&file).expect("read"), text, "byte for byte");
    }

    #[test]
    fn switching_off_removes_the_entry_from_a_file_without_comments() {
        let dir = TempDir::new("memory-off-plain");
        let (state, file) = opencode_state(&dir, &opencode_config(""));

        let off = block_on(set_enabled(&state, false)).expect("switched off");

        assert_eq!(off.manual_removals, []);
        let config: Value = serde_json::from_str(&std::fs::read_to_string(&file).expect("read")).expect("JSON");
        assert_eq!(config["mcp"], json!({}));
    }

    #[test]
    fn a_launch_run_that_gets_the_lock_after_a_switch_off_adds_nothing() {
        let dir = TempDir::new("memory-late-launch");
        let state = state(&dir);
        if install(&dir).is_none() {
            return;
        }

        // The launch run started while the pref was on; the switch went off before it got the lock.
        block_on(set_enabled(&state, false)).expect("switched off");
        let outcomes = block_on(on_launch(&state));

        assert_eq!(outcomes, Some(Vec::new()));
        assert!(!omp_mcp_file(&state).exists());
        assert!(!state.config_dir.join(MEMORY_RECORD_FILE).exists());
    }

    #[test]
    fn without_the_server_switching_on_registers_nothing_and_says_so() {
        let dir = TempDir::new("memory-not-installed");
        let state = state(&dir);
        let Some((server, _)) = install(&dir) else {
            return;
        };

        block_on(on_launch(&state));
        block_on(set_enabled(&state, false)).expect("switched off");
        assert!(state.config_dir.join(MEMORY_RECORD_FILE).exists());
        std::fs::remove_file(&server).expect("uninstall the server");

        let on = block_on(set_enabled(&state, true)).expect("switched on");

        assert!(on.memory_enabled);
        assert!(!on.server.installed);
        assert_eq!(on.server.version, None);
        assert!(!on.server.command.is_empty(), "the installer to offer");
        assert!(state.prefs.with(|store| store.load()).memory_enabled);
        assert!(
            !state.config_dir.join(MEMORY_RECORD_FILE).exists(),
            "switching on forgets the record even without the server"
        );
        assert_eq!(mcp_servers(&state), json!({}));
        assert_eq!(
            block_on(on_launch(&state)),
            None,
            "nothing is registered at launch either"
        );
    }

    #[test]
    fn an_install_registers_the_server_without_a_restart() {
        let dir = TempDir::new("memory-installed");
        let state = state(&dir);

        assert_eq!(block_on(on_launch(&state)), None);
        assert!(!omp_mcp_file(&state).exists());

        let Some((_, entry)) = install(&dir) else {
            return;
        };
        let registered = block_on(after_install(&state)).expect("the server is installed");

        assert_eq!(registered.len(), 1);
        assert_eq!(registered[0].0, HarnessId::Omp);
        assert_eq!(mcp_servers(&state), json!({ "harness-memory": entry }));
    }

    #[test]
    fn an_install_while_memory_is_off_registers_nothing() {
        let dir = TempDir::new("memory-installed-off");
        let state = state(&dir);
        block_on(set_enabled(&state, false)).expect("switched off");
        if install(&dir).is_none() {
            return;
        }

        assert_eq!(block_on(after_install(&state)), Some(Vec::new()));
        assert!(!omp_mcp_file(&state).exists());
    }

    #[test]
    // `~/.local/bin` is the installer's folder only on Linux and macOS (Windows has its own, tested apart).
    #[cfg(unix)]
    fn the_installers_server_is_registered_over_a_silent_one_on_path() {
        use std::sync::Arc;

        use harness_business::ecosystem::MemoryRegistration;

        use crate::test_support::place_executable;

        let dir = TempDir::new("memory-shadowed");
        let mut state = state(&dir);
        let bun = dir.path().join("home").join(".bun").join("bin");
        place_executable(&bun, "harness-memory");
        state.env = Arc::new(state.env.as_ref().clone().with_var("PATH", bun.to_string_lossy()));
        let Some((_, entry)) = install(&dir) else {
            return;
        };

        let registered = block_on(on_launch(&state)).expect("the installed server reports its version");

        assert_eq!(registered, [(HarnessId::Omp, MemoryRegistration::Added)]);
        assert_eq!(mcp_servers(&state), json!({ "harness-memory": entry }));
    }

    #[test]
    fn a_server_that_reports_no_version_is_not_installed() {
        let dir = TempDir::new("memory-silent");
        let state = state(&dir);
        place_companion(&dir.path().join("home"), "harness-memory");

        assert_eq!(block_on(on_launch(&state)), None, "nothing registered");
        assert!(!omp_mcp_file(&state).exists());

        // An entry the app wrote before is not re-pointed at it either.
        std::fs::create_dir_all(omp_mcp_file(&state).parent().expect("agent dir")).expect("agent dir");
        let before =
            json!({ "mcpServers": { "harness-memory": { "command": "/old/harness-memory", "args": ["mcp"] } } });
        std::fs::write(omp_mcp_file(&state), before.to_string()).expect("write mcp.json");
        assert_eq!(block_on(register(&state, false)), None);
        assert_eq!(mcp_servers(&state), before["mcpServers"]);

        let on = block_on(set_enabled(&state, true)).expect("switched on");
        assert!(!on.server.installed, "the installer is offered");
    }
}
