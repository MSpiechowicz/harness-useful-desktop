//! The process lifecycle every chat engine shares: spawn, the emitter, the stdin writer, the
//! redacted stderr reader, the exit waiter, the startup deadline and shutdown.
//!
//! The stdout reader is the engine's own (RPC frames, a server's listening line); it is passed to
//! [`AgentProcess::spawn`] and drained before the exit is reported.
//!
//! Stop: close stdin → (engines that ask for it, [`ProcessTraits::terminate_on_stop`]: SIGTERM to
//! the process group) → wait up to the grace period (3 s) → kill the process group. `exited` is
//! always the last event and is emitted exactly once, also when the agent crashes. A process
//! dropped without `stop` is killed.
//!
//! Stderr (and whatever stdout an engine passes on) reaches the session as redacted, rate-limited
//! lines ([`LineForwarder`]); a secret the agent has in its environment
//! ([`ProcessTraits::secret`]) is masked in them and in every event.

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
use crate::pi_family::rpc::{Line, LineSplitter};
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

/// How an engine's process differs from the RPC default.
#[derive(Default)]
pub(crate) struct ProcessTraits {
    /// `stop` asks the process to shut down (SIGTERM, see `ChildHandle::terminate`) right after
    /// closing stdin, for servers that ignore stdin EOF.
    pub terminate_on_stop: bool,
    /// A secret the agent gets in its environment; masked in every line and event.
    pub secret: Option<String>,
}

/// What the exit waiter is told to do with the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Control {
    Run,
    Terminate,
    Kill,
}

/// A running agent process and its tasks (see the module docs).
pub(crate) struct AgentProcess {
    shared: Arc<Shared>,
    exited: watch::Receiver<Option<ExitInfo>>,
    control: watch::Sender<Control>,
    emitter: Mutex<Option<JoinHandle<()>>>,
    stop_grace: Duration,
    terminate_on_stop: bool,
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
        traits: ProcessTraits,
        read_stdout: R,
    ) -> Result<Self>
    where
        R: FnOnce(Stdout, Arc<Shared>) -> F,
        F: Future<Output = ()> + Send + 'static,
    {
        let process = spawner.spawn(spec)?;
        let (events, emitter) = spawn_emitter(key, sink, options.coalesce_window);
        let (writer, frames) = mpsc::channel(WRITER_QUEUE);
        let shared = Arc::new(Shared::new(writer, events, traits.secret));

        let (control_tx, control_rx) = watch::channel(Control::Run);
        let (exit_tx, exit_rx) = watch::channel(None);

        tokio::spawn(write_loop(process.stdin, frames));
        let readers = Readers {
            stdout: tokio::spawn(read_stdout(process.stdout, shared.clone())),
            stderr: tokio::spawn(stderr_loop(process.stderr, shared.clone())),
        };
        tokio::spawn(wait_loop(process.child, control_rx, exit_tx, readers, shared.clone()));

        Ok(Self {
            shared,
            exited: exit_rx,
            control: control_tx,
            emitter: Mutex::new(Some(emitter)),
            stop_grace: options.stop_grace,
            terminate_on_stop: traits.terminate_on_stop,
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

    /// Fails once the process exited.
    pub fn ensure_running(&self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }

        Err(BusinessError::Process("the agent session has exited".to_owned()))
    }

    /// Changes to `Some` once the process exited.
    pub fn exit_watch(&self) -> watch::Receiver<Option<ExitInfo>> {
        self.exited.clone()
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

        if self.terminate_on_stop {
            self.control.send_if_modified(|control| {
                let running = *control == Control::Run;
                if running {
                    *control = Control::Terminate;
                }

                running
            });
        }

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
        self.control.send_replace(Control::Kill);
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
        self.control.send_replace(Control::Kill);
    }
}

/// Runs the startup `steps` within the deadline of `options`, unless `abort` resolves first.
pub(crate) async fn run_startup<T>(
    options: &SessionOptions,
    abort: StartAbort<'_>,
    steps: impl Future<Output = Result<T>>,
) -> Result<T> {
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

/// Passes stderr on (see [`LineForwarder`]).
async fn stderr_loop(stderr: Box<dyn AsyncRead + Send + Unpin>, shared: Arc<Shared>) {
    let mut lines = LineSplitter::new(stderr, MAX_STDERR_LINE);
    let mut forwarder = LineForwarder::default();

    while let Some(line) = lines.next_line().await {
        forwarder.pass(&shared, &line);
    }

    forwarder.finish(&shared);
}

/// Passes agent output on as redacted `stderr` events of at most [`MAX_STDERR_LINE`] bytes, at
/// most [`MAX_STDERR_PER_SECOND`] per second; a dropped burst is summarized in one line. Blank
/// lines are skipped. Split lines with [`LineSplitter::new`]`(…, MAX_STDERR_LINE)`.
#[derive(Default)]
pub(crate) struct LineForwarder {
    window: Option<Instant>,
    sent: u32,
    suppressed: u64,
}

impl LineForwarder {
    /// Passes `line` on, or counts it when the second's budget is spent.
    pub fn pass(&mut self, shared: &Shared, line: &Line) {
        let raw = String::from_utf8_lossy(&line.bytes);
        let masked = shared.mask(raw.trim_end(), line.overflowed);
        let text = truncate(redact_text(&masked), MAX_STDERR_LINE);

        if text.trim().is_empty() {
            return;
        }

        shared.remember_stderr(&text);

        let now = Instant::now();
        if self
            .window
            .is_none_or(|start| now.duration_since(start) >= Duration::from_secs(1))
        {
            self.window = Some(now);
            self.sent = 0;

            if self.suppressed > 0 {
                shared.emit(suppressed_line(self.suppressed));
                self.suppressed = 0;
                self.sent = 1;
            }
        }

        if self.sent < MAX_STDERR_PER_SECOND {
            shared.emit(AgentEvent::Stderr { line: text });
            self.sent += 1;
        } else {
            self.suppressed += 1;
        }
    }

    /// Reports the lines still suppressed at the end of the stream.
    pub fn finish(&mut self, shared: &Shared) {
        if self.suppressed > 0 {
            shared.emit(suppressed_line(self.suppressed));
            self.suppressed = 0;
        }
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
    mut control: watch::Receiver<Control>,
    exit: watch::Sender<Option<ExitInfo>>,
    readers: Readers,
    shared: Arc<Shared>,
) {
    let mut terminated = false;

    let result = loop {
        let wanted = tokio::select! {
            result = child.wait() => break result,
            // A dropped sender means the process is gone too: kill.
            changed = control.changed() => match changed {
                Ok(()) => *control.borrow_and_update(),
                Err(_) => Control::Kill,
            },
        };

        match wanted {
            Control::Run => {}
            Control::Terminate if !terminated => {
                terminated = true;
                child.terminate();
            }
            Control::Terminate => {}
            Control::Kill => {
                child.start_kill().ok();
                break child.wait().await;
            }
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
