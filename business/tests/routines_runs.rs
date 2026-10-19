//! Run records (see "Run records" in `docs/routines-format.md`): newest first and at most 100 per
//! routine, pages over all routines with filters, leftover `running` runs stopped at launch,
//! limits on what a record keeps, and an invalid file set aside.

mod routine_support;

use std::collections::BTreeSet;

use harness_business::model::{HarnessId, HookOrigin, RoutineRunsQuery, RunSession, RunStatus, RunTrigger};
use harness_business::routines::runs::{CLOSED_MESSAGE, MAX_EXCERPT_BYTES, RunEnd, excerpt_tail, started_run};
use harness_business::routines::{RoutineStore, RunStore};
use jiff::{SignedDuration, Timestamp};
use pretty_assertions::assert_eq;
use routine_support::{command_draft, moment, rid};

fn start_time(minute: i64) -> Timestamp {
    "2026-06-01T08:00:00Z".parse::<Timestamp>().unwrap() + SignedDuration::from_mins(minute)
}

/// A store with one routine named `name`, and its runs.
fn routine_named(name: &str) -> (tempfile::TempDir, harness_business::model::Routine, RunStore) {
    let temp = tempfile::tempdir().unwrap();
    let store = RoutineStore::new(temp.path().join("root"), temp.path().join("config"));
    let view = store
        .save(
            &command_draft(name),
            harness_business::model::HookAuthor::Desktop,
            moment(0),
        )
        .unwrap();
    let runs = RunStore::new(&temp.path().join("local"));

    (temp, view.routine.unwrap(), runs)
}

fn query(routine_id: Option<&str>, status: Option<RunStatus>, offset: u32, limit: u32) -> RoutineRunsQuery {
    RoutineRunsQuery {
        routine_id: routine_id.map(str::to_owned),
        status,
        offset,
        limit,
    }
}

#[test]
fn a_run_is_recorded_when_it_starts_and_rewritten_when_it_ends() {
    let (_temp, routine, runs) = routine_named("Report");

    let run = started_run(&routine, HookOrigin::Local, RunTrigger::Manual, None, start_time(0));
    runs.start(&run).unwrap();

    let stored = runs.runs_of(&routine.id);
    assert_eq!((stored.len(), stored[0].record.status), (1, RunStatus::Running));

    let end = RunEnd {
        status: RunStatus::Failed,
        ended_at: start_time(2),
        message: Some("exit code 2".to_owned()),
        excerpt: Some("out".to_owned()),
        error_excerpt: Some("err".to_owned()),
        exit_code: Some(2),
    };
    let finished = runs.finish(&routine.id, &run.record.id, end).unwrap().unwrap();

    assert_eq!(finished.record.duration_ms, Some(120_000));
    assert_eq!(finished.record.ended_at.as_deref(), Some("2026-06-01T08:02:00Z"));
    assert_eq!(runs.runs_of(&routine.id), [finished]);
}

#[test]
fn a_routine_keeps_its_newest_hundred_runs() {
    let (_temp, routine, runs) = routine_named("Report");

    for minute in 0..105 {
        let run = started_run(
            &routine,
            HookOrigin::Local,
            RunTrigger::Schedule,
            None,
            start_time(minute),
        );
        runs.start(&run).unwrap();
    }

    let stored = runs.runs_of(&routine.id);
    assert_eq!(stored.len(), 100);
    assert_eq!(stored[0].record.started_at, "2026-06-01T09:44:00Z");
    assert_eq!(stored[99].record.started_at, "2026-06-01T08:05:00Z");
}

#[test]
fn pages_cover_every_routine_newest_first_with_filters() {
    let temp = tempfile::tempdir().unwrap();
    let store = RoutineStore::new(temp.path().join("root"), temp.path().join("config"));
    let runs = RunStore::new(temp.path());
    let author = harness_business::model::HookAuthor::Desktop;

    let first = store
        .save(&command_draft("First"), author, moment(0))
        .unwrap()
        .routine
        .unwrap();
    let second = store
        .save(&command_draft("Second"), author, moment(1))
        .unwrap()
        .routine
        .unwrap();

    for minute in 0..5 {
        let routine = if minute % 2 == 0 { &first } else { &second };
        let run = started_run(
            routine,
            HookOrigin::Local,
            RunTrigger::Schedule,
            None,
            start_time(minute),
        );
        runs.start(&run).unwrap();

        if minute == 1 {
            let ok = RunEnd {
                status: RunStatus::Ok,
                ended_at: start_time(minute),
                message: None,
                excerpt: None,
                error_excerpt: None,
                exit_code: Some(0),
            };
            runs.finish(&second.id, &run.record.id, ok).unwrap();
        }
    }

    let page = runs.page(&query(None, None, 0, 2));
    let names: Vec<&str> = page.runs.iter().map(|run| run.record.routine_name.as_str()).collect();
    assert_eq!(
        (names, page.total, page.next_offset),
        (vec!["First", "Second"], 5, Some(2))
    );

    let last = runs.page(&query(None, None, 4, 2));
    assert_eq!((last.runs.len(), last.next_offset), (1, None));

    let mine = runs.page(&query(Some(&second.id), None, 0, 50));
    assert!(mine.runs.iter().all(|run| run.routine_id == second.id));
    assert_eq!(mine.total, 2);

    let done = runs.page(&query(None, Some(RunStatus::Ok), 0, 0));
    assert_eq!((done.total, done.runs.len()), (1, 1), "a limit of 0 counts as 1");
    assert_eq!(runs.find(&done.runs[0].record.id), Some(done.runs[0].clone()));
}

