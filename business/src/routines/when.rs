//! Schedules as slots: the instants a routine is due (see "Slots" and "Daylight saving time" in
//! `docs/routines-format.md`).
//!
//! - Every time is wall-clock time in the machine's time zone, passed in by the caller
//!   ([`local_zone`] reads it from the operating system, again on every scheduler tick).
//! - `daily` and `weekdays`: `at` on each (Monday to Friday) local day. `cron`: every local minute
//!   that matches. A local time inside a spring-forward gap runs once right after the gap, shifted
//!   by the gap's length (`02:30` in a gap from 02:00 to 03:00 runs at 03:30); a time that happens
//!   twice runs at its first occurrence. Equal instants count once.
//! - `everyMinutes`, `everyHours`: `k * every` of elapsed time after the first instant of each
//!   local day, while before the next day's first instant, so the count restarts every day.
//! - `appStart` has no slots.

use jiff::civil::{Date, Weekday};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

use super::cron::CronExpr;
use super::format::{check_when, parse_at};
use crate::model::{RoutineWhen, SchedulePreview};

/// How many days a search for a slot looks at: a cron that fires only on 29 February may wait
/// eight years (2096 to 2104).
const MAX_SCAN_DAYS: usize = 8 * 366 + 2;

/// How many slots a preview shows.
pub const PREVIEW_SLOTS: usize = 5;

/// The machine's time zone now (UTC when the operating system does not say).
pub fn local_zone() -> TimeZone {
    TimeZone::try_system().unwrap_or(TimeZone::UTC)
}

/// `time` as RFC 3339 UTC to the second (`2026-10-18T09:00:00Z`).
pub fn rfc3339_utc(time: Timestamp) -> String {
    Timestamp::from_second(time.as_second()).unwrap_or(time).to_string()
}

/// A schedule ready to produce slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Schedule {
    /// At `hour:minute` every day, or Monday to Friday only (`weekdays`).
    At {
        /// 0 to 23.
        hour: i8,
        /// 0 to 59.
        minute: i8,
        /// Monday to Friday only.
        weekdays: bool,
    },
    /// Every `minutes` of elapsed time from the start of each local day.
    Every {
        /// 1 to 1440.
        minutes: i64,
    },
    /// The minutes a cron expression matches.
    Cron(CronExpr),
    /// No slots: once at launch.
    AppStart,
}

impl Schedule {
    /// The schedule of `when`, or why it is not valid.
    pub fn of(when: &RoutineWhen) -> Result<Self, String> {
        check_when(when)?;

        let at = |at: &str, weekdays: bool| {
            let (hour, minute) = parse_at(at).ok_or_else(|| "when.at must be a time like 09:00".to_owned())?;
            Ok(Self::At { hour, minute, weekdays })
        };

        match when {
            RoutineWhen::Daily { at: time } => at(time, false),
            RoutineWhen::Weekdays { at: time } => at(time, true),
            RoutineWhen::EveryMinutes { every } => Ok(Self::Every {
                minutes: i64::from(*every),
            }),
            RoutineWhen::EveryHours { every } => Ok(Self::Every {
                minutes: i64::from(*every) * 60,
            }),
            RoutineWhen::Cron { expression } => CronExpr::parse(expression).map(Self::Cron),
            RoutineWhen::AppStart => Ok(Self::AppStart),
        }
    }

    /// The slots of the local day `date` in `tz`, earliest first, each instant once.
    pub fn day_slots(&self, date: Date, tz: &TimeZone) -> Vec<Timestamp> {
        let mut slots = match self {
            Self::At { hour, minute, weekdays } => {
                let working = !matches!(date.weekday(), Weekday::Saturday | Weekday::Sunday);

                if *weekdays && !working {
                    Vec::new()
                } else {
                    local_instant(date, *hour, *minute, tz).into_iter().collect()
                }
            }
            Self::Every { minutes } => every_slots(date, *minutes, tz),
            Self::Cron(expr) => {
                let weekday = date.weekday().to_sunday_zero_offset();

                if expr.matches_date(date.month(), date.day(), weekday) {
                    expr.times()
                        .filter_map(|(hour, minute)| local_instant(date, hour, minute, tz))
                        .collect()
                } else {
                    Vec::new()
                }
            }
            Self::AppStart => Vec::new(),
        };

        slots.sort();
        slots.dedup();
        slots
    }

