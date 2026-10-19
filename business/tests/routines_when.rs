//! Schedules as slots in the machine's time zone (see "Slots" and "Daylight saving time" in
//! `docs/routines-format.md`): spring-forward gaps run once right after the gap, fall-back folds
//! at the first occurrence (Europe/Warsaw, America/New_York, and Australia/Lord_Howe's 30-minute
//! shift), every N counted in elapsed time from local midnight, and the schedule preview.

use harness_business::model::RoutineWhen;
use harness_business::routines::when::{Schedule, schedule_preview};
use jiff::Timestamp;
use jiff::civil::date;
use jiff::tz::TimeZone;
use pretty_assertions::assert_eq;

fn zone(name: &str) -> TimeZone {
    TimeZone::get(name).unwrap()
}

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn daily(time: &str) -> Schedule {
    Schedule::of(&RoutineWhen::Daily { at: time.to_owned() }).unwrap()
}

fn cron(expression: &str) -> Schedule {
    Schedule::of(&RoutineWhen::Cron {
        expression: expression.to_owned(),
    })
    .unwrap()
}

/// The next `count` slots of `schedule` after `from`.
fn next(schedule: &Schedule, from: &str, tz: &TimeZone, count: usize) -> Vec<String> {
    let mut after = at(from);
    let mut slots = Vec::new();

    for _ in 0..count {
        after = schedule.next_after(after, tz).unwrap();
        slots.push(after.to_string());
    }

    slots
}

#[test]
fn warsaw_spring_gap_runs_once_after_the_gap_and_fall_back_at_the_first_time() {
    let warsaw = zone("Europe/Warsaw");

    assert_eq!(
        next(&daily("02:30"), "2026-03-28T00:00:00Z", &warsaw, 3),
        [
            "2026-03-28T01:30:00Z", // 02:30 CET
            "2026-03-29T01:30:00Z", // 02:30 does not exist: 03:30 CEST
            "2026-03-30T00:30:00Z", // 02:30 CEST
        ]
    );

    assert_eq!(
        next(&daily("02:30"), "2026-10-24T12:00:00Z", &warsaw, 2),
        [
            "2026-10-25T00:30:00Z", // the first 02:30 (CEST), not the second (01:30Z)
            "2026-10-26T01:30:00Z",
        ]
    );
}

#[test]
fn new_york_follows_its_own_transitions() {
    let new_york = zone("America/New_York");

    assert_eq!(
        next(&daily("02:30"), "2026-03-08T00:00:00Z", &new_york, 1),
        ["2026-03-08T07:30:00Z"] // 03:30 EDT
    );
    assert_eq!(
        next(&daily("01:30"), "2026-11-01T00:00:00Z", &new_york, 2),
        ["2026-11-01T05:30:00Z", "2026-11-02T06:30:00Z"] // EDT once, then EST
    );
}

#[test]
fn lord_howe_shifts_by_its_thirty_minute_gap() {
    let lord_howe = zone("Australia/Lord_Howe");

    // 2026-10-04 02:00 jumps to 02:30: 02:15 runs at 02:45 (+11:00).
    assert_eq!(
        next(&daily("02:15"), "2026-10-03T12:00:00Z", &lord_howe, 1),
        ["2026-10-03T15:45:00Z"]
    );

    // 2026-04-05 02:00 falls back to 01:30: 01:45 runs once, at its first time (+11:00).
    assert_eq!(
        next(&daily("01:45"), "2026-04-04T12:00:00Z", &lord_howe, 2),
        ["2026-04-04T14:45:00Z", "2026-04-05T15:15:00Z"]
    );
}

#[test]
fn every_minutes_restart_at_local_midnight() {
    let every = Schedule::of(&RoutineWhen::EveryMinutes { every: 7 }).unwrap();
    let utc = TimeZone::UTC;

    let day = every.day_slots(date(2026, 6, 1), &utc);
    assert_eq!(day.len(), 206);
    assert_eq!(day.last().unwrap().to_string(), "2026-06-01T23:55:00Z");

    assert_eq!(
        next(&every, "2026-06-01T23:55:00Z", &utc, 2),
        ["2026-06-02T00:00:00Z", "2026-06-02T00:07:00Z"]
    );
}

