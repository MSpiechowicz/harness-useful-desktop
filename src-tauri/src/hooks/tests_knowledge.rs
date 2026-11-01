//! The Harness Knowledge hooks follow the member: they run while Harness Knowledge is installed,
//! lists `mcp` and its `knowledgeEnabled` pref is on, and wait for it otherwise. The family dialog
//! counts it as installed whatever the pref.

use std::time::{Instant, SystemTime};

use harness_business::hooks::{HookCapabilities, seed_defaults};
use tauri::async_runtime::block_on;

use super::members::Members;
use super::tests_code::planned_names;
use crate::state::AppState;
use crate::test_support::{TempDir, home_state};

const FIRST: &str = "Check Knowledge first";
const RECALL: &str = "Recall from Harness Knowledge";

/// An app state whose family hooks of Harness Knowledge are seeded and whose cached members are
/// `members`.
fn state_with(dir: &TempDir, members: Members) -> AppState {
    let state = home_state(dir);
    let store = state.hooks.store().expect("a hooks folder");

    let available = HookCapabilities {
        knowledge_available: true,
        ..HookCapabilities::default()
    };
    seed_defaults(&store, available, SystemTime::now()).expect("the family hooks are seeded");

    state.hooks.lock_cache().members = Some((Instant::now(), members));
    state
}

fn knowledge(enabled: bool, mcp: bool) -> Members {
    Members {
        knowledge_installed: true,
        knowledge_enabled: enabled,
        knowledge_mcp: mcp,
        ..Members::default()
    }
}

#[test]
fn the_hooks_run_while_knowledge_is_installed_listed_and_on() {
    let dir = TempDir::new("knowledge-hooks-on");
    let state = state_with(&dir, knowledge(true, true));

    assert_eq!(planned_names(dir.path(), &state), [FIRST, RECALL]);
    assert!(block_on(state.hooks.capabilities()).knowledge_available);
}

#[test]
fn the_hooks_wait_while_the_pref_is_off_or_the_server_lists_no_mcp() {
    for (enabled, mcp) in [(false, true), (true, false)] {
        let dir = TempDir::new("knowledge-hooks-off");
        let state = state_with(&dir, knowledge(enabled, mcp));

        assert_eq!(
            planned_names(dir.path(), &state),
            Vec::<String>::new(),
            "enabled {enabled}, mcp {mcp}"
        );
        assert!(!block_on(state.hooks.capabilities()).knowledge_available);
    }
}

#[test]
fn the_dialog_counts_knowledge_as_installed_whatever_its_pref() {
    let dir = TempDir::new("knowledge-hooks-installed");
    let state = state_with(&dir, knowledge(false, true));

    assert!(block_on(state.hooks.installed()).knowledge_available);
}
