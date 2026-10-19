//! The runtime around a run's start: a launch never stops a run this session started, a project
//! routine whose folder is gone is paused (not run, not notified, refused by "Run now"), a
//! routine waiting for a free place says so, and the runs going on are listed from memory.

use harness_business::model::{
    HookAuthor, HookScope, IpcErrorCode, RoutineDraft, RoutineRunsQuery, RoutineStatus, RoutineWhen, RunStatus,
};
use harness_business::routines::view::PROJECT_MISSING;
use tauri::async_runtime::block_on;

use super::job::{STOPPED_MESSAGE, StopReason};
use super::queue::MAX_SHELL_RUNS;
use super::runner::ALREADY_QUEUED;
use super::tests::{code, command, draft, ended, every_minutes, plan, runs_of, save};
use crate::commands::routines as commands;
use crate::commands::routines::runs;
use crate::test_support::{ProjectHome, collecting_channel};

#[cfg(unix)]
#[test]
fn stopping_the_leftovers_spares_a_run_this_session_started() {
    let setup = ProjectHome::new("routines-launch-race");
    let state = &setup.state;
    let routine = save(
        state,
        &draft("Early", RoutineWhen::AppStart, command("sleep 30")),
        HookAuthor::Desktop,
    );

    let run = block_on(state.routines.run_now(&routine.id))
        .expect("ran")
        .expect("a free place");

    state.routines.stop_leftovers();
    assert_eq!(runs_of(state, &routine.id)[0].record.status, RunStatus::Running);

    state.routines.stop_run(&run.record.id).expect("stopped");
    let runs = ended(state, &routine.id, 1);
    assert_eq!(runs[0].record.message.as_deref(), Some(STOPPED_MESSAGE));
}

#[test]
fn a_project_routine_whose_folder_is_gone_is_paused_and_never_runs() {
    let setup = ProjectHome::new("routines-paused");
    let state = &setup.state;
    let runtime = &state.routines;

    let folder = setup.home.join("side");
    std::fs::create_dir_all(&folder).expect("create the project");
    let folder = dunce::canonicalize(&folder).expect("canonical");

    let routine = save(
        state,
        &RoutineDraft {
            scope: HookScope::Project,
            project: Some(folder.to_string_lossy().into_owned()),
            ..draft("Side", every_minutes(1), command("echo side"))
        },
        HookAuthor::Desktop,
    );
    assert!(plan(runtime, "2026-10-18T09:00:30Z", true).is_empty(), "armed");

    std::fs::remove_dir(&folder).expect("remove the project");

    let shown = block_on(commands::get(state, routine.id.clone())).expect("shown");
    assert_eq!(shown.status, RoutineStatus::Paused);
    assert_eq!(shown.problem.as_deref(), Some(PROJECT_MISSING));
    assert!(!shown.can_run_now);

    assert!(
        plan(runtime, "2026-10-18T09:01:10Z", false).is_empty(),
        "a paused routine is not planned"
    );

    let refused = block_on(runtime.run_now(&routine.id)).expect_err("paused");
    assert!(refused.to_string().contains(PROJECT_MISSING), "{refused}");
    assert!(runs_of(state, &routine.id).is_empty(), "nothing ran, nothing to notify");

    std::fs::create_dir_all(&folder).expect("the project is back");
    let back = block_on(commands::get(state, routine.id.clone())).expect("shown");
    assert_eq!(back.status, RoutineStatus::Active);
    assert_eq!(back.problem, None);
}

#[cfg(unix)]
#[test]
fn a_routine_waiting_for_a_free_place_is_listed_as_queued_and_run_now_says_so() {
    let setup = ProjectHome::new("routines-queued");
    let state = &setup.state;
    let runtime = &state.routines;
    let (listener, events) = collecting_channel();
    runtime.listen(listener);

    for n in 0..MAX_SHELL_RUNS {
        let busy = save(
            state,
            &draft(&format!("Busy {n}"), RoutineWhen::AppStart, command("sleep 30")),
            HookAuthor::Desktop,
        );
        block_on(runtime.run_now(&busy.id)).expect("ran").expect("a free place");
    }

    let waiting = save(
        state,
        &draft("Waiting", RoutineWhen::AppStart, command("echo done")),
        HookAuthor::Desktop,
    );
    assert_eq!(block_on(runtime.run_now(&waiting.id)).expect("queued"), None);
    assert!(events.lock().unwrap().iter().any(|event| event["kind"] == "changed"));

    let shown = block_on(commands::get(state, waiting.id.clone())).expect("shown");
    assert!(shown.queued);
    assert_eq!(shown.status, RoutineStatus::Active);

    let running = RoutineRunsQuery {
        routine_id: None,
        status: Some(RunStatus::Running),
        offset: 0,
        limit: 3,
    };
    let page = block_on(runs::runs(state, running.clone())).expect("the runs going on");
    assert_eq!((page.total, page.runs.len(), page.next_offset), (4, 3, Some(3)));
    assert!(page.runs.iter().all(|run| run.record.status == RunStatus::Running));

    let one = RoutineRunsQuery {
        routine_id: Some(page.runs[0].routine_id.clone()),
        ..running
    };
    assert_eq!(block_on(runs::runs(state, one)).expect("one routine's").total, 1);

    let refused = block_on(runtime.run_now(&waiting.id)).expect_err("waits");
    assert!(refused.to_string().contains(ALREADY_QUEUED), "{refused}");
    assert_eq!(code(refused.into()), IpcErrorCode::Conflict);

    runtime.stop_all(StopReason::User);
    runtime.wait_idle(std::time::Duration::from_secs(5));
}
