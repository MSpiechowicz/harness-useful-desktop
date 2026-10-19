//! The scheduler task (see "Catch-up and the schedule file" in `docs/routines-format.md`).
//!
//! - The first look (the records the last session left `running` were stopped at launch) runs
//!   app start routines and catches each missed routine up once.
//! - Each look ([`RoutineRuntime::tick`]) plans the runnable routines at the wall-clock time in the
//!   machine's time zone, read again every time ([`harness_business::routines::due`]), starts or
//!   queues the due ones and logs the skipped ones (busy), and saves the schedule file when it
//!   changed.
//! - A look whose listing cannot be trusted (the routines folder or the approvals could not be
//!   read) plans nothing, forgets nothing and saves nothing; the launch look is then still to
//!   come. While Harness Memory could not say whether team routines are shared, nothing is
//!   forgotten either, and team app start routines wait for the first look that knows.
//! - After the team routines changed (a refresh, sharing off, a routine turned off) the next look
//!   first stops the team runs that may no longer run ([`RoutineRuntime::reconcile_team`]).
//! - Then it sleeps until the next slot, at most [`MAX_SLEEP`] (the zone or the clock may change,
//!   the machine may sleep), or until a change of the folder, the approvals or a write wakes it.
//!
//! An unreadable schedule file counts as empty: every routine is armed again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, MutexGuard, PoisonError};
use std::time::Duration;

use harness_business::model::{RoutineView, RunTrigger};
use harness_business::routines::{
    Due, Launch, ScheduleState, Scheduled, due, forget_unplanned, local_zone, next_wake, scheduled_from,
};
use jiff::Timestamp;
use jiff::tz::TimeZone;

use super::RoutineRuntime;
use super::queue::RunClass;

/// Longest sleep between two looks.
pub const MAX_SLEEP: Duration = Duration::from_secs(30);

/// Shortest sleep between two looks (a slot that is due now was handled by the look before).
const MIN_SLEEP: Duration = Duration::from_millis(250);

/// The schedule as of the latest look.
#[derive(Default)]
pub struct SchedulePlan {
    /// The schedule file, loaded at the first look.
    state: Option<ScheduleState>,
    /// The routines planned at the latest look.
    scheduled: Vec<Scheduled>,
    /// The launch look ran before Harness Memory said whether team routines are shared: their app
    /// start routines run at the first look that knows.
    team_launch_pending: bool,
}

impl SchedulePlan {
    /// Which app start routines a look runs (`launch`: the launch look; `team_known`: whether
    /// team routines are shared is known).
    fn launch(&mut self, launch: bool, team_known: bool) -> Launch {
        if launch && !team_known {
            self.team_launch_pending = true;
        }

        let team = team_known && (launch || std::mem::take(&mut self.team_launch_pending));

        Launch { local: launch, team }
    }
}

impl RoutineRuntime {
    fn lock_plan(&self) -> MutexGuard<'_, SchedulePlan> {
        // Every change leaves a complete value, so a panic cannot leave it half-written.
        self.plan.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn schedule_path(&self) -> Option<PathBuf> {
        self.local_data.as_deref().map(ScheduleState::path)
    }

    /// One look at `now` in `tz` (`launch`: the first look of this launch, until one went
    /// through): the due routines are started or queued, the busy ones skipped. Returns what was
    /// due; `None` when the routines could not be read (nothing planned, the launch look still to
    /// come).
    pub async fn tick(self: &Arc<Self>, now: Timestamp, tz: &TimeZone, launch: bool) -> Option<Vec<Due>> {
        let Some(path) = self.schedule_path() else {
            return Some(Vec::new());
        };

        if self.team_changed.swap(false, Ordering::SeqCst) {
            self.reconcile_team().await;
        }

        let views = self.runnable_views().await?;
        let scheduled = scheduled_from(&views, &self.busy());
        let team_known = self.team.known();

        self.load_schedule(&path).await;

        let (planned, changed) = {
            let mut plan = self.lock_plan();
            let launch = plan.launch(launch, team_known);
            let state = plan.state.get_or_insert_with(ScheduleState::default);
            let before = state.clone();

            // While team routines may be missing from the listing, none is forgotten.
            if team_known {
                forget_unplanned(state, &scheduled);
            }

            let planned = due(now, tz, &scheduled, state, launch);
            let changed = (*state != before).then(|| state.clone());
            plan.scheduled = scheduled;

            (planned, changed)
        };

        if let Some(state) = changed {
            self.save_schedule(path, state).await;
        }

        let classes: HashMap<&str, RunClass> = views
            .iter()
            .filter_map(|view| Some((view.id.as_str(), RunClass::of(&view.routine.as_ref()?.action))))
            .collect();

        for item in &planned {
            if item.skipped {
                tracing::info!(
                    routine = item.id,
                    trigger = item.trigger.as_str(),
                    "routine slot skipped: it still runs"
                );
                continue;
            }

            if let Some(class) = classes.get(item.id.as_str()) {
                self.request(&item.id, *class, item.trigger);
            }
        }

        Some(planned)
    }

