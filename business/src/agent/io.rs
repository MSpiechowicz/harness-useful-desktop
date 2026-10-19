//! State shared by a session's tasks, and the RPC stdout reader (frames → dialect → routing).
//!
//! The other tasks (stdin writer, stderr reader, exit waiter) belong to the shared process
//! lifecycle in `process.rs`.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::{mpsc, oneshot};

use super::process::Stdout;
use super::sink::{EmitMsg, PromptTaken};
use crate::model::AgentEvent;
use crate::pi_family::rpc::{
    DecodeState, Frame, FrameReader, Inbound, PendingRequests, RequestKind, RpcDialect, encode_frame,
};

/// Stderr lines kept for startup error messages.
const STDERR_TAIL: usize = 5;

/// Messages to the stdin writer.
pub(crate) enum WriterMsg {
    /// Bytes of one encoded frame.
    Frame(Vec<u8>),
    /// Close stdin (agents treat EOF as shutdown).
    Close,
}

/// State shared by a session's tasks.
pub(crate) struct Shared {
    pub writer: mpsc::Sender<WriterMsg>,
    pub pending: PendingRequests,
    pub events: mpsc::UnboundedSender<EmitMsg>,
    dialogs: Mutex<HashSet<String>>,
    stderr_tail: Mutex<VecDeque<String>>,
    /// A project the session moved to, applied by the reader before it decodes the next frame.
    project_change: Mutex<Option<PathBuf>>,
    /// Told in event order when the agent takes the next prompt.
    prompt_taken: Mutex<Option<PromptTaken>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    pub fn new(writer: mpsc::Sender<WriterMsg>, events: mpsc::UnboundedSender<EmitMsg>) -> Self {
        Self {
            writer,
            pending: PendingRequests::default(),
            events,
            dialogs: Mutex::new(HashSet::new()),
            stderr_tail: Mutex::new(VecDeque::new()),
            project_change: Mutex::new(None),
            prompt_taken: Mutex::new(None),
        }
    }

    /// Calls `taken` in event order when the agent takes the next prompt (see [`PromptTaken`]),
    /// replacing a call set before.
    pub fn on_prompt_taken(&self, taken: PromptTaken) {
        *lock(&self.prompt_taken) = Some(taken);
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

    /// Keeps `line` for [`Shared::stderr_tail`].
    pub fn remember_stderr(&self, line: &str) {
        let mut tail = lock(&self.stderr_tail);

        if tail.len() == STDERR_TAIL {
            tail.pop_front();
        }

        tail.push_back(line.to_owned());
    }
}

/// Reads stdout, decodes every frame and routes the result; fails waiting requests at EOF.
pub(crate) async fn read_loop(
    stdout: Stdout,
    shared: Arc<Shared>,
    dialect: &'static dyn RpcDialect,
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
            Frame::Json(value) => dialect.decode(value, &mut state),
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
            route(&shared, dialect, item, &mut state, &mut ready).await;
        }
    }

    shared.pending.close("the agent closed its output");
}

async fn route(
    shared: &Shared,
    dialect: &'static dyn RpcDialect,
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
                for event in dialect.on_prompt_ack(id, response.data.as_ref(), state) {
                    shared.emit(event);
                }

                if let Some(taken) = lock(&shared.prompt_taken).take() {
                    shared.events.send(EmitMsg::Call(taken)).ok();
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
            commands: dialect.catalog().merged(&raw),
        }),
    }
}
