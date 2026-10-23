//! Keeps `harness-memory embedder run` running while vector search is on.
//!
//! The embedder is one process per computer. The app spawns it through the tracked spawner (so a
//! crash of the app does not leave it behind, see [`crate::process::TrackedSpawner`]) with a piped
//! stdin it keeps open: the embedder exits when stdin closes, so stopping it is closing stdin,
//! waiting briefly, then killing what is left.
//!
//! - Its stdout lines are folded into a [`MemoryEmbedderState`] ([`EmbedderTracker`]) that
//!   [`EmbedderSupervisor::subscribe`] watches.
//! - `busy` means another app instance runs the embedder: the state is `elsewhere`, the process
//!   exited, and it is not started again (until [`EmbedderSupervisor::start`] is called again).
//! - A process that crashes (or cannot be started) is started again after 1, 5 and 30 seconds, at
//!   most three times within ten minutes, then the state is `failed` for good. A process that
//!   exits 0 by itself is not started again: the state is `off`.
//!
//! Only the phases are logged, never what the embedder printed.

use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, BufReader};
use tokio::sync::{Notify, watch};
use tokio::task::JoinHandle;

use super::catalog::MEMORY_BINARY;
use super::memory_embedder::{EmbedderTracker, embedder_status, feature_server, parse_event, set_embedder_enabled};
use super::memory_features::{FeaturesSupport, MemoryFeatures};
use crate::env::Env;
use crate::model::MemoryEmbedderState;
use crate::process::{ChildHandle, ExecSpec, ProcessRunner, ProcessSpawner, SpawnedProcess, launcher_search_dirs};

/// The longest stdout line read; the rest of a longer line is dropped.
const MAX_LINE_BYTES: u64 = 64 * 1024;

/// How the supervisor restarts and stops the embedder.
#[derive(Debug, Clone)]
pub struct SupervisorOptions {
    /// How long to wait before each restart in a row; its length is the most restarts within
    /// [`Self::window`] (1, 5 and 30 seconds).
    pub backoff: Vec<Duration>,
    /// How far back crashes count towards giving up (ten minutes).
    pub window: Duration,
    /// How long a stopped embedder gets to exit after its stdin closed before it is killed.
    pub stop_grace: Duration,
}

impl Default for SupervisorOptions {
    fn default() -> Self {
        Self {
            backoff: vec![Duration::from_secs(1), Duration::from_secs(5), Duration::from_secs(30)],
            window: Duration::from_secs(10 * 60),
            stop_grace: Duration::from_secs(2),
        }
    }
}

/// `harness-memory embedder run` for `server`, in the home directory.
pub fn embedder_spec(env: &Env, server: &Path) -> ExecSpec {
    ExecSpec::new(server)
        .args(["embedder", "run"])
        .cwd(&env.home)
        .env("NO_COLOR", "1")
        .search_dirs(launcher_search_dirs(env, server))
}

/// The state and the tracker behind it, shared with the task.
struct Shared {
    tracker: Mutex<EmbedderTracker>,
    state: watch::Sender<MemoryEmbedderState>,
}

impl Shared {
    /// Runs `change` on the tracker and publishes the state when it changed.
    fn update(&self, change: impl FnOnce(&mut EmbedderTracker) -> bool) {
        let mut tracker = self.tracker.lock().unwrap_or_else(PoisonError::into_inner);

        if change(&mut tracker) {
            self.state.send_replace(tracker.state().clone());
        }
    }

    fn is_busy(&self) -> bool {
        self.tracker.lock().unwrap_or_else(PoisonError::into_inner).is_busy()
    }
}

/// The running task and the way to stop it.
struct Running {
    stop: Arc<Notify>,
    task: JoinHandle<()>,
}

/// See the module docs.
pub struct EmbedderSupervisor {
    spawner: Arc<dyn ProcessSpawner>,
    options: SupervisorOptions,
    shared: Arc<Shared>,
    running: tokio::sync::Mutex<Option<Running>>,
    /// Held while [`Self::reconcile`] decides, so two callers never start and stop it crosswise.
    reconciling: tokio::sync::Mutex<()>,
}

impl EmbedderSupervisor {
    /// A supervisor in the `off` state that starts the embedder through `spawner`.
    pub fn new(spawner: Arc<dyn ProcessSpawner>) -> Self {
        Self::with_options(spawner, SupervisorOptions::default())
    }