    /// How long to sleep after a look at `now` in `tz`: until the next slot, within
    /// [`MIN_SLEEP`] and [`MAX_SLEEP`].
    pub fn sleep_after(&self, now: Timestamp, tz: &TimeZone) -> Duration {
        let plan = self.lock_plan();

        let next = plan
            .state
            .as_ref()
            .and_then(|state| next_wake(now, tz, &plan.scheduled, state));

        let until = next
            .and_then(|at| Duration::try_from(at.duration_since(now)).ok())
            .unwrap_or(MAX_SLEEP);

        until.clamp(MIN_SLEEP, MAX_SLEEP)
    }

    /// Forgets the schedule entries of `ids` (their routines were deleted).
    pub async fn forget_schedule(&self, ids: &[String]) {
        let Some(path) = self.schedule_path() else {
            return;
        };

        self.load_schedule(&path).await;

        let changed = {
            let mut plan = self.lock_plan();
            let state = plan.state.get_or_insert_with(ScheduleState::default);
            let forgot = ids.iter().fold(false, |forgot, id| state.forget(id) | forgot);

            forgot.then(|| state.clone())
        };

        if let Some(state) = changed {
            self.save_schedule(path, state).await;
        }
    }

    /// Reads the schedule file at `path` unless it was read already.
    async fn load_schedule(&self, path: &Path) {
        if self.lock_plan().state.is_some() {
            return;
        }

        let loaded = load_state(path.to_path_buf()).await;
        self.lock_plan().state.get_or_insert(loaded);
    }

    /// The routines the scheduler may run: the local routine files and the team routines on their
    /// schedule here (each is checked again when a run starts). `None` when the folder or the
    /// approvals could not be read.
    async fn runnable_views(&self) -> Option<Vec<RoutineView>> {
        match self.schedule_views().await {
            Ok(Some(views)) => Some(views),
            Ok(None) => {
                tracing::warn!("routines not planned: their folder or approvals could not be read");
                None
            }
            Err(error) => {
                tracing::warn!(code = ?error.code(), "routines not read for the schedule");
                None
            }
        }
    }

    async fn save_schedule(&self, path: PathBuf, state: ScheduleState) {
        let saved = self.locked(move || state.save(&path)).await;

        if let Err(error) = saved {
            tracing::warn!(code = ?error.code(), "routine schedule not saved");
        }
    }
}

/// The schedule file at `path`; empty when it cannot be read.
async fn load_state(path: PathBuf) -> ScheduleState {
    let loaded = tauri::async_runtime::spawn_blocking(move || ScheduleState::load(&path)).await;

    match loaded {
        Ok(Ok(state)) => state,
        Ok(Err(error)) => {
            tracing::warn!(code = ?error.code(), "routine schedule not read: every routine is armed again");
            ScheduleState::default()
        }
        Err(_) => ScheduleState::default(),
    }
}

/// Starts the scheduler task (it runs until the app exits; the leftover records were stopped at
/// launch): it looks at the routines (the first look with `launch`) and sleeps in turn.
pub fn spawn(runtime: Arc<RoutineRuntime>) {
    drop(tauri::async_runtime::spawn(async move {
        let mut launch = true;

        loop {
            let tz = local_zone();

            // The launch look counts once it went through.
            if let Some(planned) = runtime.tick(Timestamp::now(), &tz, launch).await {
                launch = false;
                log_planned(&planned);
            }

            let sleep = runtime.sleep_after(Timestamp::now(), &tz);

            tokio::select! {
                () = tokio::time::sleep(sleep) => {}
                () = runtime.wake.notified() => {}
            }
        }
    }));
}

fn log_planned(planned: &[Due]) {
    if planned.is_empty() {
        return;
    }

    let catch_up = planned
        .iter()
        .filter(|item| item.trigger == RunTrigger::CatchUp)
        .count();
    tracing::debug!(due = planned.len(), catch_up, "routines planned");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_app_start_routines_run_at_the_first_look_that_knows_about_sharing() {
        let mut plan = SchedulePlan::default();

        let launch = plan.launch(true, false);
        assert_eq!((launch.local, launch.team), (true, false), "sharing unknown at launch");

        let unknown = plan.launch(false, false);
        assert_eq!((unknown.local, unknown.team), (false, false), "still unknown");

        let known = plan.launch(false, true);
        assert_eq!((known.local, known.team), (false, true), "known now: team ones run");

        let after = plan.launch(false, true);
        assert_eq!((after.local, after.team), (false, false), "once per launch");

        let mut known_at_launch = SchedulePlan::default();
        assert_eq!(
            known_at_launch.launch(true, true),
            Launch {
                local: true,
                team: true
            }
        );
        assert_eq!(known_at_launch.launch(false, true), Launch::default());
    }
}
