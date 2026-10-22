//! Routines in the app window (see `harness_business::routines` and `docs/routines-format.md`).
//!
//! - [`RoutineRuntime`] (in [`AppState`]) owns the routines folder, the run records and the
//!   schedule file: it serializes the app's writes, and tells the Routines view about changes and
//!   runs (`routines_listen`).
//! - The folder and the approvals are looked at every second ([`crate::doc_watch`]); a change is
//!   sent as `changed` and wakes the [`scheduler`].
//! - [`scheduler`]: a task that plans the due routines (catch-up once at launch, app start
//!   routines), then sleeps until the next slot (30 s at most) or a change.
//! - [`runner`]: one run per routine, at most 2 agent and 4 command or script runs at once, the
//!   rest waiting in the order they were due. Commands run in [`command_run`], scripts over their
//!   own Harness Memory connection in [`script_run`], agents through the [`job::AgentRunner`] set
//!   with [`RoutineRuntime::set_agent_runner`] ([`agent_run`] at launch: the chat, its [`relay`]
//!   for a window that opens it, its questions in [`attention`]).
//! - [`notify`]: OS notifications of runs that need the user, failed or finished.
//! - [`team`]: the team routines shared through Harness Memory, refreshed every minute and
//!   scheduled like local ones once turned on here and trusted or approved.
//! - At launch, before anything else, the records a closed app left `running` become `stopped`
//!   (never one of a run this session started); at exit nothing starts
//!   any more, the live and starting runs are stopped (their processes killed) and marked the same
//!   way ([`RoutineRuntime::shutdown`]).
//!
//! Logs carry routine and run ids, never a routine's prompt, command or output.

mod agent_chat;
pub mod agent_run;
pub mod attention;
pub mod command_run;
pub mod job;
pub mod notify;
pub mod progress;
pub mod queue;
pub mod relay;
pub mod runner;
pub mod scheduler;
pub mod script_run;
pub mod team;
#[cfg(test)]
mod team_copy_tests;
#[cfg(test)]
mod team_stop_tests;
#[cfg(test)]
mod team_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_agent;
#[cfg(test)]
mod tests_agent_claude;
#[cfg(test)]
mod tests_agent_end;
#[cfg(test)]
mod tests_agent_opencode;
#[cfg(test)]
mod tests_agent_opencode2;
#[cfg(test)]
mod tests_agent_wait;
#[cfg(test)]
mod tests_launch;
#[cfg(test)]
mod tests_runs;
#[cfg(test)]
mod tests_stops;
#[cfg(test)]
mod tests_workspace;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Duration;

use harness_business::ecosystem::MemoryScripts;
use harness_business::env::Env;
use harness_business::model::{RoutinesEvent, TeamRoutines};
use harness_business::routines::{RoutineKind, RoutineStore, RunStore};
use harness_business::{BusinessError, Result};
use jiff::Timestamp;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Notify;

use self::job::{AgentRunner, AgentsUnavailable};
use self::notify::Alerts;
use self::queue::Runs;
use self::scheduler::SchedulePlan;
use crate::doc_watch::{self, DocWatch};
use crate::state::AppState;
use crate::team_sync::TeamSync;

/// See the module docs.
pub struct RoutineRuntime {
    store: Option<RoutineStore>,
    /// `<app_local_data_dir>`: the run records and the schedule file.
    local_data: Option<PathBuf>,
    env: Arc<Env>,
    scripts: Arc<MemoryScripts>,
    /// Held by every write of the app (routine files, approvals, run records, the schedule), so
    /// two writers never load and save the same file at once.
    writes: Arc<Mutex<()>>,
    listener: Mutex<Option<Channel<RoutinesEvent>>>,
    watch: DocWatch,
    /// Wakes the scheduler (a change, a write).
    wake: Notify,
    /// Told whenever a run ends or leaves the queue.
    released: Notify,
    runs: Mutex<Runs>,
    plan: Mutex<SchedulePlan>,
    alerts: Alerts,
    agents: RwLock<Arc<dyn AgentRunner>>,
    /// The chats of the live agent runs ([`relay`]).
    relays: relay::Relays,
    /// The team routines shared through Harness Memory ([`team`]).
    team: TeamSync<RoutineKind, TeamRoutines>,
    /// The team routines changed: the next look checks the team runs going on
    /// ([`RoutineRuntime::reconcile_team`]).
    team_changed: AtomicBool,
}

/// How long the app's exit waits for the stopped runs to end (their processes are killed at once).
const EXIT_WAIT: Duration = Duration::from_secs(1);

impl RoutineRuntime {
    /// Routines in `store` (`None`: no routines folder, routines are unavailable), their runs and
    /// schedule in `local_data`, scripts run through `scripts`' server.
    pub fn new(
        store: Option<RoutineStore>,
        local_data: Option<PathBuf>,
        env: Arc<Env>,
        scripts: Arc<MemoryScripts>,
    ) -> Self {
        Self {
            store,
            local_data,
            env,
            scripts,
            writes: Arc::default(),
            listener: Mutex::default(),
            watch: DocWatch::default(),
            wake: Notify::new(),
            released: Notify::new(),
            runs: Mutex::default(),
            plan: Mutex::default(),
            alerts: Alerts::default(),
            agents: RwLock::new(Arc::new(AgentsUnavailable)),
            relays: relay::Relays::default(),
            team: TeamSync::default(),
            team_changed: AtomicBool::new(false),
        }
    }

    /// The routines folder; `unsupported` when there is none (Windows without `LOCALAPPDATA`).
    pub fn store(&self) -> Result<RoutineStore> {
        self.store
            .clone()
            .ok_or_else(|| BusinessError::Unsupported("routines need a local data folder".to_owned()))
    }

