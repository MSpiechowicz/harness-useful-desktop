//! Starts and tracks routine runs (see "How a run starts" in `docs/routines-format.md`).
//!
//! - One run per routine: a routine waiting for a place or running is busy, its due slots are
//!   skipped and "Run now" is refused.
//! - At most 2 agent and 4 command or script runs at once; the others wait in the order they were
//!   due ([`Runs`]) and are recorded when they start.
//! - A run re-reads its routine when it starts and goes ahead only while it may still run (valid,
//!   approved, and on for a scheduled trigger; a team routine turned on here and trusted or
//!   approved). A project routine's folder must still be that project, and a team project
//!   routine's folder still a copy of its repository, and a team routine that runs on trust alone
//!   still in the team database with the bytes of its copy, else the run fails.
//! - Each run is written when it starts and when it ends, the Routines view hears both
//!   (`runStarted`, `runFinished`), and the end is notified ([`super::notify`]).
//! - Stopping a run (the user, a deleted routine, a team routine no longer allowed here, app exit)
//!   signals it, also while it is still starting; a command run drops its work at once, which
//!   kills the process and its process group, and a script run cancels its request so that
//!   Harness Memory stops the script ([`script_run`]). Once the app closes nothing starts, and no run
//!   is taken in for a routine while it is being deleted.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, MutexGuard, PoisonError};
use std::time::Duration;

use harness_business::model::{
    HookOrigin, HookScope, Routine, RoutineAction, RoutineRun, RoutineRunsPage, RoutineRunsQuery, RoutineStatus,
    RoutineView, RoutinesEvent, RunTrigger,
};
use harness_business::routines::runs::{page_of, started_run};
use harness_business::routines::view::{LiveRoutines, on_schedule, project_problem};
use harness_business::routines::{ensure_routine_workspace, workspace_name};
use harness_business::{BusinessError, Result};
use jiff::Timestamp;
use tokio::sync::watch;

use super::job::{RunJob, RunOutcome, StopReason, StopSignal};
use super::notify::Alert;
use super::progress::RunProgress;
use super::queue::{Reserved, RunClass, Runs};
use super::{RoutineRuntime, command_run, script_run};

/// Longest wait for stopped runs to end before their routine is deleted or forgotten.
pub const STOP_BUDGET: Duration = Duration::from_secs(5);

/// "Run now" of a routine whose run goes on.
pub const ALREADY_RUNNING: &str = "the routine is already running";

/// "Run now" of a routine whose run waits for a free place.
pub const ALREADY_QUEUED: &str = "the routine is already waiting for a free place";

/// "Run now" of a paused team routine.
pub const PAUSED_MESSAGE: &str = "The project folder is missing or no longer the routine's project";

impl RoutineRuntime {
    pub(super) fn lock_runs(&self) -> MutexGuard<'_, Runs> {
        // Every change leaves the maps complete, so a panic cannot leave them half-written.
        self.runs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The routines waiting or running.
    pub fn busy(&self) -> BTreeSet<String> {
        self.lock_runs().busy()
    }

    /// The routines with a run going on.
    pub fn running(&self) -> BTreeSet<String> {
        self.lock_runs().running()
    }

    /// The routines with a run going on or waiting for a free place, for listings.
    pub fn live_routines(&self) -> LiveRoutines {
        let runs = self.lock_runs();

        LiveRoutines {
            running: runs.running(),
            queued: runs.queued(),
        }
    }

    /// One page of the runs going on (`query.status` is `running`), from memory: no record file
    /// is read.
    pub fn live_page(&self, query: &RoutineRunsQuery) -> RoutineRunsPage {
        page_of(self.lock_runs().live_records(), query)
    }

    /// `runs` with whether each waits for an answer filled in.
    pub fn with_needs_you(&self, mut runs: Vec<RoutineRun>) -> Vec<RoutineRun> {
        let live = self.lock_runs();

        for run in &mut runs {
            run.needs_you = live.needs_you(&run.record.id);
        }

        runs
    }