#[test]
fn leftover_running_runs_are_stopped_at_launch() {
    let (_temp, routine, runs) = routine_named("Report");

    let left = started_run(&routine, HookOrigin::Team, RunTrigger::CatchUp, None, start_time(0));
    runs.start(&left).unwrap();
    let live = started_run(&routine, HookOrigin::Local, RunTrigger::Manual, None, start_time(1));
    runs.start(&live).unwrap();
    let going_on = BTreeSet::from([live.record.id.clone()]);

    assert_eq!(runs.stop_leftovers(start_time(30), &going_on).unwrap(), 1);

    let records = runs.runs_of(&routine.id);
    let stopped = &records[1].record;
    assert_eq!(stopped.id, left.record.id);
    assert_eq!(stopped.status, RunStatus::Stopped);
    assert_eq!(stopped.message.as_deref(), Some(CLOSED_MESSAGE));
    assert_eq!(stopped.origin, HookOrigin::Team);
    assert_eq!(
        records[0].record.status,
        RunStatus::Running,
        "a run of this session goes on"
    );

    assert_eq!(runs.stop_leftovers(start_time(31), &going_on).unwrap(), 0);
    assert_eq!(
        runs.stop_leftovers(start_time(32), &BTreeSet::new()).unwrap(),
        1,
        "at exit"
    );
}

#[test]
fn a_record_keeps_short_texts_and_sessions() {
    let (_temp, routine, runs) = routine_named("Report");
    let run = started_run(&routine, HookOrigin::Local, RunTrigger::Manual, None, start_time(0));
    runs.start(&run).unwrap();

    let long = format!("{}é{}", "a".repeat(MAX_EXCERPT_BYTES), "z".repeat(10));
    assert!(excerpt_tail(&long).len() <= MAX_EXCERPT_BYTES);
    assert!(excerpt_tail(&long).ends_with("zzzzzzzzzz"));

    let updated = runs
        .update(&routine.id, &run.record.id, |record| {
            record.session = Some(RunSession {
                harness: HarnessId::Omp,
                id: "s-1".to_owned(),
            });
            record.message = Some("m".repeat(900));
            record.excerpt = Some(long.clone());
        })
        .unwrap()
        .unwrap();

    assert_eq!(updated.record.message.unwrap().chars().count(), 500);
    assert!(updated.record.excerpt.unwrap().len() <= MAX_EXCERPT_BYTES);
    assert_eq!(updated.record.session.unwrap().id, "s-1");

    assert_eq!(runs.update(&routine.id, &rid(9), |_| {}).unwrap(), None);
}

#[test]
fn deleting_and_an_invalid_file() {
    let (temp, routine, runs) = routine_named("Report");
    let run = started_run(&routine, HookOrigin::Local, RunTrigger::Manual, None, start_time(0));
    runs.start(&run).unwrap();

    let path = temp
        .path()
        .join("local/routine-runs")
        .join(format!("{}.json", routine.id));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(&format!("\"routineId\": \"{}\"", routine.id)), "{text}");

    std::fs::write(&path, b"[]").unwrap();
    assert!(runs.runs_of(&routine.id).is_empty());

    runs.start(&run).unwrap();
    assert_eq!(runs.runs_of(&routine.id).len(), 1, "the history starts again");

    runs.delete(&routine.id).unwrap();
    assert!(runs.runs_of(&routine.id).is_empty());
    assert!(!path.exists());
}

fn ended_ok(at: Timestamp) -> RunEnd {
    RunEnd {
        status: RunStatus::Ok,
        ended_at: at,
        message: None,
        excerpt: None,
        error_excerpt: None,
        exit_code: Some(0),
    }
}

#[test]
fn a_run_ending_after_its_records_were_deleted_writes_nothing() {
    let (_temp, routine, runs) = routine_named("Report");

    let run = started_run(&routine, HookOrigin::Local, RunTrigger::Manual, None, start_time(0));
    runs.start(&run).unwrap();
    runs.delete(&routine.id).unwrap();

    assert_eq!(
        runs.finish(&routine.id, &run.record.id, ended_ok(start_time(1)))
            .unwrap(),
        None
    );
    assert_eq!(
        runs.update(&routine.id, &run.record.id, |record| record.exit_code = Some(1))
            .unwrap(),
        None
    );
    assert!(runs.routine_ids().is_empty(), "no record file comes back");
}

#[test]
fn the_runs_of_a_team_routine_that_left_are_deleted_and_the_others_kept() {
    let (_temp, routine, runs) = routine_named("Report");
    let team_routine = |n: u16| harness_business::model::Routine {
        id: rid(n),
        ..routine.clone()
    };

    let local = started_run(&routine, HookOrigin::Local, RunTrigger::Manual, None, start_time(0));
    let gone = started_run(
        &team_routine(1),
        HookOrigin::Team,
        RunTrigger::Manual,
        None,
        start_time(0),
    );
    let still_there = started_run(
        &team_routine(2),
        HookOrigin::Team,
        RunTrigger::Manual,
        None,
        start_time(0),
    );
    let running = started_run(
        &team_routine(3),
        HookOrigin::Team,
        RunTrigger::Manual,
        None,
        start_time(0),
    );

    for run in [&local, &gone, &still_there, &running] {
        runs.start(run).unwrap();
    }

    let busy = std::collections::BTreeSet::from([rid(3)]);
    let deleted = runs.delete_gone_team_runs(&busy, |id| id == rid(2)).unwrap();

    assert_eq!(deleted, [rid(1)]);
    assert!(runs.runs_of(&rid(1)).is_empty());
    assert_eq!(runs.runs_of(&rid(2)).len(), 1, "its team copy is still there");
    assert_eq!(runs.runs_of(&rid(3)).len(), 1, "it still runs");
    assert_eq!(runs.runs_of(&routine.id).len(), 1, "a local routine's runs stay");
}