    /// [`Self::new`] with other timing.
    pub fn with_options(spawner: Arc<dyn ProcessSpawner>, options: SupervisorOptions) -> Self {
        let (state, _) = watch::channel(MemoryEmbedderState::default());

        Self {
            spawner,
            options,
            shared: Arc::new(Shared {
                tracker: Mutex::new(EmbedderTracker::new()),
                state,
            }),
            running: tokio::sync::Mutex::new(None),
            reconciling: tokio::sync::Mutex::new(()),
        }
    }

    /// The state now.
    pub fn state(&self) -> MemoryEmbedderState {
        self.shared.state.borrow().clone()
    }

    /// A receiver of the state: it holds the current one and wakes on each change.
    pub fn subscribe(&self) -> watch::Receiver<MemoryEmbedderState> {
        self.shared.state.subscribe()
    }

    /// Starts `spec` (see [`embedder_spec`]) and keeps it running. False when a supervising task is
    /// still going (the embedder runs, or waits to be restarted); a task that gave up (`failed`,
    /// `elsewhere`) or ended (`off`) is replaced.
    pub async fn start(&self, spec: ExecSpec) -> bool {
        let mut running = self.running.lock().await;

        if running.as_ref().is_some_and(|running| !running.task.is_finished()) {
            return false;
        }

        // `starting` is the state the moment this returns, not once the task first runs.
        self.shared.update(EmbedderTracker::starting);

        let stop = Arc::new(Notify::new());
        let task = tokio::spawn(supervise(
            self.spawner.clone(),
            self.options.clone(),
            self.shared.clone(),
            spec,
            stop.clone(),
        ));

        *running = Some(Running { stop, task });
        true
    }

    /// Makes the embedder run when `wanted` (project memory and vector search are on) and the
    /// installed Harness Memory lists `embedder`, and stop otherwise. A server that does not know
    /// the embedder, or is not installed, starts nothing. The embedder's own switch is turned on
    /// first when `embedder status` says it is off (an app that updated while the pref was already
    /// on). Failures are logged: the state tells the rest.
    pub async fn reconcile(&self, env: &Env, runner: &dyn ProcessRunner, features: &MemoryFeatures, wanted: bool) {
        let _deciding = self.reconciling.lock().await;

        if !wanted {
            self.stop().await;
            return;
        }

        let Ok(server) = feature_server(env, runner, features, FeaturesSupport::embedder, "vector search").await else {
            self.stop().await;
            return;
        };

        match embedder_status(env, runner, &server).await {
            Ok(status) if status.enabled => {}
            Ok(_) => {
                if let Err(error) = set_embedder_enabled(env, runner, &server, true).await {
                    tracing::warn!(%error, "the embedder could not be switched on");
                    return;
                }
            }
            Err(error) => {
                tracing::warn!(%error, "the embedder's status could not be read");
                return;
            }
        }

        self.start(embedder_spec(env, &server)).await;
    }

    /// Stops the embedder: closes its stdin, waits [`SupervisorOptions::stop_grace`], then kills
    /// it. The state is `off` afterwards, whatever it was. Does nothing when it is not running.
    pub async fn stop(&self) {
        let running = self.running.lock().await.take();

        if let Some(running) = running {
            running.stop.notify_one();

            if let Err(error) = running.task.await {
                tracing::warn!(%error, "the embedder supervisor ended abnormally");
            }
        }

        self.shared.update(EmbedderTracker::stopped);
    }
}

