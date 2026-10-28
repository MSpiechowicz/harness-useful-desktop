//! The PI-family RPC protocol (`--mode rpc`): framing ([`codec`]), request correlation
//! ([`pending`]), the per-harness [`RpcDialect`] and the command encoding and reply parsing the
//! dialects share ([`shared`]).
//!
//! The desktop stays on OMP protocol v1 (`negotiate_protocol` is never sent).

pub mod codec;
pub mod pending;
pub mod shared;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::error::Result;
use crate::harness::CommandCatalog;
use crate::model::{AgentAction, AgentEvent, ChatMessage, HarnessId, UiRequest, UiResponse};

pub use codec::{Frame, FrameFault, FrameReader, Line, LineSplitter, MAX_FRAME_BYTES, encode_frame, parse_frame};

/// Everything a dialect implementation needs: `use crate::pi_family::rpc::prelude::*;`.
pub mod prelude {
    pub use serde_json::Value;

    pub use super::shared::encode_shared;
    pub use super::{DecodeState, EncodeContext, Handshake, Inbound, RpcDialect, command_frame};
    pub use crate::error::{BusinessError, Result};
    pub use crate::harness::CommandCatalog;
    pub use crate::model::{AgentAction, AgentEvent, HarnessId, PromptStatus};
    pub use crate::pi_family::events::{decode_common, frame_type};
}
pub use pending::{PendingRequests, RequestKind, RpcResponse, await_response, timeout_for};

/// What a decoded frame means to the session.
#[derive(Debug, Clone, PartialEq)]
pub enum Inbound {
    /// OMP's startup frame, with its protocol version.
    Ready(Option<u32>),
    /// The answer to a request, correlated by id.
    Response(RpcResponse),
    /// Something to show.
    Event(AgentEvent),
    /// A dialog the user must answer.
    UiRequest(UiRequest),
    /// A frame to send back at once without involving the user (e.g. cancel an `ask`).
    AutoReply(Value),
    /// The agent's raw slash-command list (merged with the catalog by the session).
    Commands(Vec<Value>),
}

/// How the session confirms the agent is up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handshake {
    /// Wait for a `{"type":"ready"}` frame (OMP).
    ReadyFrame,
    /// No startup frame; the first `get_state` doubles as the probe (PI).
    Probe,
}

/// Inputs an encoder may need beyond the action.
#[derive(Debug, Clone, Copy)]
pub struct EncodeContext<'a> {
    /// The project's session folder ([`crate::harness::Harness::sessions_dir`]), for resolving a
    /// `SessionRef` to a file.
    pub sessions_dir: &'a Path,
}

/// Per-session decoding state, owned by the session's reader.
#[derive(Debug, Clone)]
pub struct DecodeState {
    project: PathBuf,
    minted: u64,
    open_messages: Vec<String>,
    tool_args: HashMap<String, (String, Value)>,
    open_prompts: Vec<String>,
    run_error: Option<String>,
    /// OMP's subagent steps, derived from its progress snapshots.
    pub(crate) subagent_steps: crate::harnesses::omp::SubagentSteps,
}

/// Tool calls whose arguments are kept until their end event (bounds memory on a stuck run).
const MAX_TRACKED_TOOLS: usize = 256;

/// Longest agent-chosen message id that is used as is.
const MAX_MESSAGE_ID: usize = 128;

impl DecodeState {
    /// State for a session running in `project`.
    pub fn new(project: impl Into<PathBuf>) -> Self {
        Self {
            project: project.into(),
            minted: 0,
            open_messages: Vec::new(),
            tool_args: HashMap::new(),
            open_prompts: Vec::new(),
            run_error: None,
            subagent_steps: Default::default(),
        }
    }

    /// Follows the session to another project (OMP `/move`).
    pub fn set_project(&mut self, project: PathBuf) {
        self.project = project;
    }

    /// The project directory.
    pub fn project(&self) -> &Path {
        &self.project
    }

    fn mint(&mut self) -> String {
        self.minted += 1;
        format!("msg-{}", self.minted)
    }

    fn usable(given: Option<&str>) -> Option<String> {
        given
            .filter(|id| !id.is_empty() && id.len() <= MAX_MESSAGE_ID)
            .map(str::to_owned)
    }

    /// The id of a starting message: the agent's (OMP) or a minted one (PI).
    pub fn open_message(&mut self, given: Option<&str>) -> String {
        let id = Self::usable(given).unwrap_or_else(|| self.mint());
        self.open_messages.push(id.clone());
        id
    }

    /// The id of the message a `message_update` belongs to.
    pub fn current_message(&mut self, given: Option<&str>) -> String {
        if let Some(id) = Self::usable(given) {
            return id;
        }

        match self.open_messages.last() {
            Some(id) => id.clone(),
            None => self.open_message(None),
        }
    }

