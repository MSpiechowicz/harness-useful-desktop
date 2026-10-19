//! Which routines are due (see "Catch-up and the schedule file" in `docs/routines-format.md`): a
//! slot runs on time or once as a catch-up, never before the routine was armed or at or before
//! the last slot handled, a busy routine's slot is skipped, app start routines run once per
//! launch, and the schedule file round-trips.

use harness_business::model::{RoutineWhen, RunTrigger};
use harness_business::routines::{Due, Launch, ScheduleState, Scheduled, due, forget_unplanned, next_wake};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use pretty_assertions::assert_eq;

const UTC: TimeZone = TimeZone::UTC;

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn routine(id: &str, sha256: &str, when: RoutineWhen) -> Scheduled {
    Scheduled {
        id: id.to_owned(),
        sha256: sha256.to_owned(),
        when,
        busy: false,
        team: false,
    }
}

fn daily_nine() -> Scheduled {
    routine("a", "v1", RoutineWhen::Daily { at: "09:00".to_owned() })
}

/// `(slot, trigger, skipped)` of each due routine.
fn summary(due: &[Due]) -> Vec<(Option<String>, RunTrigger, bool)> {
    due.iter()
        .map(|item| (item.slot.map(|slot| slot.to_string()), item.trigger, item.skipped))
        .collect()
}

#[test]
fn a_routine_never_runs_for_a_time_before_it_was_armed() {
    let mut state = ScheduleState::default();
    let routines = [daily_nine()];

    // Created at 10:00: today's 09:00 is before it existed.
    assert!(due(at("2026-06-01T10:00:00Z"), &UTC, &routines, &mut state, true).is_empty());
    assert_eq!(state.get("a").unwrap().armed_at, at("2026-06-01T10:00:00Z"));

    assert_eq!(
        summary(&due(at("2026-06-02T09:00:30Z"), &UTC, &routines, &mut state, false)),
        [(Some("2026-06-02T09:00:00Z".to_owned()), RunTrigger::Schedule, false)]
    );

    assert!(
        due(at("2026-06-02T09:01:00Z"), &UTC, &routines, &mut state, false).is_empty(),
        "handled once"
    );
}

#[test]
fn missed_slots_are_caught_up_once_with_the_latest() {
    let mut state = ScheduleState::default();
    let routines = [daily_nine()];

    due(at("2026-06-01T10:00:00Z"), &UTC, &routines, &mut state, true);

    // The laptop was closed over three mornings.
    let caught = due(at("2026-06-04T12:00:00Z"), &UTC, &routines, &mut state, true);
    assert_eq!(
        summary(&caught),
        [(Some("2026-06-04T09:00:00Z".to_owned()), RunTrigger::CatchUp, false)]
    );

    assert!(due(at("2026-06-04T12:00:10Z"), &UTC, &routines, &mut state, false).is_empty());
    assert_eq!(state.get("a").unwrap().last_slot, Some(at("2026-06-04T09:00:00Z")));
}

#[test]
fn a_slot_handled_more_than_two_minutes_late_is_a_catch_up() {
    let mut state = ScheduleState::default();
    let routines = [daily_nine()];
    due(at("2026-06-01T10:00:00Z"), &UTC, &routines, &mut state, true);

    let late = due(at("2026-06-02T09:02:01Z"), &UTC, &routines, &mut state, false);
    assert_eq!(late[0].trigger, RunTrigger::CatchUp);
}

#[test]
fn a_clock_set_backwards_never_runs_a_handled_slot_again() {
    let mut state = ScheduleState::default();
    let routines = [daily_nine()];
    due(at("2026-06-01T10:00:00Z"), &UTC, &routines, &mut state, true);
    due(at("2026-06-02T09:00:05Z"), &UTC, &routines, &mut state, false);

    // Back by a day, then forward to the same slot again.
    assert!(due(at("2026-06-01T09:00:05Z"), &UTC, &routines, &mut state, false).is_empty());
    assert!(due(at("2026-06-02T09:00:30Z"), &UTC, &routines, &mut state, false).is_empty());

    assert_eq!(
        next_wake(at("2026-06-01T09:00:05Z"), &UTC, &routines, &state),
        Some(at("2026-06-03T09:00:00Z")),
        "the next wake is after the handled slot"
    );
}

