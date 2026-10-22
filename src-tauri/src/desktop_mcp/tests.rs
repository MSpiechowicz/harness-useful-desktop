use harness_business::ecosystem::DESKTOP_RECORD_FILE;
use serde_json::{Value, json};
use tauri::async_runtime::block_on;

use super::*;
use crate::test_support::{TempDir, cache_detection, home_state, omp_mcp_file};

/// A state over `<dir>/home` with only OMP detected (at a binary that is never run).
fn omp_only(dir: &TempDir) -> AppState {
    let state = home_state(dir);
    cache_detection(&state, HarnessId::Omp, Some(&dir.path().join("omp")));
    cache_detection(&state, HarnessId::Pi, None);
    cache_detection(&state, HarnessId::OpenCode, None);
    cache_detection(&state, HarnessId::Claude, None);

    state
}

fn read(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json")
}

#[test]
fn a_launch_registers_the_app_in_detected_harnesses_only() {
    let dir = TempDir::new("desktop-mcp-launch");
    let state = omp_only(&dir);
    let program = dir.path().join("apps").join("harness-desktop");

    let outcomes = block_on(register_detected(&state, program.clone()));

    assert_eq!(outcomes, vec![(HarnessId::Omp, DesktopRegistration::Added)]);
    assert_eq!(
        read(&omp_mcp_file(&state))["mcpServers"]["harness-desktop"],
        json!({ "command": program.to_string_lossy(), "args": ["mcp"] })
    );
    assert!(
        !state.env.home.join(".pi").exists(),
        "a harness that is not detected is left alone"
    );
    assert!(state.config_dir.join(DESKTOP_RECORD_FILE).exists());
}

#[test]
fn a_removed_entry_stays_out_at_launch_but_the_user_can_add_it_back() {
    let dir = TempDir::new("desktop-mcp-removed");
    let state = omp_only(&dir);
    let program = dir.path().join("apps").join("harness-desktop");

    block_on(register_detected(&state, program.clone()));
    std::fs::write(omp_mcp_file(&state), "{}").expect("the user removes it");

    let outcomes = block_on(register_detected(&state, program.clone()));
    assert_eq!(outcomes, vec![(HarnessId::Omp, DesktopRegistration::SkippedRemoved)]);
    assert_eq!(read(&omp_mcp_file(&state)), json!({}));

    let shown = block_on(register_program_in(&state, HarnessId::Omp, program)).expect("registered");
    assert_eq!(
        shown,
        McpState::Configured {
            file: "~/.omp/agent/mcp.json".to_owned(),
            name: "harness-desktop".to_owned()
        }
    );
}

#[test]
fn a_new_program_is_written_into_the_apps_entry_at_launch() {
    let dir = TempDir::new("desktop-mcp-moved");
    let state = omp_only(&dir);

    let old = dir.path().join("old").join("Harness.AppImage");
    block_on(register_detected(&state, old));

    let new = dir.path().join("new").join("Harness.AppImage");
    let outcomes = block_on(register_detected(&state, new.clone()));

    assert_eq!(outcomes, vec![(HarnessId::Omp, DesktopRegistration::Updated)]);
    assert_eq!(
        read(&omp_mcp_file(&state))["mcpServers"]["harness-desktop"]["command"],
        json!(new.to_string_lossy())
    );
}
