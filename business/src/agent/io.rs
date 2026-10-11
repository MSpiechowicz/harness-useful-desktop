//! The four tasks of a running session: stdin writer, stdout reader (frames → dialect → routing),
//! stderr reader (redacted, rate-limited lines) and the exit waiter.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use super::sink::EmitMsg;
use crate::model::AgentEvent;
use crate::pi_family::rpc::{
    DecodeState, Frame, FrameReader, Inbound, LineSplitter, PendingRequests, RequestKind, RpcDialect, encode_frame,
};
use crate::process::{ChildHandle, ExitInfo};
use crate::security::redact_text;

/// Longest stderr line passed on (2 KiB).
pub const MAX_STDERR_LINE: usize = 2048;

/// Stderr lines passed on per second at most.
pub const MAX_STDERR_PER_SECOND: u32 = 20;

/// Stderr lines kept for startup error messages.
const STDERR_TAIL: usize = 5;

/// How long the waiter lets the readers drain after the process exited.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// Messages to the stdin writer.
pub(crate) enum WriterMsg {
    /// Bytes of one encoded frame.
    Frame(Vec<u8>),
    /// Close stdin (agents treat EOF as shutdown).
    Close,
}

/// State shared by a session's tasks.
pub(crate) struct Shared {
    pub dialect: &'static dyn RpcDialect,
    pub writer: mpsc::Sender<WriterMsg>,
    pub pending: PendingRequests,
    pub events: mpsc::UnboundedSender<EmitMsg>,
    dialogs: Mutex<HashSet<String>>,
    stderr_tail: Mutex<VecDeque<String>>,
    /// A project the session moved to, applied by the reader before it decodes the next frame.
    project_change: Mutex<Option<PathBuf>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    pub fn new(
        dialect: &'static dyn RpcDialect,
        writer: mpsc::Sender<WriterMsg>,
        events: mpsc::UnboundedSender<EmitMsg>,
    ) -> Self {
        Self {
            dialect,
            writer,
            pending: PendingRequests::default(),
            events,
            dialogs: Mutex::new(HashSet::new()),
            stderr_tail: Mutex::new(VecDeque::new()),
            project_change: Mutex::new(None),
        }
    }

    /// Makes the reader resolve paths (touched files) against `project` from the next frame on.
    pub fn change_project(&self, project: PathBuf) {
        *lock(&self.project_change) = Some(project);
    }

    pub fn emit(&self, event: AgentEvent) {
        self.events.send(EmitMsg::Event(Box::new(event))).ok();
    }

    pub fn has_open_dialog(&self) -> bool {
        !lock(&self.dialogs).is_empty()
    }

    /// Forgets dialog `id`; false when it was not open.
    pub fn close_dialog(&self, id: &str) -> bool {
        lock(&self.dialogs).remove(id)
    }

    /// The last stderr lines, joined.
    pub fn stderr_tail(&self) -> String {
        lock(&self.stderr_tail).iter().cloned().collect::<Vec<_>>().join(" | ")
    }

    fn remember_stderr(&self, line: &str) {
        let mut tail = lock(&self.stderr_tail);

        if tail.len() == STDERR_TAIL {
            tail.pop_front();
        }

        tail.push_back(line.to_owned());
    }
}

/// Writes frames to stdin until told to close (or the pipe breaks), then closes stdin.
pub(crate) async fn write_loop(mut stdin: Box<dyn AsyncWrite + Send + Unpin>, mut frames: mpsc::Receiver<WriterMsg>) {
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

/// Reads stdout, decodes every frame and routes the result; fails waiting requests at EOF.
pub(crate) async fn read_loop(
    stdout: Box<dyn AsyncRead + Send + Unpin>,
    shared: Arc<Shared>,
    ready: oneshot::Sender<Option<u32>>,
    mut state: DecodeState,
) {
    let mut frames = FrameReader::new(stdout);
    let mut ready = Some(ready);

    while let Some(frame) = frames.next_frame().await {
        if let Some(project) = lock(&shared.project_change).take() {
            state.set_project(project);
        }

        let inbound = match frame {
            Frame::Json(value) => shared.dialect.decode(value, &mut state),
            Frame::Fault(fault) => {
                if let Some(id) = &fault.response_id {
                    shared.pending.fail(
                        id,
                        crate::BusinessError::Protocol("the response exceeded the frame limit".to_owned()),
                    );
                }

                vec![Inbound::Event(AgentEvent::FrameError {
                    original_type: fault.original_type,
                    error: fault.error,
                })]
            }
        };

        for item in inbound {
            route(&shared, item, &mut state, &mut ready).await;
        }
    }

    shared.pending.close("the agent closed its output");
}

async fn route(
    shared: &Shared,
    inbound: Inbound,
    state: &mut DecodeState,
    ready: &mut Option<oneshot::Sender<Option<u32>>>,
) {
    match inbound {
        Inbound::Ready(version) => {
            if let Some(ready) = ready.take() {
                ready.send(version).ok();
            }
        }
        Inbound::Response(response) => {
            let prompt = response
                .id
                .as_deref()
                .filter(|id| response.success && shared.pending.kind(id) == Some(RequestKind::Prompt));

            if let Some(id) = prompt {
                for event in shared.dialect.on_prompt_ack(id, response.data.as_ref(), state) {
                    shared.emit(event);
                }
            }

            shared.pending.resolve(response);
        }
        Inbound::Event(event) => {
            if let AgentEvent::UiCancel { request_id } = &event {
                shared.close_dialog(request_id);
            }

            shared.emit(event);
        }
        Inbound::UiRequest(request) => {
            lock(&shared.dialogs).insert(request.id.clone());
            shared.emit(AgentEvent::UiRequest { request });
        }
        Inbound::AutoReply(frame) => {
            shared.writer.send(WriterMsg::Frame(encode_frame(&frame))).await.ok();
        }
        Inbound::Commands(raw) => shared.emit(AgentEvent::CommandsUpdated {
            commands: shared.dialect.catalog().merged(&raw),
        }),
    }
}

/// Passes stderr on as redacted lines of at most [`MAX_STDERR_LINE`] bytes, at most
/// [`MAX_STDERR_PER_SECOND`] per second; a dropped burst is summarized in one line.
pub(crate) async fn stderr_loop(stderr: Box<dyn AsyncRead + Send + Unpin>, shared: Arc<Shared>) {
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
pub(crate) struct Readers {
    pub stdout: JoinHandle<()>,
    pub stderr: JoinHandle<()>,
}

/// Waits for the process (killing it when `kill` is set), lets the readers drain, fails what is
/// still waiting, publishes the exit and emits `exited` as the session's last event.
pub(crate) async fn wait_loop(
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