/// How one process ended.
enum Outcome {
    /// The supervisor stopped it.
    Stopped,
    /// Another app instance runs the embedder.
    Busy,
    /// It exited 0 by itself.
    Exited,
    /// It crashed, or could not be started.
    Crashed(&'static str),
}

async fn supervise(
    spawner: Arc<dyn ProcessSpawner>,
    options: SupervisorOptions,
    shared: Arc<Shared>,
    spec: ExecSpec,
    stop: Arc<Notify>,
) {
    let mut restarts: VecDeque<Instant> = VecDeque::new();

    loop {
        shared.update(EmbedderTracker::starting);

        match run_once(spawner.as_ref(), &options, &shared, &spec, &stop).await {
            Outcome::Stopped | Outcome::Exited => {
                shared.update(EmbedderTracker::stopped);
                return;
            }
            Outcome::Busy => {
                tracing::info!("another instance runs the embedder");
                return;
            }
            Outcome::Crashed(reason) => {
                let now = Instant::now();
                restarts.retain(|restarted| now.duration_since(*restarted) < options.window);

                let Some(backoff) = options.backoff.get(restarts.len()).copied() else {
                    tracing::warn!("the embedder keeps stopping; giving up");
                    shared.update(|tracker| tracker.failed(reason));
                    return;
                };

                tracing::warn!(
                    backoff_ms = backoff.as_millis(),
                    reason,
                    "the embedder stopped; starting it again"
                );
                restarts.push_back(now);

                tokio::select! {
                    () = tokio::time::sleep(backoff) => {}
                    () = stop.notified() => {
                        shared.update(EmbedderTracker::stopped);
                        return;
                    }
                }
            }
        }
    }
}

/// One process, from its spawn to its exit.
async fn run_once(
    spawner: &dyn ProcessSpawner,
    options: &SupervisorOptions,
    shared: &Shared,
    spec: &ExecSpec,
    stop: &Notify,
) -> Outcome {
    let process = match spawner.spawn(spec) {
        Ok(process) => process,
        Err(error) => {
            tracing::warn!(%error, "{MEMORY_BINARY} embedder could not be started");
            return Outcome::Crashed("Harness Memory could not start the embedder");
        }
    };

    let SpawnedProcess {
        stdin,
        stdout,
        stderr,
        mut child,
    } = process;

    let drain = tokio::spawn(discard(stderr));
    let mut lines = BufReader::new(stdout);
    let mut buffer = Vec::new();

    let stopped = loop {
        tokio::select! {
            biased;
            () = stop.notified() => break true,
            line = next_line(&mut lines, &mut buffer) => match line {
                Ok(Some(line)) => {
                    if let Some(event) = parse_event(&line) {
                        shared.update(|tracker| tracker.apply(event));
                    }
                }
                Ok(None) | Err(_) => break false,
            },
        }
    };

    let outcome = if stopped {
        close_and_wait(stdin, child.as_mut(), options.stop_grace).await;
        Outcome::Stopped
    } else {
        finish(stdin, child.as_mut(), options.stop_grace, shared.is_busy()).await
    };

    drain.abort();
    outcome
}

/// The process closed its stdout: waits for its exit (a process that stays alive without stdout is
/// stopped like any other).
async fn finish(
    stdin: Box<dyn AsyncWrite + Send + Unpin>,
    child: &mut dyn ChildHandle,
    grace: Duration,
    busy: bool,
) -> Outcome {
    let exit = match tokio::time::timeout(grace, child.wait()).await {
        Ok(exit) => exit.ok(),
        Err(_) => {
            kill(child).await;
            None
        }
    };

    drop(stdin);

    if busy {
        return Outcome::Busy;
    }

    match exit.and_then(|exit| exit.code) {
        Some(0) => Outcome::Exited,
        _ => Outcome::Crashed("Harness Memory's embedder stopped unexpectedly"),
    }
}

/// Closes the lifeline the embedder exits on, waits `grace`, then kills what is left.
async fn close_and_wait(stdin: Box<dyn AsyncWrite + Send + Unpin>, child: &mut dyn ChildHandle, grace: Duration) {
    drop(stdin);

    if tokio::time::timeout(grace, child.wait()).await.is_err() {
        kill(child).await;
    }
}

/// Kills the process group and reaps the child.
async fn kill(child: &mut dyn ChildHandle) {
    child.start_kill().ok();
    child.kill_process_group();

    tokio::time::timeout(Duration::from_secs(2), child.wait()).await.ok();
}

/// Reads and drops `stream` so a full pipe never blocks the embedder. Its text is not kept: it can
/// quote the data the embedder works on.
async fn discard(mut stream: Box<dyn AsyncRead + Send + Unpin>) {
    let mut buffer = [0_u8; 4096];

    while matches!(stream.read(&mut buffer).await, Ok(read) if read > 0) {}
}

/// The next line of `reader` (at most [`MAX_LINE_BYTES`], the rest of a longer one is dropped);
/// `None` at the end of the stream.
async fn next_line<R: AsyncBufRead + Unpin>(reader: &mut R, buffer: &mut Vec<u8>) -> io::Result<Option<String>> {
    loop {
        buffer.clear();
        let read = (&mut *reader).take(MAX_LINE_BYTES).read_until(b'\n', buffer).await?;

        if read == 0 {
            return Ok(None);
        }

        if buffer.ends_with(b"\n") || (read as u64) < MAX_LINE_BYTES {
            return Ok(Some(String::from_utf8_lossy(buffer).into_owned()));
        }

        skip_rest_of_line(reader, buffer).await?;
    }
}

/// Reads up to the end of the line a read stopped in the middle of.
async fn skip_rest_of_line<R: AsyncBufRead + Unpin>(reader: &mut R, buffer: &mut Vec<u8>) -> io::Result<()> {
    loop {
        buffer.clear();
        let read = (&mut *reader).take(MAX_LINE_BYTES).read_until(b'\n', buffer).await?;

        if read == 0 || buffer.ends_with(b"\n") {
            return Ok(());
        }
    }
}
