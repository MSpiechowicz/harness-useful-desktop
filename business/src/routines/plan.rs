//! Which routines are due at a moment, and when to look again (see "Catch-up and the schedule
//! file" in `docs/routines-format.md`). Pure: the caller passes the time, the time zone, the
//! runnable routines and the schedule state, and saves the state when [`due`] changed it.
//!
//! - Only runnable routines are planned (valid, approved, on; a team routine turned on here and
//!   trusted or approved). A routine is armed (`armedAt`) the first time it is seen runnable with
//!   its bytes, and armed again when its bytes change; a routine that is no longer runnable or no
//!   longer there is forgotten ([`forget_unplanned`]), so it is armed again when it comes back
//!   (enabled, approved). The caller forgets only against a listing it trusts: a folder or an
//!   approvals file that could not be read forgets nothing.
//! - A routine's next slot is the latest slot in `(max(lastSlot, armedAt), now]`. Handled within
//!   [`ON_TIME`] it runs as `schedule`; later (the machine slept, the app was closed, the clock
//!   jumped) it runs once as `catchUp` and the older missed slots are dropped. Either way it
//!   becomes `lastSlot`, so a clock set backwards never runs a slot at or before it again.
//! - A slot that is due while the routine still runs (or waits for a free place) is skipped: it
//!   counts as handled and comes back with `skipped` for the log.
//! - `appStart` routines run when [`Launch`] says so, once per launch: the caller sets it only for
//!   the first plan after the routines were loaded, for the team routines once it knows whether
//!   they are shared.

use std::collections::BTreeSet;

use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};

use super::schedule_state::ScheduleState;
use super::view::on_schedule;
use super::when::Schedule;
use crate::model::{HookOrigin, RoutineView, RoutineWhen, RunTrigger};

/// How late a slot may be handled and still run as `schedule`.
pub const ON_TIME: SignedDuration = SignedDuration::from_mins(2);

/// A runnable routine the scheduler plans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheduled {
    /// The routine id.
    pub id: String,
    /// The SHA-256 of its bytes (a change arms it again).
    pub sha256: String,
    /// When it runs.
    pub when: RoutineWhen,
    /// A run of it is going on or waits for a free place: a due slot is skipped.
    pub busy: bool,
    /// A team routine (see [`Launch::team`]).
    pub team: bool,
}

/// Which `appStart` routines a plan runs: the local ones (`local`), the team ones (`team`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Launch {
    /// The first plan of this launch: local `appStart` routines run.
    pub local: bool,
    /// Team `appStart` routines run (the first plan that knew whether they are shared).
    pub team: bool,
}

impl From<bool> for Launch {
    /// Both or neither.
    fn from(launch: bool) -> Self {
        Self {
            local: launch,
            team: launch,
        }
    }
}

impl Launch {
    fn of(self, routine: &Scheduled) -> bool {
        if routine.team { self.team } else { self.local }
    }
}

/// A routine that is due.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    /// The routine id.
    pub id: String,
    /// The slot it is due for, `None` for app start.
    pub slot: Option<Timestamp>,
    /// `schedule`, `catchUp` or `appStart`.
    pub trigger: RunTrigger,
    /// The routine is busy: the slot is handled without a run (log it).
    pub skipped: bool,
}

/// The routines of `views` (a listing, local and team) the scheduler plans: those on their
/// schedule ([`on_schedule`]), busy while their id is in `busy` (running or waiting for a place).
pub fn scheduled_from(views: &[RoutineView], busy: &BTreeSet<String>) -> Vec<Scheduled> {
    views
        .iter()
        .filter(|view| on_schedule(view))
        .filter_map(|view| {
            let routine = view.routine.as_ref()?;

            Some(Scheduled {
                id: view.id.clone(),
                sha256: view.sha256.clone(),
                when: routine.when.clone(),
                busy: busy.contains(&view.id),
                team: view.origin == HookOrigin::Team,
            })
        })
        .collect()
}

/// Forgets the schedule entries of the routines `routines` no longer has (turned off, pending,
/// deleted). Call it only with a complete listing.
pub fn forget_unplanned(state: &mut ScheduleState, routines: &[Scheduled]) {
    state.retain(|id| routines.iter().any(|routine| routine.id == id));
}

/// The routines of `routines` due at `now` in `tz`, by slot (app start ones first), and the
/// schedule state updated: routines armed and their slots handled. `launch`: which app start
/// routines run (see [`Launch`]).
pub fn due(
    now: Timestamp,
    tz: &TimeZone,
    routines: &[Scheduled],
    state: &mut ScheduleState,
    launch: impl Into<Launch>,
) -> Vec<Due> {
    let launch = launch.into();
    let mut due = Vec::new();

    for routine in routines {
        let armed = state
            .get(&routine.id)
            .is_some_and(|entry| entry.sha256 == routine.sha256);

        if !armed {
            state.arm(&routine.id, &routine.sha256, now);
        }

        let Ok(schedule) = Schedule::of(&routine.when) else {
            continue;
        };

        if schedule == Schedule::AppStart {
            if launch.of(routine) {
                due.push(Due {
                    id: routine.id.clone(),
                    slot: None,
                    trigger: RunTrigger::AppStart,
                    skipped: routine.busy,
                });
            }

            continue;
        }

        let Some(entry) = state.get_mut(&routine.id) else {
            continue;
        };

        let floor = entry.last_slot.map_or(entry.armed_at, |last| last.max(entry.armed_at));

        let Some(slot) = schedule.last_at_or_before(now, tz).filter(|slot| *slot > floor) else {
            continue;
        };

        entry.last_slot = Some(slot);

        let trigger = if now.duration_since(slot) <= ON_TIME {
            RunTrigger::Schedule
        } else {
            RunTrigger::CatchUp
        };

        due.push(Due {
            id: routine.id.clone(),
            slot: Some(slot),
            trigger,
            skipped: routine.busy,
        });
    }

    due.sort_by_key(|item| item.slot);
    due
}

/// When the scheduler should look again: the earliest next slot of `routines` after `now` (or
/// after what was handled, when the clock is behind it), `None` when no routine has one. The
/// caller caps the wait (the zone or the clock may change meanwhile).
pub fn next_wake(now: Timestamp, tz: &TimeZone, routines: &[Scheduled], state: &ScheduleState) -> Option<Timestamp> {
    routines
        .iter()
        .filter_map(|routine| {
            let schedule = Schedule::of(&routine.when).ok()?;

            let handled = state
                .get(&routine.id)
                .filter(|entry| entry.sha256 == routine.sha256)
                .map(|entry| entry.last_slot.map_or(entry.armed_at, |last| last.max(entry.armed_at)));

            let after = handled.map_or(now, |handled| handled.max(now));

            schedule.next_after(after, tz)
        })
        .min()
}