#[test]
fn every_hours_count_elapsed_time_through_a_short_day() {
    let every = Schedule::of(&RoutineWhen::EveryHours { every: 5 }).unwrap();
    let warsaw = zone("Europe/Warsaw");

    let slots: Vec<String> = every
        .day_slots(date(2026, 3, 29), &warsaw)
        .iter()
        .map(ToString::to_string)
        .collect();

    // 00:00 CET, then 06:00, 11:00, 16:00 and 21:00 CEST: five hours apart each.
    assert_eq!(
        slots,
        [
            "2026-03-28T23:00:00Z",
            "2026-03-29T04:00:00Z",
            "2026-03-29T09:00:00Z",
            "2026-03-29T14:00:00Z",
            "2026-03-29T19:00:00Z",
        ]
    );

    let daily_once = Schedule::of(&RoutineWhen::EveryHours { every: 24 }).unwrap();
    assert_eq!(daily_once.day_slots(date(2026, 3, 29), &warsaw).len(), 1);
}

#[test]
fn cron_minutes_in_a_gap_or_a_fold_fire_once_per_instant() {
    let warsaw = zone("Europe/Warsaw");
    let quarter = cron("*/15 * * * *");

    let spring = quarter.day_slots(date(2026, 3, 29), &warsaw);
    assert_eq!(spring.len(), 23 * 4, "the gap's minutes land on 03:xx, once each");

    let fall = quarter.day_slots(date(2026, 10, 25), &warsaw);
    assert_eq!(fall.len(), 24 * 4, "the repeated hour fires once");

    assert_eq!(
        next(&cron("30 2 * * *"), "2026-03-29T00:00:00Z", &warsaw, 1),
        ["2026-03-29T01:30:00Z"]
    );
}

#[test]
fn weekdays_skip_the_weekend_and_a_leap_day_cron_waits_for_one() {
    let weekdays = Schedule::of(&RoutineWhen::Weekdays { at: "08:00".to_owned() }).unwrap();

    // 2026-10-16 is a Friday.
    assert_eq!(
        next(&weekdays, "2026-10-16T09:00:00Z", &TimeZone::UTC, 2),
        ["2026-10-19T08:00:00Z", "2026-10-20T08:00:00Z"]
    );

    assert_eq!(
        next(&cron("0 0 29 2 *"), "2026-03-01T00:00:00Z", &TimeZone::UTC, 1),
        ["2028-02-29T00:00:00Z"]
    );
}

#[test]
fn the_latest_slot_is_found_backwards() {
    let warsaw = zone("Europe/Warsaw");

    assert_eq!(
        daily("09:00").last_at_or_before(at("2026-06-03T06:59:59Z"), &warsaw),
        Some(at("2026-06-02T07:00:00Z"))
    );
    assert_eq!(
        daily("09:00").last_at_or_before(at("2026-06-03T07:00:00Z"), &warsaw),
        Some(at("2026-06-03T07:00:00Z"))
    );
    assert_eq!(
        Schedule::of(&RoutineWhen::AppStart)
            .unwrap()
            .last_at_or_before(at("2026-06-03T07:00:00Z"), &warsaw),
        None
    );
}

#[test]
fn a_preview_lists_the_next_runs_or_the_problem() {
    let preview = schedule_preview(
        &RoutineWhen::Daily { at: "09:00".to_owned() },
        at("2026-06-01T10:00:00Z"),
        &TimeZone::UTC,
    );

    assert_eq!(preview.problem, None);
    assert_eq!(
        preview.next,
        [
            "2026-06-02T09:00:00Z",
            "2026-06-03T09:00:00Z",
            "2026-06-04T09:00:00Z",
            "2026-06-05T09:00:00Z",
            "2026-06-06T09:00:00Z",
        ]
    );

    let broken = schedule_preview(
        &RoutineWhen::Cron {
            expression: "0 0 31 2 *".to_owned(),
        },
        at("2026-06-01T10:00:00Z"),
        &TimeZone::UTC,
    );
    assert!(broken.next.is_empty() && broken.problem.is_some());

    let start = schedule_preview(&RoutineWhen::AppStart, at("2026-06-01T10:00:00Z"), &TimeZone::UTC);
    assert_eq!((start.next.len(), start.problem), (0, None));
}
