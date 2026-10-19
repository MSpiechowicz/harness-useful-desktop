//! Stopping and holding runs back: a stopped or quit command leaves no process of its group, a
//! delete checks the version before it stops anything and takes no run in meanwhile, nothing
//! starts once the app closes, a look whose routines could not be read plans and forgets nothing,
//! and global routines run in folders of their own that Sessions covers through the workspace.

use std::path::{Path, PathBuf};
use std::time::Duration;

use harness_business::model::{HarnessId, HookAuthor, IpcErrorCode, RoutineWhen, RunStatus, RunTrigger, SessionRef};
use harness_business::routines::ScheduleState;
use jiff::tz::TimeZone;
use tauri::async_runtime::block_on;

use super::job::{STOPPED_MESSAGE, StopReason};
use super::queue::{Reserved, RunClass, Runs};
use super::tests::{at, command, draft, ended, every_minutes, runs_of, save};
use super::tests_agent::SESSION_ID;
use super::tests_workspace::ran_with_session;
use crate::commands::routines as commands;
use crate::commands::sessions;
use crate::error::CommandError;
use crate::state::AppState;
use crate::test_support::{ProjectHome, wait_until};

/// A command that starts `sleep 30` in the background, writes its pid to `pid_file` and waits.
fn background_sleep(pid_file: &Path) -> String {
    format!("sleep 30 & echo $! > '{}'; wait", pid_file.display())
}

/// The pid `pid_file` holds, once the command wrote it.
fn written_pid(pid_file: &Path) -> u32 {
    wait_until("the background process to start", || {
        std::fs::read_to_string(pid_file).is_ok_and(|text| text.trim().parse::<u32>().is_ok())
    });

    std::fs::read_to_string(pid_file).unwrap().trim().parse().unwrap()
}

/// True while process `pid` runs (a zombie, killed and not reaped yet, does not count).
#[cfg(target_os = "linux")]
fn alive(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };

    stat.rsplit_once(')')
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .is_some_and(|state| state != "Z")
}

/// A long command routine of `setup` that leaves its background pid in a file, run now.
fn start_background(setup: &ProjectHome, name: &str) -> (String, PathBuf, String) {
    let pid_file = setup.home.join(format!("{name}.pid"));
    let routine = save(
        &setup.state,
        &draft(name, RoutineWhen::AppStart, command(&background_sleep(&pid_file))),
        HookAuthor::Desktop,
    );

    let run = block_on(setup.state.routines.run_now(&routine.id))
        .expect("ran")
        .expect("a free place");

    (routine.id, pid_file, run.record.id)
}

#[cfg(target_os = "linux")]
#[test]
fn stopping_a_command_run_ends_what_the_command_started() {
    let setup = ProjectHome::new("routines-stop-group");
    let (id, pid_file, run_id) = start_background(&setup, "Background");
    let pid = written_pid(&pid_file);

    setup.state.routines.stop_run(&run_id).expect("stopped");

    let runs = ended(&setup.state, &id, 1);
    assert_eq!(runs[0].record.message.as_deref(), Some(STOPPED_MESSAGE));
    wait_until("the background process to end", || !alive(pid));
}

#[cfg(target_os = "linux")]
#[test]
fn quitting_ends_what_a_command_started() {
    let setup = ProjectHome::new("routines-quit-group");
    let (id, pid_file, _) = start_background(&setup, "Background");
    let pid = written_pid(&pid_file);

    setup.state.routines.shutdown();

    assert_eq!(runs_of(&setup.state, &id)[0].record.status, RunStatus::Stopped);
    wait_until("the background process to end", || !alive(pid));
}

