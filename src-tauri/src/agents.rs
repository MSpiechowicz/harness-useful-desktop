//! The app's agent sessions: the business [`AgentManager`], the process spawner it uses, the
//! stops started in the background (closed channels, page reloads) and the shutdown phase.
//!
//! Everything that stops sessions outside an IPC call is tracked here so that app exit can wait
//! for it: [`Agents::stop_all`] stops the remaining sessions and awaits the background stops.
//!
//! The production spawner records every agent and chat server it starts (see
//! [`harness_business::process::TrackedSpawner`]), and [`Agents::sweep_orphans_in_background`]
//! stops the ones an earlier run that crashed left behind.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use harness_business::agent::{AgentManager, TrustMemory};
use harness_business::process::{ChildRegistry, ProcessSpawner, TokioSpawner, TrackedSpawner, sweep_orphans};
use tauri::async_runtime::{self, JoinHandle};

/// See the module docs.
pub struct Agents {
    manager: AgentManager,
    spawner: Arc<dyn ProcessSpawner>,
    /// The projects the user trusted once in this run (Claude Code), never saved.
    trust: TrustMemory,
    /// Where the started children are recorded; `None` when they are not.
    children_dir: Option<PathBuf>,
    background: Mutex<Vec<JoinHandle<()>>>,
    phase: AtomicU8,
}

/// Values of [`Agents::phase`].
const RUNNING: u8 = 0;
const STOPPING: u8 = 1;
const STOPPED: u8 = 2;

impl Agents {
    /// Production sessions: real processes, default timing.
    pub fn new() -> Self {
        Self::with(AgentManager::new(), Arc::new(TokioSpawner))
    }

    /// Production sessions whose processes are recorded in `children_dir` (when given and the
    /// system supports it), so a later launch can stop what a crash left behind.
    pub fn tracked(children_dir: Option<PathBuf>) -> Self {
        let registry = children_dir.as_ref().and_then(ChildRegistry::open);

        let Some(registry) = registry else {
            return Self::new();
        };

        let spawner = Arc::new(TrackedSpawner::new(Arc::new(TokioSpawner), registry));

        Self {
            children_dir,
            ..Self::with(AgentManager::new(), spawner)
        }
    }

    /// Sessions of `manager`, started through `spawner` (tests).
    pub fn with(manager: AgentManager, spawner: Arc<dyn ProcessSpawner>) -> Self {
        Self {
            manager,
            spawner,
            trust: TrustMemory::new(),
            children_dir: None,
            background: Mutex::new(Vec::new()),
            phase: AtomicU8::new(RUNNING),
        }
    }

    /// The running sessions.
    pub fn manager(&self) -> &AgentManager {
        &self.manager
    }

    /// The projects the user trusted once in this run.
    pub fn trust(&self) -> &TrustMemory {
        &self.trust
    }

    /// Starts agent processes.
    pub fn spawner(&self) -> Arc<dyn ProcessSpawner> {
        self.spawner.clone()
    }

    /// Stops, off the main thread, the agents and chat servers an earlier run of the app recorded
    /// and left running when it ended abruptly (see [`sweep_orphans`]); logs what it stopped.
    pub fn sweep_orphans_in_background(&self) {
        let Some(dir) = self.children_dir.clone() else {
            return;
        };

        drop(async_runtime::spawn_blocking(move || {
            let report = sweep_orphans(&dir);

            if !report.killed.is_empty() || report.corrupt > 0 {
                tracing::info!(
                    killed = report.killed.len(),
                    pids = ?report.killed,
                    corrupt = report.corrupt,
                    "stopped child processes left by an earlier run"
                );
            }
        }));
    }

    /// True once app shutdown began; no new session may start.
    pub fn is_shutting_down(&self) -> bool {
        self.phase.load(Ordering::Acquire) != RUNNING
    }

    /// Enters the stopping phase; false when shutdown already began.
    pub fn begin_shutdown(&self) -> bool {
        self.phase
            .compare_exchange(RUNNING, STOPPING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Marks shutdown as complete: the next exit request is let through.
    pub fn finish_shutdown(&self) {
        self.phase.store(STOPPED, Ordering::Release);
    }

    /// True once shutdown completed.
    pub fn is_stopped(&self) -> bool {
        self.phase.load(Ordering::Acquire) == STOPPED
    }

    /// Stops session `key` without waiting (its event channel closed); unknown keys are ignored.
    /// The session leaves the manager at once, but its id stays held until the agent exited.
    pub fn stop_in_background(&self, key: String) {
        if let Some(stopping) = self.manager.begin_stop(&key) {
            self.track(async_runtime::spawn(stopping.finish()));
        }
    }

    /// Stops every session without waiting (the webview reloaded, so every channel is dead). The
    /// reloaded page may resume the same session ids at once: those starts wait for the old
    /// agents to exit.
    pub fn stop_all_in_background(&self) {
        self.stop_all_in_background_except(&HashSet::new());
    }

    /// Like [`Agents::stop_all_in_background`], sparing the sessions whose key is in `keep` (the
    /// chats of routine runs, which no page owns).
    pub fn stop_all_in_background_except(&self, keep: &HashSet<String>) {
        let stopping = self.manager.begin_stop_all_except(keep);

        self.track(async_runtime::spawn(stopping.finish()));
    }

    fn track(&self, task: JoinHandle<()>) {
        let mut background = self.background.lock().unwrap_or_else(PoisonError::into_inner);

        background.retain(|running| !running.inner().is_finished());
        background.push(task);
    }

    /// Stops every session and waits for the background stops too.
    pub async fn stop_all(&self) {
        self.manager.stop_all().await;

        let pending: Vec<JoinHandle<()>> = self
            .background
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect();

        for task in pending {
            task.await.ok();
        }
    }
}

impl Default for Agents {
    fn default() -> Self {
        Self::new()
    }
}