    /// Asks for a run of routine `id` (`class`, `trigger`): started now when a place is free,
    /// else queued (the Routines view hears `changed`).
    /// Refused while the app closes or the routine is being deleted.
    pub fn request(self: &Arc<Self>, id: &str, class: RunClass, trigger: RunTrigger) -> Reserved {
        let reserved = self.lock_runs().reserve(id, class, trigger);

        if reserved == Reserved::Queued {
            self.emit(RoutinesEvent::Changed);
        }

        if reserved == Reserved::Now {
            let runtime = self.clone();
            let id = id.to_owned();

            drop(tauri::async_runtime::spawn(async move {
                runtime.launch(&id, trigger).await;
            }));
        }

        reserved
    }

    /// "Run now": a valid, approved routine runs at once when a place is free (the run returned),
    /// else it waits (`None`). `conflict` while it runs ([`ALREADY_RUNNING`]) or waits for a place
    /// ([`ALREADY_QUEUED`]), `invalidInput` for a pending, invalid or paused routine (a paused
    /// local one with its reason), `notFound` for a missing one.
    pub async fn run_now(self: &Arc<Self>, id: &str) -> Result<Option<RoutineRun>> {
        let view = self.routine_view(id).await?;

        if view.status == RoutineStatus::Paused {
            let problem = view.problem.unwrap_or_else(|| PAUSED_MESSAGE.to_owned());
            return Err(BusinessError::InvalidInput(problem));
        }

        let Some(routine) = view.routine.as_ref().filter(|_| view.can_run_now) else {
            return Err(BusinessError::InvalidInput(
                "only a valid, approved routine can run".to_owned(),
            ));
        };

        let reserved = self
            .lock_runs()
            .reserve(id, RunClass::of(&routine.action), RunTrigger::Manual);

        match reserved {
            Reserved::Busy if self.lock_runs().queued().contains(id) => {
                Err(BusinessError::Conflict(ALREADY_QUEUED.to_owned()))
            }
            Reserved::Busy => Err(BusinessError::Conflict(ALREADY_RUNNING.to_owned())),
            Reserved::Refused => Err(BusinessError::Conflict("the routine can no longer run".to_owned())),
            Reserved::Queued => {
                self.emit(RoutinesEvent::Changed);
                Ok(None)
            }
            Reserved::Now => match self.launch(id, RunTrigger::Manual).await {
                Some(run) => Ok(Some(run)),
                None => Err(BusinessError::Conflict("the routine can no longer run".to_owned())),
            },
        }
    }

    /// Starts the run of routine `id`, whose place is taken: records it and carries it out in the
    /// background. `None` (the place freed) when the routine may no longer run or the record
    /// cannot be written.
    async fn launch(self: &Arc<Self>, id: &str, trigger: RunTrigger) -> Option<RoutineRun> {
        let view = self.routine_view(id).await.ok().filter(|view| may_start(view, trigger));
        let team_copy = view.clone().filter(|view| view.origin == HookOrigin::Team);

        let Some((routine, origin, project)) = view.and_then(run_target) else {
            tracing::info!(
                routine = id,
                trigger = trigger.as_str(),
                "routine run dropped: it may no longer run"
            );
            self.release(id);
            return None;
        };

        if let Some(reason) = self.lock_runs().stop_before_start(id) {
            tracing::info!(
                routine = id,
                ?reason,
                "routine run dropped: it was stopped while starting"
            );
            self.release(id);
            return None;
        }

        let run = started_run(&routine, origin, trigger, project, Timestamp::now());

        let started = run.clone();
        if let Err(error) = self.record(move |runs| runs.start(&started)).await {
            tracing::warn!(routine = id, code = ?error.code(), "routine run not started: its record was not written");
            self.release(id);
            return None;
        }

        let (stop, stopped) = watch::channel(None);

        // A stop asked for meanwhile (app exit, a delete) reaches the run at once.
        self.lock_runs().set_live(&run, stop);

        tracing::info!(
            routine = id,
            run = run.record.id,
            trigger = trigger.as_str(),
            "routine run started"
        );
        self.emit(RoutinesEvent::RunStarted { run: run.clone() });

        let runtime = self.clone();
        let job = (routine, run.clone(), team_copy);

        drop(tauri::async_runtime::spawn(async move {
            let (routine, run, team_copy) = job;
            let outcome = runtime
                .perform(routine, run.clone(), team_copy, StopSignal(stopped))
                .await;
            runtime.finish(&run, outcome).await;
        }));

        Some(run)
    }