    /// The run records; `unsupported` without a local data folder.
    pub fn run_store(&self) -> Result<RunStore> {
        self.local_data
            .as_deref()
            .map(RunStore::new)
            .ok_or_else(|| BusinessError::Unsupported("routines need a local data folder".to_owned()))
    }

    /// Runs agent routines through `agents` from now on (see [`job::AgentRunner`]).
    pub fn set_agent_runner(&self, agents: Arc<dyn AgentRunner>) {
        *self.agents.write().unwrap_or_else(PoisonError::into_inner) = agents;
    }

    fn agent_runner(&self) -> Arc<dyn AgentRunner> {
        self.agents.read().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Runs `task` on the store off the async runtime, without the write lock.
    pub async fn read<T, F>(&self, task: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&RoutineStore) -> Result<T> + Send + 'static,
    {
        let store = self.store()?;

        off_runtime(move || task(&store)).await
    }

    /// Runs the write `task` on the store off the async runtime, one write at a time, then wakes
    /// the scheduler (the routine may run differently now).
    pub async fn write<T, F>(&self, task: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&RoutineStore) -> Result<T> + Send + 'static,
    {
        let store = self.store()?;
        let written = self.locked(move || task(&store)).await;

        self.wake.notify_one();
        written
    }

    /// Runs `task` (a write to the run records) off the async runtime under the write lock.
    pub async fn record<T, F>(&self, task: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&RunStore) -> Result<T> + Send + 'static,
    {
        let runs = self.run_store()?;

        self.locked(move || task(&runs)).await
    }

    /// Reads the run records off the async runtime, without the write lock.
    pub async fn read_runs<T, F>(&self, task: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&RunStore) -> T + Send + 'static,
    {
        let runs = self.run_store()?;

        off_runtime(move || Ok(task(&runs))).await
    }

    async fn locked<T, F>(&self, task: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T> + Send + 'static,
    {
        let writes = self.writes.clone();

        off_runtime(move || {
            let _writing = writes.lock().unwrap_or_else(PoisonError::into_inner);
            task()
        })
        .await
    }

    /// Sends the changes and runs to `channel`, replacing the previous listener.
    pub fn listen(&self, channel: Channel<RoutinesEvent>) {
        *self.listener.lock().unwrap_or_else(PoisonError::into_inner) = Some(channel);
    }

    /// Sends `event` to the listener; a listener whose page is gone is dropped.
    pub fn emit(&self, event: RoutinesEvent) {
        let mut listener = self.listener.lock().unwrap_or_else(PoisonError::into_inner);

        if let Some(channel) = listener.as_ref()
            && channel.send(event).is_err()
        {
            *listener = None;
        }
    }

    /// Makes the scheduler look again now.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// One look at the folder and the approvals (see [`DocWatch::poll`]): a change deletes the
    /// workspace folders of routines that are gone, is sent as `changed` and wakes the scheduler.
    /// True when something changed. Blocks: call it off the async runtime.
    pub fn poll(&self) -> bool {
        let Some(store) = &self.store else {
            return false;
        };

        if !self.watch.poll(store, &self.writes) {
            return false;
        }

        // An agent may have removed a routine: its folder in the workspace goes too.
        self.forget_workspaces();
        self.emit(RoutinesEvent::Changed);
        self.wake();
        true
    }

    /// Marks the records still `running` as stopped: at launch the runs the last session left (a
    /// run this session started already goes on). Blocks: call it off the async runtime.
    pub fn stop_leftovers(&self) {
        let live = self.lock_runs().live_runs();

        self.stop_records(&live);
    }

    /// Marks the records still `running` as stopped but those of `live`.
    fn stop_records(&self, live: &BTreeSet<String>) {
        let Ok(runs) = self.run_store() else {
            return;
        };

        let _writing = self.writes.lock().unwrap_or_else(PoisonError::into_inner);

        match runs.stop_leftovers(Timestamp::now(), live) {
            Ok(0) => {}
            Ok(stopped) => tracing::info!(stopped, "routine runs left running were stopped"),
            Err(error) => tracing::warn!(code = ?error.code(), "routine runs left running were not stopped"),
        }
    }

    /// App exit: no run starts any more, every live or starting run is stopped (briefly waited
    /// for, so its processes are gone) and its record marked stopped. Blocks: call it off the async
    /// runtime (the shutdown thread).
    pub fn shutdown(&self) {
        self.stop_all(job::StopReason::Closed);
        self.wait_idle(EXIT_WAIT);
        self.stop_records(&BTreeSet::new());
    }
}

/// Runs `task` on a blocking thread.
pub(crate) async fn off_runtime<T, F>(task: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| BusinessError::Internal(format!("routines task failed: {error}")))?
}

/// Starts the routines of this launch: first the records a closed app left running are stopped
/// (here, before any run can start) and the workspace folders of routines that are gone deleted,
/// then OS notifications through `app`, the folder watcher, and the scheduler, which the window
/// does not wait for. Does nothing without a routines folder.
pub fn spawn_on_launch<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let runtime = state.routines.clone();

    if runtime.store.is_none() {
        return;
    }

    runtime.stop_leftovers();
    runtime.forget_workspaces();

    let handle = app.clone();
    runtime
        .alerts
        .attach(Arc::new(move |alert| notify::native(&handle, alert)));

    let watched = runtime.clone();
    doc_watch::spawn::<RoutineKind>(move || {
        watched.poll();
    });

    team::spawn(runtime, scheduler::spawn);
}
