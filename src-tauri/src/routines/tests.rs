//! The routines runtime: the scheduler at injected times (on time, catch-up, app start, busy
//! slots, a clock set back), "Run now", "Stop", deleting a routine, and the records a closed app
//! left. Runs carry out real shell commands in a temporary home.

use std::time::{Duration, SystemTime};

use harness_business::model::{
    HookAuthor, HookOrigin, HookScope, IpcErrorCode, RoutineAction, RoutineDraft, RoutineFilter, RoutineRun,
    RoutineRunsQuery, RoutineStatus, RoutineView, RoutineWhen, RunStatus, RunTrigger,
};
use harness_business::routines::ScheduleState;
use harness_business::routines::runs::{CLOSED_MESSAGE, started_run};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use serde_json::Value;
use tauri::async_runtime::block_on;

use super::job::{AGENTS_UNAVAILABLE, STOPPED_MESSAGE};
use super::scheduler::MAX_SLEEP;
use crate::commands::routines as commands;
use crate::error::CommandError;
use crate::state::AppState;
use crate::test_support::{ProjectHome, collecting_channel, wait_until};

/// A global routine named `name` that runs `action` on `when`, switched on.
pub(super) fn draft(name: &str, when: RoutineWhen, action: RoutineAction) -> RoutineDraft {
    RoutineDraft {
        id: None,
        expected_sha256: None,
        name: name.to_owned(),
        description: None,
        scope: HookScope::Global,
        project: None,
        when,
        action,
        enabled: true,
    }
}

/// A command action with the default timeout.
pub(super) fn command(text: &str) -> RoutineAction {
    RoutineAction::Command {
        command: text.to_owned(),
        timeout_secs: None,
    }
}

pub(super) fn every_minutes(every: u32) -> RoutineWhen {
    RoutineWhen::EveryMinutes { every }
}

/// Writes `draft` as `author` (the desktop's is approved, an agent's pending).
pub(super) fn save(state: &AppState, draft: &RoutineDraft, author: HookAuthor) -> RoutineView {
    let store = state.routines.store().expect("a routines folder");

    store.save(draft, author, SystemTime::now()).expect("routine saved")
}

/// The runs of routine `id`, newest first.
pub(super) fn runs_of(state: &AppState, id: &str) -> Vec<RoutineRun> {
    state.routines.run_store().expect("run records").runs_of(id)
}

/// Waits until routine `id` has `count` runs and none still runs; returns them, newest first.
pub(super) fn ended(state: &AppState, id: &str, count: usize) -> Vec<RoutineRun> {
    wait_until("the routine runs to end", || {
        let runs = runs_of(state, id);
        runs.len() == count && runs.iter().all(|run| run.record.status != RunStatus::Running)
    });

    // The place is freed right after the record is written.
    wait_until("the routine to be free", || !state.routines.busy().contains(id));

    runs_of(state, id)
}

pub(super) fn at(text: &str) -> Timestamp {
    text.parse().expect("a timestamp")
}

pub(super) fn plan(
    runtime: &std::sync::Arc<super::RoutineRuntime>,
    now: &str,
    launch: bool,
) -> Vec<(Option<Timestamp>, RunTrigger, bool)> {
    block_on(runtime.tick(at(now), &TimeZone::UTC, launch))
        .expect("the routines were read")
        .into_iter()
        .map(|due| (due.slot, due.trigger, due.skipped))
        .collect()
}

fn schedule_of(state: &AppState) -> ScheduleState {
    let local_data = state.routines.local_data.clone().expect("a local data folder");

    ScheduleState::load(&ScheduleState::path(&local_data)).expect("the schedule file")
}

pub(super) fn code(error: CommandError) -> IpcErrorCode {
    error.code()
}