#[test]
fn a_byte_change_arms_the_routine_again() {
    let mut state = ScheduleState::default();
    due(at("2026-06-01T08:00:00Z"), &UTC, &[daily_nine()], &mut state, true);

    // Edited at 08:59:59, before the slot: it still runs at 09:00.
    let edited = [routine("a", "v2", RoutineWhen::Daily { at: "09:00".to_owned() })];
    due(at("2026-06-01T08:59:59Z"), &UTC, &edited, &mut state, false);
    assert_eq!(state.get("a").unwrap().sha256, "v2");

    let ran = due(at("2026-06-01T09:00:00Z"), &UTC, &edited, &mut state, false);
    assert_eq!(ran.len(), 1);

    // Planning alone forgets nothing (the listing may be incomplete)...
    due(at("2026-06-01T09:30:00Z"), &UTC, &[], &mut state, false);
    assert!(state.get("a").is_some());

    // ...a routine no longer in a complete listing (turned off, pending, deleted) is forgotten.
    forget_unplanned(&mut state, &[]);
    assert!(state.get("a").is_none());
}

#[test]
fn a_busy_routines_slot_is_skipped_and_counts_as_handled() {
    let mut state = ScheduleState::default();
    let every = routine("b", "v1", RoutineWhen::EveryMinutes { every: 15 });
    due(
        at("2026-06-01T10:00:00Z"),
        &UTC,
        std::slice::from_ref(&every),
        &mut state,
        true,
    );

    let busy = Scheduled {
        busy: true,
        ..every.clone()
    };

    assert_eq!(
        summary(&due(at("2026-06-01T10:15:00Z"), &UTC, &[busy], &mut state, false)),
        [(Some("2026-06-01T10:15:00Z".to_owned()), RunTrigger::Schedule, true)]
    );

    assert!(
        due(
            at("2026-06-01T10:16:00Z"),
            &UTC,
            std::slice::from_ref(&every),
            &mut state,
            false
        )
        .is_empty()
    );
    assert_eq!(
        due(at("2026-06-01T10:30:00Z"), &UTC, &[every], &mut state, false).len(),
        1
    );
}

#[test]
fn app_start_routines_run_once_per_launch() {
    let mut state = ScheduleState::default();
    let start = [routine("c", "v1", RoutineWhen::AppStart)];

    assert_eq!(
        summary(&due(at("2026-06-01T10:00:00Z"), &UTC, &start, &mut state, true)),
        [(None, RunTrigger::AppStart, false)]
    );
    assert!(due(at("2026-06-01T10:00:01Z"), &UTC, &start, &mut state, false).is_empty());
    assert_eq!(next_wake(at("2026-06-01T10:00:01Z"), &UTC, &start, &state), None);
}

#[test]
fn team_app_start_routines_wait_until_the_plan_runs_them() {
    let mut state = ScheduleState::default();
    let local = routine("l", "v1", RoutineWhen::AppStart);
    let team = Scheduled {
        team: true,
        ..routine("t", "v1", RoutineWhen::AppStart)
    };
    let both = [local, team];
    let ids = |due: &[Due]| due.iter().map(|item| item.id.clone()).collect::<Vec<_>>();

    let launch = Launch {
        local: true,
        team: false,
    };
    assert_eq!(
        ids(&due(at("2026-06-01T10:00:00Z"), &UTC, &both, &mut state, launch)),
        ["l"]
    );

    let team_known = Launch {
        local: false,
        team: true,
    };
    assert_eq!(
        ids(&due(at("2026-06-01T10:01:00Z"), &UTC, &both, &mut state, team_known)),
        ["t"]
    );
}

#[test]
fn the_next_wake_is_the_earliest_next_slot() {
    let mut state = ScheduleState::default();
    let routines = [daily_nine(), routine("d", "v1", RoutineWhen::EveryHours { every: 6 })];
    due(at("2026-06-01T07:00:00Z"), &UTC, &routines, &mut state, true);

    assert_eq!(
        next_wake(at("2026-06-01T07:00:00Z"), &UTC, &routines, &state),
        Some(at("2026-06-01T09:00:00Z"))
    );
}

#[test]
fn the_schedule_file_round_trips_and_an_invalid_one_is_set_aside() {
    let temp = tempfile::tempdir().unwrap();
    let path = ScheduleState::path(temp.path());

    let mut state = ScheduleState::default();
    due(at("2026-06-01T10:00:00Z"), &UTC, &[daily_nine()], &mut state, true);
    state.save(&path).unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"armedAt\": \"2026-06-01T10:00:00Z\""), "{text}");
    assert!(text.contains("\"lastSlot\": null"), "{text}");
    assert_eq!(ScheduleState::load(&path).unwrap(), state);

    std::fs::write(&path, "{ not json").unwrap();
    assert_eq!(ScheduleState::load(&path).unwrap(), ScheduleState::default());

    let aside = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .any(|name| name.starts_with("routine-schedule.json.corrupt-"));
    assert!(aside, "the invalid file is kept aside");
}
