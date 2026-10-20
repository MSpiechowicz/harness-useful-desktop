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

    for companion in [Companion::Dashboard, Companion::Memory, Companion::OpenCodeSkills] {
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
        (Companion::OpenCodeSkills, &state.opencode_skills_install),
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

/// OpenCode's global config file in `state`'s home, with OpenCode detected.
fn opencode_config(dir: &TempDir, state: &AppState, name: &str) -> std::path::PathBuf {
    cache_detection(state, HarnessId::OpenCode, Some(&dir.path().join("missing-opencode")));

    dir.path().join("home/.config/opencode").join(name)
}

#[test]
fn mcp_registration_writes_the_dashboard_server_into_the_opencode_config() {
    let dir = TempDir::new("setup-mcp-opencode");
    let state = state(&dir);
    install_dashboard(&dir.path().join("home"));
    let path = opencode_config(&dir, &state, "opencode.json");

    let registered = block_on(register(&state, HarnessId::OpenCode)).expect("registered");

    assert_eq!(
        registered,
        McpState::Configured {
            file: "~/.config/opencode/opencode.json".to_owned(),
            name: "harness-dashboard".to_owned()
        }
    );
    let document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("opencode.json")).expect("valid JSON");
    assert_eq!(
        document["mcp"]["harness-dashboard"],
        json!({ "type": "local", "command": ["harness-dashboard", "mcp"], "enabled": true })
    );
}

#[test]
fn registration_in_a_commented_opencode_config_offers_the_entry_to_paste() {
    let dir = TempDir::new("setup-opencode-manual");
    let state = state(&dir);
    install_dashboard(&dir.path().join("home"));
    let path = opencode_config(&dir, &state, "opencode.jsonc");
    let text = "{\n  // mine\n  \"theme\": \"opencode\"\n}\n";
    std::fs::create_dir_all(path.parent().expect("config dir")).expect("config dir");
    std::fs::write(&path, text).expect("write opencode.jsonc");

    let dashboard = block_on(register(&state, HarnessId::OpenCode)).expect("answered");

    assert!(
        matches!(&dashboard, McpState::ManualEdit { file, snippet }
            if file == "~/.config/opencode/opencode.jsonc" && snippet.starts_with("\"harness-dashboard\": {")),
        "{dashboard:?}"
    );
    assert_eq!(std::fs::read_to_string(&path).expect("opencode.jsonc"), text);

    let Some(server) = place_memory_server(&dir.path().join("home")) else {
        return;
    };

    let memory = block_on(register_memory(&state, HarnessId::OpenCode)).expect("answered");

    let McpState::ManualEdit { file, snippet } = &memory else {
        panic!("expected a manual edit, got {memory:?}");
    };
    assert_eq!(file, "~/.config/opencode/opencode.jsonc");
    let pasted: serde_json::Value = serde_json::from_str(&format!("{{{snippet}}}")).expect("an object member");
    assert_eq!(
        pasted,
        json!({ "harness-memory": { "type": "local", "command": [server.to_string_lossy(), "mcp"], "enabled": true } })
    );
    assert_eq!(std::fs::read_to_string(&path).expect("opencode.jsonc"), text);
    assert!(
        MemoryRecord::load(&state.config_dir).registered().is_empty(),
        "a file the app could not write is not recorded"
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