#[test]
fn the_scheduler_runs_a_slot_on_time_and_a_missed_one_once_as_catch_up() {
    let setup = ProjectHome::new("routines-schedule");
    let state = &setup.state;
    let runtime = &state.routines;
    let routine = save(
        state,
        &draft("Tick", every_minutes(5), command("echo tick")),
        HookAuthor::Desktop,
    );

    assert!(
        plan(runtime, "2026-10-18T09:01:00Z", true).is_empty(),
        "armed now: no slot before it existed runs"
    );

    let on_time = plan(runtime, "2026-10-18T09:05:30Z", false);
    assert_eq!(
        on_time,
        [(Some(at("2026-10-18T09:05:00Z")), RunTrigger::Schedule, false)]
    );

    let runs = ended(state, &routine.id, 1);
    assert_eq!(runs[0].record.status, RunStatus::Ok);
    assert_eq!(runs[0].record.trigger, RunTrigger::Schedule);
    assert_eq!(runs[0].record.excerpt.as_deref(), Some("tick"));
    assert_eq!(runs[0].record.origin, HookOrigin::Local);
    assert_eq!(runs[0].record.project, None);

    // The machine slept from 09:06 to 09:42: the latest missed slot runs once.
    let late = plan(runtime, "2026-10-18T09:42:30Z", false);
    assert_eq!(late, [(Some(at("2026-10-18T09:40:00Z")), RunTrigger::CatchUp, false)]);

    let runs = ended(state, &routine.id, 2);
    assert_eq!(runs[0].record.trigger, RunTrigger::CatchUp);

    assert!(
        plan(runtime, "2026-10-18T09:43:00Z", false).is_empty(),
        "handled already"
    );
    assert!(
        plan(runtime, "2026-10-18T09:20:00Z", false).is_empty(),
        "a clock set back never runs again"
    );

    let saved = schedule_of(state);
    let entry = saved.get(&routine.id).expect("the routine's schedule entry");
    assert_eq!(entry.last_slot, Some(at("2026-10-18T09:40:00Z")));
    assert_eq!(entry.sha256, routine.sha256);

    // The next slot is 09:45: the wait is capped, then exact.
    assert_eq!(
        runtime.sleep_after(at("2026-10-18T09:43:00Z"), &TimeZone::UTC),
        MAX_SLEEP
    );
    assert_eq!(
        runtime.sleep_after(at("2026-10-18T09:44:50Z"), &TimeZone::UTC),
        Duration::from_secs(10)
    );
}

#[test]
fn app_start_routines_run_on_the_launch_look_only_and_pending_ones_never() {
    let setup = ProjectHome::new("routines-app-start");
    let state = &setup.state;
    let runtime = &state.routines;

    let mine = save(
        state,
        &draft("On launch", RoutineWhen::AppStart, command("echo hi")),
        HookAuthor::Desktop,
    );
    let agents = save(
        state,
        &draft("From an agent", RoutineWhen::AppStart, command("echo agent")),
        HookAuthor::Agent,
    );

    assert!(plan(runtime, "2026-10-18T09:00:00Z", false).is_empty());
    assert_eq!(
        plan(runtime, "2026-10-18T09:00:01Z", true),
        [(None, RunTrigger::AppStart, false)]
    );

    let runs = ended(state, &mine.id, 1);
    assert_eq!(runs[0].record.trigger, RunTrigger::AppStart);
    assert!(runs_of(state, &agents.id).is_empty(), "a pending routine never runs");

    let error = block_on(runtime.run_now(&agents.id)).expect_err("pending");
    assert_eq!(code(error.into()), IpcErrorCode::InvalidInput, "not even with Run now");
}

#[test]
fn run_now_records_the_run_and_tells_the_listener() {
    let setup = ProjectHome::new("routines-run-now");
    let state = &setup.state;
    let runtime = &state.routines;
    let (channel, received) = collecting_channel();
    runtime.listen(channel);

    // Off: Run now does not need the schedule.
    let routine = save(
        state,
        &RoutineDraft {
            enabled: false,
            ..draft(
                "Manual",
                RoutineWhen::Daily { at: "07:00".to_owned() },
                command("echo manual"),
            )
        },
        HookAuthor::Desktop,
    );

    let run = block_on(runtime.run_now(&routine.id))
        .expect("ran")
        .expect("a free place");
    assert_eq!(run.record.status, RunStatus::Running);
    assert_eq!(run.record.trigger, RunTrigger::Manual);
    assert_eq!(run.record.routine_name, "Manual");

    let runs = ended(state, &routine.id, 1);
    assert_eq!(runs[0].record.id, run.record.id);
    assert_eq!(runs[0].record.status, RunStatus::Ok);
    assert!(runs[0].record.duration_ms.is_some() && runs[0].record.ended_at.is_some());

    let kinds: Vec<Value> = received
        .lock()
        .unwrap()
        .iter()
        .map(|event| event["kind"].clone())
        .collect();
    assert_eq!(kinds, ["runStarted", "runFinished"]);

    let page = block_on(commands::runs::runs(
        state,
        RoutineRunsQuery {
            routine_id: Some(routine.id.clone()),
            status: None,
            offset: 0,
            limit: 10,
        },
    ))
    .expect("listed");
    assert_eq!(page.total, 1);
    assert!(!page.runs[0].needs_you);
}