    /// The first slot after `after`.
    pub fn next_after(&self, after: Timestamp, tz: &TimeZone) -> Option<Timestamp> {
        if *self == Self::AppStart {
            return None;
        }

        // A day before: a gap may push a slot of that day past midnight.
        let mut date = tz.to_datetime(after).date().yesterday().ok()?;

        for _ in 0..MAX_SCAN_DAYS {
            let found = self.day_slots(date, tz).into_iter().find(|slot| *slot > after);

            if let Some(found) = found {
                // The next day's first slot can only be earlier after an odd shift: take the least.
                let next = date
                    .tomorrow()
                    .ok()
                    .and_then(|next| self.day_slots(next, tz).into_iter().find(|slot| *slot > after));

                return Some(next.map_or(found, |next| next.min(found)));
            }

            date = date.tomorrow().ok()?;
        }

        None
    }

    /// The latest slot at or before `at`.
    pub fn last_at_or_before(&self, at: Timestamp, tz: &TimeZone) -> Option<Timestamp> {
        if *self == Self::AppStart {
            return None;
        }

        // A day after: a fold or an odd shift may resolve a slot of that day before midnight.
        let mut date = tz.to_datetime(at).date().tomorrow().ok()?;

        for _ in 0..MAX_SCAN_DAYS {
            let found = self.day_slots(date, tz).into_iter().rev().find(|slot| *slot <= at);

            if let Some(found) = found {
                let before = date
                    .yesterday()
                    .ok()
                    .and_then(|before| self.day_slots(before, tz).into_iter().rev().find(|slot| *slot <= at));

                return Some(before.map_or(found, |before| before.max(found)));
            }

            date = date.yesterday().ok()?;
        }

        None
    }
}

/// The instant of the local time `hour:minute` on `date` in `tz`: a time in a gap is shifted
/// forward by the gap, a time that happens twice is its first occurrence.
fn local_instant(date: Date, hour: i8, minute: i8, tz: &TimeZone) -> Option<Timestamp> {
    tz.to_ambiguous_timestamp(date.at(hour, minute, 0, 0)).compatible().ok()
}

/// `k * minutes` of elapsed time after the first instant of `date`, before the next day's.
fn every_slots(date: Date, minutes: i64, tz: &TimeZone) -> Vec<Timestamp> {
    let Some(start) = local_instant(date, 0, 0, tz) else {
        return Vec::new();
    };

    let Some(end) = date.tomorrow().ok().and_then(|next| local_instant(next, 0, 0, tz)) else {
        return Vec::new();
    };

    let step = SignedDuration::from_mins(minutes);
    let mut slots = Vec::new();
    let mut slot = start;

    while slot < end {
        slots.push(slot);

        match slot.checked_add(step) {
            Ok(next) => slot = next,
            Err(_) => break,
        }
    }

    slots
}

/// The next [`PREVIEW_SLOTS`] slots of `when` after `now` in `tz` (RFC 3339 UTC), or why it is
/// not valid. An app start routine has no slots.
pub fn schedule_preview(when: &RoutineWhen, now: Timestamp, tz: &TimeZone) -> SchedulePreview {
    let schedule = match Schedule::of(when) {
        Ok(schedule) => schedule,
        Err(problem) => {
            return SchedulePreview {
                next: Vec::new(),
                problem: Some(problem),
            };
        }
    };

    let mut next = Vec::new();
    let mut after = now;

    while next.len() < PREVIEW_SLOTS {
        let Some(slot) = schedule.next_after(after, tz) else {
            break;
        };

        next.push(rfc3339_utc(slot));
        after = slot;
    }

    SchedulePreview { next, problem: None }
}
