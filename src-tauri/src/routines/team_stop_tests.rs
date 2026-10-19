//! Team runs that may no longer run: turning a team routine off here, the routine leaving the team
//! and sharing turned off stop its run going on; a routine that left takes its run records along.

use harness_business::model::RunStatus;
use jiff::tz::TimeZone;
use serde_json::{Value, json};
use tauri::async_runtime::block_on;

use super::job::WITHDRAWN_MESSAGE;
use super::team_tests::{TEAM_ID, Team, row, team};
use super::tests::{at, ended, runs_of};
use crate::test_support::wait_until;

/// A command that runs until it is stopped.
fn sleeper() -> Value {
    json!({"type": "command", "command": "sleep 30", "timeoutSecs": null})
}

/// A trusted team routine that sleeps, turned on here and running.
fn running(name: &str) -> Option<Team> {
    let team = team(name)?;

    team.sharing(Some((true, true)));
    team.rows(&[row(TEAM_ID, None, sleeper())]);
    team.refresh();
    team.enable(TEAM_ID, true, None).expect("turned on");

    block_on(team.state().routines.run_now(TEAM_ID))
        .expect("ran")
        .expect("a free place");

    Some(team)
}

/// One look of the scheduler (it checks the team runs after a change).
fn look(team: &Team) {
    block_on(
        team.state()
            .routines
            .tick(at("2026-10-18T09:00:00Z"), &TimeZone::UTC, false),
    );
}

#[cfg(unix)]
#[test]
fn turning_a_team_routine_off_here_stops_its_run() {
    let Some(team) = running("team-routines-turned-off") else {
        return;
    };

    team.enable(TEAM_ID, false, None).expect("turned off");

    let runs = ended(team.state(), TEAM_ID, 1);
    assert_eq!(runs[0].record.status, RunStatus::Stopped);
    assert_eq!(runs[0].record.message.as_deref(), Some(WITHDRAWN_MESSAGE));
}

#[cfg(unix)]
#[test]
fn a_team_routine_that_leaves_the_team_stops_and_takes_its_runs_along() {
    let Some(team) = running("team-routines-left") else {
        return;
    };

    team.rows(&[]);
    team.refresh();
    look(&team);

    wait_until("the run to stop", || !team.state().routines.busy().contains(TEAM_ID));
    assert!(runs_of(team.state(), TEAM_ID).is_empty(), "its run records are deleted");
}

#[cfg(unix)]
#[test]
fn sharing_turned_off_stops_the_team_runs() {
    let Some(team) = running("team-routines-sharing-off") else {
        return;
    };

    team.sharing(Some((false, false)));
    team.refresh();
    look(&team);

    wait_until("the run to stop", || !team.state().routines.busy().contains(TEAM_ID));
    assert!(
        runs_of(team.state(), TEAM_ID).is_empty(),
        "the copies and their runs are gone"
    );
}
