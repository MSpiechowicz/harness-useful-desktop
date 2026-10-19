//! Copying a team routine to the local routines: one that may run on trust alone is copied only
//! while the team database still holds the bytes of its copy, and only the version shown.

use harness_business::model::{HookOrigin, IpcErrorCode, RoutineView};
use tauri::async_runtime::block_on;

use super::team_tests::{TEAM_ID, Team, echo, row, team};
use crate::commands::routines::team as team_commands;
use crate::error::CommandError;
use crate::state::AppState;

fn copy(state: &AppState, view: &RoutineView) -> Result<RoutineView, CommandError> {
    block_on(team_commands::copy(state, view.id.clone(), view.sha256.clone(), None))
}

/// A team routine trusted (sharing and trust on) and turned on here, as the view shows it; then
/// the team database holds another version of it.
fn shown_then_changed(team: &Team) -> RoutineView {
    team.sharing(Some((true, true)));
    team.rows(&[row(TEAM_ID, None, echo("team"))]);
    team.refresh();
    let shown = team.enable(TEAM_ID, true, None).expect("turned on");

    team.rows(&[row(TEAM_ID, None, echo("changed"))]);

    shown
}

/// `refused` is a conflict, and no local copy was written.
fn assert_refused(state: &AppState, refused: &CommandError) {
    assert_eq!(refused.code(), IpcErrorCode::Conflict);
    assert!(
        state.routines.store().unwrap().get(TEAM_ID).is_err(),
        "no local copy is written"
    );
}

#[test]
fn a_trusted_team_routine_is_copied_only_while_the_team_database_holds_its_bytes() {
    let Some(team) = team("team-routines-copy-confirm") else {
        return;
    };
    let state = team.state();

    // This machine has not refreshed since the team copy changed.
    let shown = shown_then_changed(&team);

    assert_refused(state, &copy(state, &shown).expect_err("not confirmed"));

    team.refresh();
    let fresh = team.enable(TEAM_ID, true, None).expect("still on");
    let local = copy(state, &fresh).expect("copied");
    assert_eq!(local.origin, HookOrigin::Local);
}

#[test]
fn a_team_routine_that_changed_since_it_was_shown_is_not_copied() {
    let Some(team) = team("team-routines-copy-stale") else {
        return;
    };
    let state = team.state();

    // A refresh brought the new version after the view was shown.
    let shown = shown_then_changed(&team);
    team.refresh();

    assert_refused(state, &copy(state, &shown).expect_err("another version"));
}