#[cfg(unix)]
#[test]
fn a_delete_of_a_stale_version_stops_nothing() {
    let setup = ProjectHome::new("routines-delete-stale");
    let state = &setup.state;
    let routine = save(
        state,
        &draft("Slow", RoutineWhen::AppStart, command("sleep 30")),
        HookAuthor::Desktop,
    );
    block_on(state.routines.run_now(&routine.id)).expect("ran");

    let stale = block_on(commands::delete(state, routine.id.clone(), "0".repeat(64)));
    assert_eq!(stale.expect_err("stale").code(), IpcErrorCode::Conflict);

    std::thread::sleep(Duration::from_millis(200));
    assert!(state.routines.running().contains(&routine.id), "the run goes on");
    assert_eq!(runs_of(state, &routine.id)[0].record.status, RunStatus::Running);

    block_on(commands::delete(state, routine.id.clone(), routine.sha256.clone())).expect("deleted");
    assert!(runs_of(state, &routine.id).is_empty());
}

#[test]
fn a_routine_being_deleted_takes_no_run() {
    let setup = ProjectHome::new("routines-deleting");
    let state = &setup.state;
    let routine = save(
        state,
        &draft("Quick", RoutineWhen::AppStart, command("echo hi")),
        HookAuthor::Desktop,
    );
    let ids = vec![routine.id.clone()];

    state.routines.begin_delete(&ids);
    let refused = block_on(state.routines.run_now(&routine.id)).map_err(CommandError::from);
    assert_eq!(refused.expect_err("being deleted").code(), IpcErrorCode::Conflict);
    assert!(runs_of(state, &routine.id).is_empty());

    state.routines.end_delete(&ids);
    block_on(state.routines.run_now(&routine.id)).expect("runs again");
    assert_eq!(ended(state, &routine.id, 1)[0].record.status, RunStatus::Ok);
}

#[test]
fn nothing_starts_once_the_app_closes() {
    let setup = ProjectHome::new("routines-closing");
    let state = &setup.state;
    let routine = save(
        state,
        &draft("On launch", RoutineWhen::AppStart, command("echo hi")),
        HookAuthor::Desktop,
    );

    state.routines.shutdown();

    let refused = block_on(state.routines.run_now(&routine.id)).map_err(CommandError::from);
    assert_eq!(refused.expect_err("closing").code(), IpcErrorCode::Conflict);

    let planned = block_on(state.routines.tick(at("2026-10-18T09:00:00Z"), &TimeZone::UTC, true));
    assert_eq!(planned.expect("read").len(), 1, "due, but not started");
    std::thread::sleep(Duration::from_millis(200));
    assert!(runs_of(state, &routine.id).is_empty());
}

#[test]
fn a_run_stopped_while_it_starts_stops_as_soon_as_it_is_recorded() {
    let mut runs = Runs::default();
    assert_eq!(runs.reserve("a", RunClass::Shell, RunTrigger::Manual), Reserved::Now);

    runs.stop_all(StopReason::Closed);
    assert_eq!(runs.stop_before_start("a"), Some(StopReason::Closed));

    let (stop, stopped) = tokio::sync::watch::channel(None);
    let run = harness_business::model::RoutineRun {
        routine_id: "a".to_owned(),
        needs_you: false,
        record: harness_business::model::RunRecord {
            id: "run".to_owned(),
            routine_name: "A".to_owned(),
            origin: harness_business::model::HookOrigin::Local,
            trigger: RunTrigger::Manual,
            started_at: "2026-10-18T09:00:00Z".to_owned(),
            ended_at: None,
            duration_ms: None,
            status: harness_business::model::RunStatus::Running,
            message: None,
            excerpt: None,
            error_excerpt: None,
            exit_code: None,
            session: None,
            project: None,
        },
    };
    runs.set_live(&run, stop);
    assert_eq!(*stopped.borrow(), Some(StopReason::Closed));

    assert_eq!(
        runs.reserve("b", RunClass::Shell, RunTrigger::Manual),
        Reserved::Refused
    );
}

/// The schedule file of `state`.
fn schedule_of(state: &AppState) -> ScheduleState {
    let local_data = state.routines.local_data.clone().expect("a local data folder");

    ScheduleState::load(&ScheduleState::path(&local_data)).expect("the schedule file")
}