    /// The id of an ending message.
    pub fn close_message(&mut self, given: Option<&str>) -> String {
        match Self::usable(given) {
            Some(id) => {
                self.open_messages.retain(|open| *open != id);
                id
            }
            None => self.open_messages.pop().unwrap_or_else(|| self.mint()),
        }
    }

    /// Keeps the arguments of a file-editing tool call for its touched paths.
    pub fn remember_tool(&mut self, tool_call_id: &str, name: &str, args: &Value) {
        if !super::touched::is_edit_tool(name) || self.tool_args.len() >= MAX_TRACKED_TOOLS {
            return;
        }

        self.tool_args
            .insert(tool_call_id.to_owned(), (name.to_owned(), args.clone()));
    }

    /// The remembered arguments of tool call `tool_call_id`.
    pub fn take_tool_args(&mut self, tool_call_id: &str) -> Option<Value> {
        self.tool_args.remove(tool_call_id).map(|(_, args)| args)
    }

    /// Records the outcome of the latest assistant message (PI prompt results report it).
    pub fn note_assistant_end(&mut self, message: &ChatMessage) {
        self.run_error = match message.stop_reason.as_deref() {
            Some("error") => Some(
                message
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "The model call failed".to_owned()),
            ),
            _ => None,
        };
    }

    /// Waits for the run to settle before reporting prompt `id` (PI).
    pub fn track_prompt(&mut self, id: &str) {
        self.open_prompts.push(id.to_owned());
    }

    /// The prompts waiting for the run to settle and the run's error, clearing both.
    pub fn settle_prompts(&mut self) -> (Vec<String>, Option<String>) {
        (std::mem::take(&mut self.open_prompts), self.run_error.take())
    }
}

/// One harness's flavour of the RPC protocol.
pub trait RpcDialect: Send + Sync {
    /// The harness speaking this dialect.
    fn harness(&self) -> HarnessId;

    /// The harness's slash-command catalog (merges the live command list).
    fn catalog(&self) -> &'static dyn CommandCatalog;

    /// How startup is confirmed.
    fn handshake(&self) -> Handshake;

    /// Commands sent (without `id`) right after the handshake, before `get_state`.
    fn after_handshake(&self) -> Vec<Value> {
        Vec::new()
    }

    /// The prompt text that moves the running session (conversation file and working directory)
    /// to `project`, or `None` when the harness cannot move a session.
    fn move_prompt(&self, _project: &str) -> Option<String> {
        None
    }

    /// Like [`RpcDialect::after_handshake`], but a refusal (an older harness without the command)
    /// does not fail the startup.
    fn optional_after_handshake(&self) -> Vec<Value> {
        Vec::new()
    }

    /// The command frame for `action` with request `id`. `getHistory` is handled by
    /// [`RpcDialect::history_request`]; unsupported actions fail with `Unsupported`.
    fn encode(&self, action: &AgentAction, id: &str, context: &EncodeContext<'_>) -> Result<Value>;

    /// The command reading the history: a page starting at `cursor` when
    /// [`RpcDialect::history_paged`], else every message.
    fn history_request(&self, cursor: Option<&str>, id: &str) -> Value;

    /// True when [`RpcDialect::history_request`] returns pages.
    fn history_paged(&self) -> bool;

    /// The command reading every message in one reply: the fallback when a paged read keeps being
    /// refused. A reply too large for one frame is refused by the agent.
    fn whole_history_request(&self, id: &str) -> Value {
        command_frame("get_messages", id, Vec::<(&str, Value)>::new())
    }

    /// The `bash` command.
    fn encode_bash(&self, command: &str, id: &str) -> Value {
        command_frame("bash", id, [("command", Value::from(command))])
    }

    /// Decodes one frame.
    fn decode(&self, frame: Value, state: &mut DecodeState) -> Vec<Inbound> {
        super::events::decode_common(&frame, state)
    }

    /// Reacts to a successful `prompt` acknowledgement in stream order (e.g. a prompt the harness
    /// handled itself is complete at once).
    fn on_prompt_ack(&self, request_id: &str, data: Option<&Value>, state: &mut DecodeState) -> Vec<AgentEvent>;

    /// The reply frame for a dialog answer.
    fn encode_ui_response(&self, response: &UiResponse) -> Value {
        super::ui_request::encode_ui_response(response)
    }
}

/// `{"id": id, "type": kind, ...params}`; `None` parameters are left out.
pub fn command_frame<I, K>(kind: &str, id: &str, params: I) -> Value
where
    I: IntoIterator<Item = (K, Value)>,
    K: Into<String>,
{
    let mut frame = Map::new();
    frame.insert("id".to_owned(), Value::from(id));
    frame.insert("type".to_owned(), Value::from(kind));

    for (key, value) in params {
        if !value.is_null() {
            frame.insert(key.into(), value);
        }
    }

    Value::Object(frame)
}
