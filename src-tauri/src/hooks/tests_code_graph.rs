//! The "Use the code graph first" hook follows the Code index switch file on every prompt, even
//! while the cached members still say the switch is on (it was turned off outside the app).

use std::path::PathBuf;
use std::time::{Instant, SystemTime};

use harness_business::ecosystem::{CodeGraphSupport, memory_data_dir};
use harness_business::hooks::{ChatContext, HookCapabilities, HookSession, PlannedPart, SendOrigin, seed_defaults};
use tauri::async_runtime::block_on;

use super::members::Members;
use super::tests::prompt;
use crate::state::AppState;
use crate::test_support::{TempDir, home_state};

const CODE_GRAPH_HOOK: &str = "Use the code graph first";
const MEMORY_HOOK: &str = "Check Memory first";

/// An app state with the family hooks of every member seeded, and cached members that say the
/// code graph is on.
struct Seeded {
    _dir: TempDir,
    state: AppState,
    switch: PathBuf,
}

fn seeded() -> Seeded {
    let dir = TempDir::new("code-graph-switch");
    let state = home_state(&dir);

    let available = HookCapabilities {
        memory_available: true,
        dashboard_available: true,
        code_graph_available: true,
    };
    let store = state.hooks.store().expect("a hooks folder");
    seed_defaults(&store, available, SystemTime::now()).expect("the family hooks are seeded");

    let data = memory_data_dir(&state.env).expect("a data folder");
    std::fs::create_dir_all(&data).expect("create the data folder");

    state.hooks.lock_cache().members = Some((
        Instant::now(),
        Members {
            memory_installed: true,
            memory_enabled: true,
            dashboard_installed: true,
            code_graph: CodeGraphSupport { listed: true, on: true },
        },
    ));

    Seeded {
        switch: data.join("code.toml"),
        state,
        _dir: dir,
    }
}

/// The names of the hooks whose context the next prompt carries.
fn injected(setup: &Seeded) -> Vec<String> {
    let hooks = block_on(setup.state.hooks.hook_set(None));
    let chat = ChatContext {
        session_id: "chat".to_owned(),
        folder: setup._dir.path().to_path_buf(),
        project: None,
        fresh: false,
        utility: false,
    };

    let plan = HookSession::new(chat).on_send(&hooks, &prompt("hello"), SendOrigin::User);

    let mut names: Vec<String> = plan
        .firings
        .iter()
        .flat_map(|firing| &firing.parts)
        .filter_map(|part| match part {
            PlannedPart::Text(part) => Some(part.hook_name.clone()),
            PlannedPart::Command(_) => None,
        })
        .collect();
    names.sort();

    names
}

#[test]
fn a_missing_switch_file_keeps_the_hook() {
    let setup = seeded();

    let names = injected(&setup);
    assert!(names.iter().any(|name| name == CODE_GRAPH_HOOK), "{names:?}");
    assert!(names.iter().any(|name| name == MEMORY_HOOK), "{names:?}");
}

#[test]
fn a_switch_file_that_is_on_keeps_the_hook() {
    let setup = seeded();
    std::fs::write(&setup.switch, "enabled = true\n").unwrap();

    assert!(injected(&setup).iter().any(|name| name == CODE_GRAPH_HOOK));
}

#[test]
fn a_switch_turned_off_outside_the_app_stops_the_hook_on_the_next_prompt() {
    let setup = seeded();
    let before = injected(&setup);
    assert!(before.iter().any(|name| name == CODE_GRAPH_HOOK), "{before:?}");

    // The members cache is fresh and still says on: only the file changed.
    std::fs::write(&setup.switch, "enabled = false\n").unwrap();

    let after = injected(&setup);
    assert!(!after.iter().any(|name| name == CODE_GRAPH_HOOK), "{after:?}");

    let others: Vec<&String> = before.iter().filter(|name| *name != CODE_GRAPH_HOOK).collect();
    assert_eq!(
        after.iter().collect::<Vec<_>>(),
        others,
        "the other family hooks are unaffected"
    );
    assert!(after.iter().any(|name| name == MEMORY_HOOK));

    // Turned on again: the hook is back without waiting for the members cache.
    std::fs::write(&setup.switch, "enabled = true\n").unwrap();
    assert!(injected(&setup).iter().any(|name| name == CODE_GRAPH_HOOK));
}

#[test]
fn an_invalid_switch_file_stops_the_hook() {
    let setup = seeded();

    for text in ["enabled = [", "enabled = \"yes\"\n"] {
        std::fs::write(&setup.switch, text).unwrap();

        let names = injected(&setup);
        assert!(!names.iter().any(|name| name == CODE_GRAPH_HOOK), "{text:?}: {names:?}");
        assert!(names.iter().any(|name| name == MEMORY_HOOK), "{text:?}: {names:?}");
    }
}

#[test]
fn a_switch_turned_off_makes_the_hooks_list_say_so_as_well() {
    let setup = seeded();
    std::fs::write(&setup.switch, "enabled = false\n").unwrap();

    assert!(!block_on(setup.state.hooks.capabilities()).code_graph_available);
    assert!(block_on(setup.state.hooks.installed()).code_graph_available);
}