#[cfg(unix)]
#[test]
fn a_look_at_an_unreadable_folder_plans_and_forgets_nothing() {
    use std::os::unix::fs::PermissionsExt;

    let setup = ProjectHome::new("routines-unreadable");
    let state = &setup.state;
    let runtime = &state.routines;
    let routine = save(
        state,
        &draft("Tick", every_minutes(5), command("echo tick")),
        HookAuthor::Desktop,
    );
    let folder = runtime.store().unwrap().routines_dir();

    // No Harness Memory: team routines are known not to be shared, so a look may forget.
    assert!(!block_on(crate::team_sync::reprobe(&**runtime)));

    let tick = |now: &str, launch: bool| block_on(runtime.tick(at(now), &TimeZone::UTC, launch));
    assert_eq!(tick("2026-10-18T09:01:00Z", true).map(|due| due.len()), Some(0));

    // A folder others may write is not read: the look is skipped.
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o770)).unwrap();
    assert_eq!(tick("2026-10-18T09:06:00Z", false), None);
    assert!(schedule_of(state).get(&routine.id).is_some(), "the routine stays armed");

    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    let late = tick("2026-10-18T09:11:00Z", false).expect("read again");
    assert_eq!(late.len(), 1, "the slot since it was armed runs");
    assert_eq!(late[0].slot, Some(at("2026-10-18T09:10:00Z")));
    assert_eq!(ended(state, &routine.id, 1)[0].record.status, RunStatus::Ok);
}

#[test]
fn each_global_routine_runs_in_its_own_folder_and_is_authorized_there() {
    let setup = ProjectHome::new("routines-own-folder");
    let state = &setup.state;
    let first = save(
        state,
        &draft("First", RoutineWhen::AppStart, command("pwd")),
        HookAuthor::Desktop,
    );
    let second = save(
        state,
        &draft("Second", RoutineWhen::AppStart, command("pwd")),
        HookAuthor::Desktop,
    );

    for id in [&first.id, &second.id] {
        block_on(state.routines.run_now(id)).expect("ran");
    }

    let workspace = state.routines.workspace().expect("the workspace exists");
    let folder_of = |id: &str| ended(state, id, 1)[0].record.excerpt.clone().map(PathBuf::from);
    assert_eq!(folder_of(&first.id), Some(workspace.join(&first.id)));
    assert_eq!(folder_of(&second.id), Some(workspace.join(&second.id)));

    let as_project = |path: &Path| path.to_string_lossy().into_owned();
    assert_eq!(
        state.routines.workspace_folder(&as_project(&workspace.join(&first.id))),
        Some(workspace.join(&first.id))
    );
    assert_eq!(state.routines.workspace_folder(&as_project(&workspace.join("x"))), None);
    assert_eq!(
        state.routines.routine_workspaces(),
        sorted([workspace.join(&first.id), workspace.join(&second.id)])
    );
}

#[test]
fn the_workspace_lists_and_opens_the_sessions_of_every_routine_folder() {
    let setup = ProjectHome::new("routines-workspace-sessions");
    let state = &setup.state;

    // The routine's session lives in its own folder.
    let (routine, workspace, _) = ran_with_session(state);

    let parent = workspace.to_string_lossy().into_owned();
    let listed = block_on(sessions::list(state, HarnessId::Omp, parent.clone())).expect("listed");
    assert!(
        listed.iter().any(|summary| summary.session.id == SESSION_ID),
        "{listed:?}"
    );

    let session = SessionRef {
        harness: HarnessId::Omp,
        id: SESSION_ID.to_owned(),
    };
    let found = block_on(sessions::session_dir(state, parent, &session)).expect("found");
    assert_eq!(found, workspace.join(&routine.id));
}

fn sorted<const N: usize>(mut paths: [PathBuf; N]) -> Vec<PathBuf> {
    paths.sort();
    paths.to_vec()
}