    /// Carries out `run` of `routine` until it ends or `stop` fires (nothing starts when it fired
    /// already). `team_copy`: the view of a team routine, checked against the team database first.
    async fn perform(
        self: &Arc<Self>,
        routine: Routine,
        run: RoutineRun,
        team_copy: Option<RoutineView>,
        stop: StopSignal,
    ) -> RunOutcome {
        if let Some(reason) = stop.fired() {
            return RunOutcome::stopped(reason);
        }

        if let Some(view) = &team_copy
            && let Err(reason) = self.confirm_team_copy(view).await
        {
            return RunOutcome::failed(reason);
        }

        let root = self.store.as_ref().map(|store| store.root().to_path_buf());
        let (checked, origin) = (routine.clone(), run.record.origin);
        let project = run.record.project.clone();
        let folder = tauri::async_runtime::spawn_blocking(move || {
            run_folder(&checked, origin, project.as_deref(), root.as_deref())
        })
        .await;

        let folder = match folder {
            Ok(Ok(folder)) => folder,
            Ok(Err(reason)) => return RunOutcome::failed(reason),
            Err(_) => return RunOutcome::failed("The run could not start"),
        };

        if let (HookOrigin::Team, Some(folder)) = (run.record.origin, folder.as_deref())
            && routine.scope == HookScope::Project
            && let Err(reason) = self.confirm_team_folder(&routine, folder).await
        {
            return RunOutcome::failed(reason);
        }

        let job = RunJob { routine, run, folder };

        match job.routine.action.clone() {
            RoutineAction::Agent { .. } => {
                let progress = RunProgress {
                    runtime: self.clone(),
                    routine_id: job.run.routine_id.clone(),
                    run_id: job.run.record.id.clone(),
                    name: job.run.record.routine_name.clone(),
                };

                self.agent_runner().run(job, progress, stop).await
            }
            RoutineAction::Command { command, timeout_secs } => {
                until_stopped(stop, command_run::run(&self.env, &job, &command, timeout_secs)).await
            }
            RoutineAction::Script { name, scope } => {
                script_run::run(&self.scripts, &self.env, &job, &name, scope, stop).await
            }
        }
    }

    /// Records how `run` ended, tells the view and the user, and frees its place.
    async fn finish(self: &Arc<Self>, run: &RoutineRun, outcome: RunOutcome) {
        let (routine_id, run_id) = (run.routine_id.clone(), run.record.id.clone());
        let status = outcome.status;
        let end = outcome.end(Timestamp::now());

        let finished = self.record(move |runs| runs.finish(&routine_id, &run_id, end)).await;

        // A view that reads the runs going on when it hears the end no longer finds this one.
        self.lock_runs().end_live(&run.routine_id, &run.record.id);

        match finished {
            Ok(Some(ended)) => {
                tracing::info!(
                    routine = run.routine_id,
                    run = run.record.id,
                    ?status,
                    "routine run ended"
                );
                self.emit(RoutinesEvent::RunFinished { run: ended.clone() });

                if let Some(alert) = Alert::of_run(&ended) {
                    self.alerts.send(&alert);
                }
            }
            Ok(None) => tracing::info!(run = run.record.id, "routine run ended after its records were deleted"),
            Err(error) => tracing::warn!(run = run.record.id, code = ?error.code(), "routine run end not recorded"),
        }

        self.release(&run.routine_id);
    }

    /// Frees the place of routine `id` and starts the runs waiting for one.
    fn release(self: &Arc<Self>, id: &str) {
        let ready = {
            let mut runs = self.lock_runs();
            runs.release(id);
            runs.next_ready()
        };

        self.released.notify_waiters();

        for (id, trigger) in ready {
            let runtime = self.clone();

            drop(tauri::async_runtime::spawn(async move {
                runtime.launch(&id, trigger).await;
            }));
        }
    }

    /// Stops the live run `run_id` ("Stop"). `notFound` when it is not running.
    pub fn stop_run(&self, run_id: &str) -> Result<()> {
        if !self.lock_runs().stop_run(run_id, StopReason::User) {
            return Err(BusinessError::NotFound("the run".to_owned()));
        }

        tracing::info!(run = run_id, "routine run stopped by the user");
        Ok(())
    }

