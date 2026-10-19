//! The folder of a global routine goes with its routine: deleting the routine deletes the folder,
//! the folder is no longer a place the session commands open, and "Global routines" in Sessions
//! no longer lists the sessions its runs kept (they stay in the harness's own session store).

use std::path::{Path, PathBuf};

use harness_business::model::{HarnessId, HookAuthor, RoutineView, RoutineWhen};
use tauri::async_runtime::block_on;

use super::tests::{command, draft, ended, save};
use super::tests_agent::{SESSION_FILE, SESSION_ID};
use crate::commands::routines as commands;
use crate::commands::sessions;
use crate::state::AppState;
use crate::test_support::ProjectHome;

/// A global command routine that ran once and left an OMP session in its own folder: the
/// routine, the workspace, and the folder that holds the session.
pub(super) fn ran_with_session(state: &AppState) -> (RoutineView, PathBuf, PathBuf) {
    let routine = save(
        state,
        &draft("Nightly", RoutineWhen::AppStart, command("true")),
        HookAuthor::Desktop,
    );
    block_on(state.routines.run_now(&routine.id)).expect("ran");
    ended(state, &routine.id, 1);

    let workspace = state.routines.workspace().expect("the workspace exists");
    let omp = state.registry.get(HarnessId::Omp).expect("OMP");
    let sessions_dir = omp.sessions_dir(&state.env, &workspace.join(&routine.id));
    std::fs::create_dir_all(&sessions_dir).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../business/tests/fixtures/sessions/omp")
        .join(SESSION_FILE);
    std::fs::copy(fixture, sessions_dir.join(SESSION_FILE)).unwrap();

    (routine, workspace, sessions_dir)
}

#[test]
fn deleting_a_global_routine_deletes_its_folder_and_leaves_its_sessions_out() {
    let setup = ProjectHome::new("routines-workspace-delete");
    let state = &setup.state;
    let (routine, workspace, sessions_dir) = ran_with_session(state);
    let folder = workspace.join(&routine.id);

    let parent = workspace.to_string_lossy().into_owned();
    let listed = |state| block_on(sessions::list(state, HarnessId::Omp, parent.clone())).expect("listed");
    assert!(listed(state).iter().any(|summary| summary.session.id == SESSION_ID));

    let current = block_on(commands::get(state, routine.id.clone())).expect("shown");
    block_on(commands::delete(state, routine.id.clone(), current.sha256)).expect("deleted");

    assert!(!folder.exists(), "the folder went with its routine");
    let as_project = folder.to_string_lossy().into_owned();
    assert_eq!(state.routines.workspace_folder(&as_project), None);
    assert!(listed(state).iter().all(|summary| summary.session.id != SESSION_ID));
    assert!(sessions_dir.join(SESSION_FILE).is_file(), "the session itself stays");
}
