//! The process lifecycle every chat engine shares: spawn, the emitter, the stdin writer, the
//! redacted stderr reader, the exit waiter, the startup deadline and shutdown.
//!
//! The stdout reader is the engine's own (RPC frames, a server's listening line); it is passed to
//! [`AgentProcess::spawn`] and drained before the exit is reported.
//!
//! Stop: close stdin → wait up to the grace period (3 s) → kill the process group. `exited` is
//! always the last event and is emitted exactly once, also when the agent crashes. A process
//! dropped without `stop` is killed.

use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::SessionOptions;
use super::io::{Shared, WriterMsg};
use super::sink::{EmitMsg, EventSink, spawn_emitter};
use crate::error::{BusinessError, Result};
use crate::model::AgentEvent;
use crate::pi_family::rpc::LineSplitter;
use crate::process::{BoxFuture, ChildHandle, ExecSpec, ExitInfo, ProcessSpawner};
use crate::security::redact_text;

/// Longest stderr line passed on (2 KiB).
pub const MAX_STDERR_LINE: usize = 2048;

/// Stderr lines passed on per second at most.
pub const MAX_STDERR_PER_SECOND: u32 = 20;

/// How long the waiter lets the readers drain after the process exited.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// How long a killed agent may take to be reaped.
const KILL_WAIT: Duration = Duration::from_secs(5);

/// How long `stop` waits for the last events to be delivered.
const EMITTER_WAIT: Duration = Duration::from_secs(2);

/// Frames queued for stdin at most.
const WRITER_QUEUE: usize = 64;

/// Resolves when a start must be given up (every session was stopped meanwhile).
pub type StartAbort<'a> = BoxFuture<'a, ()>;

/// The agent's stdout, handed to the engine's reader.
pub(crate) type Stdout = Box<dyn AsyncRead + Send + Unpin>;

/// A running agent process and its tasks (see the module docs).
pub(crate) struct AgentProcess {
    shared: Arc<Shared>,
    exited: watch::Receiver<Option<ExitInfo>>,
    kill: watch::Sender<bool>,
    emitter: Mutex<Option<JoinHandle<()>>>,
    stop_grace: Duration,
}

impl AgentProcess {
    /// Spawns `spec` as session `key`, whose events go to `sink`, and starts its tasks;
    /// `read_stdout` becomes the stdout reader.
    pub fn spawn<R, F>(
        spawner: &dyn ProcessSpawner,
        spec: &ExecSpec,
        key: String,
        sink: Arc<dyn EventSink>,
        options: &SessionOptions,
        read_stdout: R,
    ) -> Result<Self>
    where
        R: FnOnce(Stdout, Arc<Shared>) -> F,
        F: Future<Output = ()> + Send + 'static,
    {
        let process = spawner.spawn(spec)?;
        let (events, emitter) = spawn_emitter(key, sink, options.coalesce_window);
        let (writer, frames) = mpsc::channel(WRITER_QUEUE);
        let shared = Arc::new(Shared::new(writer, events));

        let (kill_tx, kill_rx) = watch::channel(false);
        let (exit_tx, exit_rx) = watch::channel(None);

        tokio::spawn(write_loop(process.stdin, frames));
        let readers = Readers {
            stdout: tokio::spawn(read_stdout(process.stdout, shared.clone())),
            stderr: tokio::spawn(stderr_loop(process.stderr, shared.clone())),
        };
        tokio::spawn(wait_loop(process.child, kill_rx, exit_tx, readers, shared.clone()));

        Ok(Self {
            shared,
            exited: exit_rx,
            kill: kill_tx,
            emitter: Mutex::new(Some(emitter)),
            stop_grace: options.stop_grace,
        })
    }

    /// State shared with the process's tasks.
    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    /// How the process ended, once it has.
    pub fn exit_info(&self) -> Option<ExitInfo> {
        *self.exited.borrow()
    }

    /// True until the process exited.
    pub fn is_running(&self) -> bool {
        self.exit_info().is_none()
    }

    /// Kills the process after a failed or abandoned startup and returns `error` with the agent's
    /// last stderr lines.
    pub async fn fail_start(&self, error: BusinessError) -> BusinessError {
        self.kill_now().await;

        with_stderr(error, &self.shared.stderr_tail())
    }

    /// Stops the agent (see the module docs) and waits until `exited` was delivered.
    pub async fn stop(&self) {
        self.shared.writer.try_send(WriterMsg::Close).ok();

        if !self.wait_exit(self.stop_grace).await {
            self.kill_now().await;
        }

        let emitter = self.emitter.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(emitter) = emitter {
            tokio::time::timeout(EMITTER_WAIT, emitter).await.ok();
        }
    }

    /// Kills the process group and waits (briefly) until the process was reaped.
    pub async fn kill_now(&self) {
        self.kill.send_replace(true);
        self.wait_exit(KILL_WAIT).await;
    }

    /// Waits at most `limit` for the process to exit; false when it is still running.
    pub async fn wait_exit(&self, limit: Duration) -> bool {
        let mut exited = self.exited.clone();
        let done = exited.wait_for(Option::is_some);

        matches!(tokio::time::timeout(limit, done).await, Ok(Ok(_)))
    }
}