#[test]
fn an_agent_routine_fails_until_agent_runs_are_wired() {
    let setup = ProjectHome::new("routines-agent-stub");
    let state = &setup.state;
    let action = RoutineAction::Agent {
        harness: harness_business::model::HarnessId::Omp,
        model: None,
        prompt: "Review the open pull requests.".to_owned(),
        approval_mode: None,
        trust_once: false,
        max_minutes: None,
    };
    let routine = save(
        state,
        &draft("Review", RoutineWhen::AppStart, action),
        HookAuthor::Desktop,
    );

    block_on(state.routines.run_now(&routine.id)).expect("ran");

    let runs = ended(state, &routine.id, 1);
    assert_eq!(runs[0].record.status, RunStatus::Failed);
    assert_eq!(runs[0].record.message.as_deref(), Some(AGENTS_UNAVAILABLE));
    assert_eq!(runs[0].record.session, None);
}

#[test]
fn a_project_routine_whose_folder_is_gone_is_paused() {
    let setup = ProjectHome::new("routines-missing-folder");
    let state = &setup.state;
    let project = setup.project.to_string_lossy().into_owned();
    let routine = save(
        state,
        &RoutineDraft {
            scope: HookScope::Project,
            project: Some(project.clone()),
            ..draft("In the project", RoutineWhen::AppStart, command("echo here"))
        },
        HookAuthor::Desktop,
    );

    block_on(state.routines.run_now(&routine.id)).expect("ran");
    let runs = ended(state, &routine.id, 1);
    assert_eq!(runs[0].record.status, RunStatus::Ok, "it runs in the project root");
    assert_eq!(runs[0].record.project.as_deref(), Some(project.as_str()));

    std::fs::remove_dir_all(&setup.project).expect("remove the project");

    // Paused with its reason: the scheduler leaves it out and "Run now" says why.
    let view = block_on(state.routines.routine_view(&routine.id)).expect("read");
    assert_eq!(view.status, RoutineStatus::Paused);
    assert_eq!(view.problem.as_deref(), Some("The project folder is missing"));
    assert!(!view.can_run_now);
    assert!(plan(&state.routines, "2026-10-18T09:00:00Z", true).is_empty());

    let error = block_on(state.routines.run_now(&routine.id)).expect_err("paused");
    assert_eq!(
        (
            error.code(),
            error.to_string().contains("The project folder is missing")
        ),
        (IpcErrorCode::InvalidInput, true)
    );
    assert_eq!(runs_of(state, &routine.id).len(), 1, "no run was recorded");
}