    /// Stops the runs of `ids` for `reason` (their routines are being deleted, or may no longer
    /// run): waiting ones leave the queue, running ones are stopped and waited for (at most
    /// `budget`), so their records are complete before anything else happens to them.
    pub async fn stop_routines(self: &Arc<Self>, ids: &[String], reason: StopReason, budget: Duration) {
        let deadline = tokio::time::Instant::now() + budget;

        loop {
            let released = self.released.notified();
            let left = self.lock_runs().stop_routines(ids, reason);

            if !left || tokio::time::timeout_at(deadline, released).await.is_err() {
                return;
            }
        }
    }

    /// Stops every live or starting run for `reason` and empties the queue; nothing starts any
    /// more (the app closes).
    pub fn stop_all(&self, reason: StopReason) {
        self.lock_runs().stop_all(reason);
    }

    /// Waits until no run is left, at most `budget`. Blocks: call it off the async runtime.
    pub fn wait_idle(&self, budget: Duration) {
        let deadline = std::time::Instant::now() + budget;

        while !self.lock_runs().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Takes no run in for `ids` until [`RoutineRuntime::end_delete`] (their routines are being
    /// deleted).
    pub fn begin_delete(&self, ids: &[String]) {
        self.lock_runs().begin_delete(ids);
    }

    /// The delete of `ids` is over.
    pub fn end_delete(&self, ids: &[String]) {
        self.lock_runs().end_delete(ids);
    }

    /// The routines whose team run is going on or starting.
    pub(super) fn team_runs(&self) -> Vec<String> {
        self.lock_runs().team_runs()
    }
}

/// `work` until it ends, or a stopped outcome when `stop` fires first (`work` is dropped, which
/// kills the process it waits for).
async fn until_stopped(mut stop: StopSignal, work: impl Future<Output = RunOutcome>) -> RunOutcome {
    tokio::select! {
        outcome = work => outcome,
        reason = stop.stopped() => RunOutcome::stopped(reason),
    }
}

/// Whether the routine of `view` may start a run for `trigger`: valid and approved, and on its
/// schedule unless the user asked.
fn may_start(view: &RoutineView, trigger: RunTrigger) -> bool {
    match trigger {
        RunTrigger::Manual => view.can_run_now,
        RunTrigger::Schedule | RunTrigger::CatchUp | RunTrigger::AppStart => view.can_run_now && on_schedule(view),
    }
}

/// What a run of the routine of `view` starts with: the routine, its origin, and the project root
/// it runs in (a local project routine's `project`, the folder a team project routine was turned
/// on for; `None` for a global one).
fn run_target(view: RoutineView) -> Option<(Routine, HookOrigin, Option<String>)> {
    let routine = view.routine?;

    let project = match (routine.scope, view.origin) {
        (HookScope::Global, _) => None,
        (HookScope::Project, HookOrigin::Local) => routine.project.clone(),
        (HookScope::Project, HookOrigin::Team) => view.team_folder,
    };

    Some((routine, view.origin, project))
}

/// The folder `routine` runs in (see [`RunJob::folder`]), or why it cannot run. `project` is the
/// project root of the run ([`run_target`]), `root` the routines root. A global agent or command
/// routine runs in its own `<root>/workspace/<id>` (`team-<id>` for a team routine of `origin`),
/// created when missing and checked (a real folder, no symbolic link) before each run.
fn run_folder(
    routine: &Routine,
    origin: HookOrigin,
    project: Option<&str>,
    root: Option<&Path>,
) -> std::result::Result<Option<PathBuf>, &'static str> {
    if routine.scope == HookScope::Project {
        let Some(stored) = project else {
            return Err("The routine names no project folder");
        };

        let folder = PathBuf::from(stored);

        return match project_problem(&folder) {
            Some(problem) => Err(problem),
            None => Ok(Some(folder)),
        };
    }

    if let RoutineAction::Script { .. } = routine.action {
        return Ok(None);
    }

    let Some(root) = root else {
        return Err("Routines have no folder on this machine");
    };

    ensure_routine_workspace(root, &workspace_name(&routine.id, origin))
        .map(Some)
        .map_err(|_| "The routine's workspace folder could not be prepared")
}