impl Drop for AgentProcess {
    fn drop(&mut self) {
        // Safety net: a session dropped without `stop` must not leave its agent running.
        self.kill.send_replace(true);
    }
}

/// Runs the startup `steps` within the deadline of `options`, unless `abort` resolves first.
pub(crate) async fn run_startup(
    options: &SessionOptions,
    abort: StartAbort<'_>,
    steps: impl Future<Output = Result<()>>,
) -> Result<()> {
    let deadline = options.startup_deadline;
    let steps = tokio::time::timeout(deadline, steps);

    tokio::select! {
        result = steps => result.unwrap_or_else(|_| {
            Err(BusinessError::Timeout(format!(
                "the agent did not finish starting within {} s",
                deadline.as_secs_f64()
            )))
        }),
        () = abort => Err(BusinessError::Process("the session was stopped while it started".to_owned())),
    }
}

/// Adds the agent's last stderr lines to a startup error.
fn with_stderr(error: BusinessError, tail: &str) -> BusinessError {
    if tail.is_empty() {
        return error;
    }

    match error {
        BusinessError::Timeout(message) => BusinessError::Timeout(format!("{message} (stderr: {tail})")),
        BusinessError::Process(message) => BusinessError::Process(format!("{message} (stderr: {tail})")),
        BusinessError::Protocol(message) => BusinessError::Protocol(format!("{message} (stderr: {tail})")),
        other => other,
    }
}

/// Writes frames to stdin until told to close (or the pipe breaks), then closes stdin.
async fn write_loop(mut stdin: Box<dyn AsyncWrite + Send + Unpin>, mut frames: mpsc::Receiver<WriterMsg>) {
    while let Some(message) = frames.recv().await {
        let WriterMsg::Frame(bytes) = message else {
            break;
        };

        if stdin.write_all(&bytes).await.is_err() || stdin.flush().await.is_err() {
            break;
        }
    }

    stdin.shutdown().await.ok();
}

/// Passes stderr on as redacted lines of at most [`MAX_STDERR_LINE`] bytes, at most
/// [`MAX_STDERR_PER_SECOND`] per second; a dropped burst is summarized in one line.
async fn stderr_loop(stderr: Box<dyn AsyncRead + Send + Unpin>, shared: Arc<Shared>) {
    let mut lines = LineSplitter::new(stderr, MAX_STDERR_LINE);
    let mut window: Option<Instant> = None;
    let mut sent = 0;
    let mut suppressed = 0_u64;

    while let Some(line) = lines.next_line().await {
        let raw = String::from_utf8_lossy(&line.bytes);
        let text = truncate(redact_text(raw.trim_end()), MAX_STDERR_LINE);

        if text.trim().is_empty() {
            continue;
        }

        shared.remember_stderr(&text);

        let now = Instant::now();
        if window.is_none_or(|start| now.duration_since(start) >= Duration::from_secs(1)) {
            window = Some(now);
            sent = 0;

            if suppressed > 0 {
                shared.emit(suppressed_line(suppressed));
                suppressed = 0;
                sent = 1;
            }
        }

        if sent < MAX_STDERR_PER_SECOND {
            shared.emit(AgentEvent::Stderr { line: text });
            sent += 1;
        } else {
            suppressed += 1;
        }
    }

    if suppressed > 0 {
        shared.emit(suppressed_line(suppressed));
    }
}

fn suppressed_line(count: u64) -> AgentEvent {
    AgentEvent::Stderr {
        line: format!("[{count} more stderr lines were not shown]"),
    }
}

/// `text` cut to at most `max` bytes at a character boundary.
fn truncate(mut text: String, max: usize) -> String {
    if text.len() > max {
        let mut end = max;
        while !text.is_char_boundary(end) {
            end -= 1;
        }

        text.truncate(end);
    }

    text
}

/// The tasks the waiter lets finish before reporting the exit.
struct Readers {
    stdout: JoinHandle<()>,
    stderr: JoinHandle<()>,
}

/// Waits for the process (killing it when `kill` is set), lets the readers drain, fails what is
/// still waiting, publishes the exit and emits `exited` as the session's last event.
async fn wait_loop(
    mut child: Box<dyn ChildHandle>,
    mut kill: watch::Receiver<bool>,
    exit: watch::Sender<Option<ExitInfo>>,
    readers: Readers,
    shared: Arc<Shared>,
) {
    let waited = tokio::select! {
        result = child.wait() => Some(result),
        _ = kill.wait_for(|kill| *kill) => None,
    };

    let result = match waited {
        Some(result) => result,
        None => {
            child.start_kill().ok();
            child.wait().await
        }
    };
    let info = result.unwrap_or_default();

    drain(readers.stdout).await;
    drain(readers.stderr).await;
    shared.pending.close("the agent exited");
    exit.send_replace(Some(info));

    shared.emit(AgentEvent::Exited {
        code: info.code,
        signal: info.signal,
    });
    shared.events.send(EmitMsg::Close).ok();
}

/// Waits for `task` briefly; a descendant holding the pipe open must not delay the exit forever.
async fn drain(task: JoinHandle<()>) {
    let abort = task.abort_handle();

    if tokio::time::timeout(DRAIN_GRACE, task).await.is_err() {
        abort.abort();
    }
}