#[test]
fn deleting_a_routine_deletes_its_runs_and_schedule_entry() {
    let setup = ProjectHome::new("routines-delete");
    let state = &setup.state;
    let routine = save(
        state,
        &draft("Gone", RoutineWhen::AppStart, command("echo bye")),
        HookAuthor::Desktop,
    );
    let kept = save(
        state,
        &draft("Kept", RoutineWhen::AppStart, command("echo stay")),
        HookAuthor::Desktop,
    );

    assert_eq!(plan(&state.routines, "2026-10-18T09:00:00Z", true).len(), 2);
    ended(state, &routine.id, 1);
    ended(state, &kept.id, 1);
    assert!(schedule_of(state).get(&routine.id).is_some());

    let error = block_on(commands::delete(state, routine.id.clone(), "0".repeat(64))).expect_err("stale version");
    assert_eq!(code(error), IpcErrorCode::Conflict);

    block_on(commands::delete(state, routine.id.clone(), routine.sha256.clone())).expect("deleted");

    assert!(runs_of(state, &routine.id).is_empty());
    assert!(schedule_of(state).get(&routine.id).is_none());
    assert_eq!(runs_of(state, &kept.id).len(), 1, "other routines keep theirs");
    assert!(schedule_of(state).get(&kept.id).is_some());

    let listing = block_on(commands::list(state, None, RoutineFilter::default())).expect("listed");
    let ids: Vec<&str> = listing.routines.iter().map(|view| view.id.as_str()).collect();
    assert_eq!(ids, [kept.id.as_str()]);

    let deleted = block_on(commands::delete_all(state, None)).expect("deleted all");
    assert_eq!(deleted.deleted, 1);
    assert!(runs_of(state, &kept.id).is_empty());
}

#[test]
fn launch_stops_the_runs_a_closed_app_left_running() {
    let setup = ProjectHome::new("routines-leftovers");
    let state = &setup.state;
    let view = save(
        state,
        &draft("Left", RoutineWhen::AppStart, command("echo x")),
        HookAuthor::Desktop,
    );
    let routine = view.routine.expect("valid");

    let run = started_run(
        &routine,
        HookOrigin::Local,
        RunTrigger::Schedule,
        None,
        Timestamp::now(),
    );
    state.routines.run_store().unwrap().start(&run).expect("recorded");

    state.routines.stop_leftovers();

    let runs = runs_of(state, &routine.id);
    assert_eq!(runs[0].record.status, RunStatus::Stopped);
    assert_eq!(runs[0].record.message.as_deref(), Some(CLOSED_MESSAGE));
}

/// A run that takes long enough to be stopped, skipped and listed while it runs.
#[cfg(unix)]
#[test]
fn a_running_routine_skips_its_slots_refuses_run_now_and_stops() {
    let setup = ProjectHome::new("routines-busy");
    let state = &setup.state;
    let runtime = &state.routines;
    let routine = save(
        state,
        &draft("Slow", every_minutes(1), command("sleep 30")),
        HookAuthor::Desktop,
    );

    assert!(plan(runtime, "2026-10-18T09:00:30Z", true).is_empty());

    let run = block_on(runtime.run_now(&routine.id))
        .expect("ran")
        .expect("a free place");

    let error = block_on(runtime.run_now(&routine.id)).expect_err("busy");
    assert_eq!(code(error.into()), IpcErrorCode::Conflict);

    let busy = plan(runtime, "2026-10-18T09:01:10Z", false);
    assert_eq!(busy, [(Some(at("2026-10-18T09:01:00Z")), RunTrigger::Schedule, true)]);

    let listing = block_on(commands::list(state, None, RoutineFilter::default())).expect("listed");
    assert_eq!(listing.routines[0].status, RoutineStatus::Running);

    runtime.stop_run(&run.record.id).expect("stopped");

    let runs = ended(state, &routine.id, 1);
    assert_eq!(runs[0].record.status, RunStatus::Stopped);
    assert_eq!(runs[0].record.message.as_deref(), Some(STOPPED_MESSAGE));
    assert!(
        runs[0].record.duration_ms.is_some_and(|ms| ms < 20_000),
        "the command was killed"
    );

    let error = runtime.stop_run(&run.record.id).expect_err("not running");
    assert_eq!(code(error.into()), IpcErrorCode::NotFound);
}

#[cfg(unix)]
#[test]
fn exit_marks_the_live_runs_stopped() {
    let setup = ProjectHome::new("routines-exit");
    let state = &setup.state;
    let routine = save(
        state,
        &draft("Long", RoutineWhen::AppStart, command("sleep 30")),
        HookAuthor::Desktop,
    );

    block_on(state.routines.run_now(&routine.id)).expect("ran");
    state.routines.shutdown();

    let runs = ended(state, &routine.id, 1);
    assert_eq!(runs[0].record.status, RunStatus::Stopped);
    assert_eq!(runs[0].record.message.as_deref(), Some(CLOSED_MESSAGE));
}
