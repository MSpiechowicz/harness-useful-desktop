//! Claude Code's frames ([`Inbound`]) as the chat's [`AgentEvent`]s, with [`Hint`]s next to them
//! for what the engine acts on itself. Pure and deterministic: no I/O, one [`ClaudeMapper`] per
//! chat process.
//!
//! - **Turns** (see [`turns`]): a model turn starts with its first request (`system/status
//!   requesting`, or the first reply frame) as `agentStart`, each API call is a `turnStart` …
//!   `turnEnd`, and the turn's one `result` ends it: what is still open ends, then `agentEnd`, a
//!   `promptResult` and `settled` once no sent message waits in the CLI's queue. A `result` of a
//!   turn that never asked the model (a `shouldQuery: false` message, `/compact`, a local command)
//!   is quiet: no `agentStart` … `settled`, only a `promptResult` (`handled`) unless the engine sent
//!   it as a quiet message.
//! - **Replies** (see [`replies`]): streamed from the `stream_event` frames, confirmed by the
//!   `assistant` frames; **tool cards** (see [`tools`]) with touched paths; **subagents** from the
//!   `task_*` frames (see [`tasks`]). Frames of a subagent (`parent_tool_use_id`) never reach the
//!   chat's turn or messages, whenever they arrive: its `assistant` and `user` frames become the
//!   subagent's steps (see [`subagent_steps`]) and its `stream_event` frames are dropped.
//! - **Bounds:** a reply's streamed text and a tool call's streamed input stop growing at
//!   [`MAX_STREAMED_BYTES`] (the reply ends with a note that it was cut).
//! - **User frames** are never shown as the user's messages (the engine shows what it sent): tool
//!   results end tool cards, `[Request interrupted by user]` aborts the turn, a compaction summary
//!   is skipped, and `<local-command-stdout>` (from `set_model`, a local command) is a notice.
//! - **Errors:** the CLI's sign-in error becomes [`SIGN_IN_ERROR`]; other API errors and `error_*`
//!   results fail the turn with their first redacted line.
//! - **Hints:** the session id (only a well-formed one, a UUID), `system/init`, where each sent
//!   message is in the CLI's queue, the turn's outcome for statistics, compactions, permission
//!   prompts and questions ([`Ask`]), hook callbacks, replies to the engine's control requests,
//!   and session changes.

mod asks;
mod notes;
mod replies;
mod subagent_steps;
mod tasks;
mod tools;
mod turns;

use std::path::Path;

pub use self::asks::{Ask, PermissionAsk, Question, QuestionAsk, QuestionOption};
pub use self::replies::REPLY_CUT;
use self::replies::Replies;
use self::tasks::Tasks;
use self::tools::Tools;
pub use self::tools::{TOOL_INPUT_CUT, UNFINISHED_TOOL};
pub use self::turns::{SIGN_IN_ERROR, TurnEnd};
use self::turns::{Sends, Turn};
use crate::claude_stream::wire::control::{HookCallback, SlashCommand};
use crate::claude_stream::wire::messages::{AssistantFrame, ContentBlock, StreamEvent, StreamEventFrame, UserFrame};
use crate::claude_stream::wire::system::{CompactMetadata, Init, System};
use crate::claude_stream::wire::{
    CliRequest, CommandLifecycle, ControlRequestFrame, ControlResponse, ConversationReset, Inbound, LifecycleState,
    RateLimitInfo, decode,
};
use crate::harnesses::claude::is_session_id;
use crate::model::AgentEvent;

/// The most bytes of streamed text one open reply (its text and thinking) or one tool call's
/// streamed input holds; what streams past it is dropped, after a note.
pub const MAX_STREAMED_BYTES: usize = 8 * 1024 * 1024;

/// One output of the mapper, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Mapped {
    /// An event for the chat (boxed: it is far larger than a hint).
    Event(Box<AgentEvent>),
    /// Something for the engine.
    Hint(Hint),
}

impl Mapped {
    fn event(event: AgentEvent) -> Self {
        Self::Event(Box::new(event))
    }

    /// The chat event, when this is one.
    pub fn as_event(&self) -> Option<&AgentEvent> {
        match self {
            Self::Event(event) => Some(event),
            Self::Hint(_) => None,
        }
    }
}

/// The chat events of `mapped`, in order.
pub fn events(mapped: &[Mapped]) -> Vec<AgentEvent> {
    mapped.iter().filter_map(Mapped::as_event).cloned().collect()
}

/// What the engine learns from a frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Hint {
    /// The CLI works in session `id` (the first frame that names one, then every change: a fork
    /// gets its own id at spawn).
    SessionId(String),
    /// A turn (or quiet run) begins: model, permission mode, tools, slash commands, effort, ….
    Init(Box<Init>),
    /// The CLI queued the sent message `uuid`.
    PromptQueued(String),
    /// A turn took the sent message `uuid` (a follow-up's turn began, a steer was read).
    PromptStarted(String),
    /// The starting model request serves these sent messages (`system/status requesting`).
    PromptsTaken(Vec<String>),
    /// The CLI is done with the sent message `uuid`.
    PromptCompleted(String),
    /// The sent message `uuid` will not run (withdrawn, interrupted, failed).
    PromptCancelled(String),
    /// A turn's `result` arrived.
    TurnEnded(TurnEnd),
    /// A compaction replaced the history.
    Compacted(CompactMetadata),
    /// The permission mode changed (as the CLI names it).
    PermissionMode(String),
    /// The session was renamed.
    Title(String),
    /// The slash-command catalog changed.
    Commands(Vec<SlashCommand>),
    /// The CLI asks the user something; answer with the CLI's request id.
    Ask(Ask),
    /// A hook the app registered fired; answer with `hook_output`.
    HookCallback { request_id: String, callback: HookCallback },
    /// A request the app does not handle; answer with `control_error`.
    Unsupported { request_id: String, subtype: String },
    /// The CLI withdrew its request `request_id` (close its dialog).
    Withdrawn(String),
    /// The reply to one of the engine's control requests.
    Reply(ControlResponse),
    /// The account's rate limit state.
    RateLimit(RateLimitInfo),
    /// `idle`, `running` or `requires_action`.
    SessionState(String),
    /// The conversation was discarded (`/clear`).
    ConversationReset(ConversationReset),
}

/// How the engine sent a user message (see [`ClaudeMapper::sent`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendKind {
    /// A prompt sent while the chat is idle.
    Prompt,
    /// Steering (`priority: next`).
    Steer,
    /// A follow-up (`priority: later`).
    FollowUp,
    /// `/compact` (its compaction is `manual`).
    Compact,
    /// A message the model only sees later (`shouldQuery: false`): its result is not shown.
    Quiet,
}

/// Collects the events and hints of one frame in order.
#[derive(Default)]
struct Out {
    mapped: Vec<Mapped>,
    events: Vec<AgentEvent>,
}

impl Out {
    fn event(&mut self, event: AgentEvent) {
        self.events.push(event);
    }

    fn hint(&mut self, hint: Hint) {
        self.flush();
        self.mapped.push(Mapped::Hint(hint));
    }

    fn flush(&mut self) {
        self.mapped.extend(self.events.drain(..).map(Mapped::event));
    }

    fn finish(mut self) -> Vec<Mapped> {
        self.flush();
        self.mapped
    }
}

/// Maps the frames of one chat process (see the module docs).
pub struct ClaudeMapper {
    session_id: Option<String>,
    replies: Replies,
    tools: Tools,
    tasks: Tasks,
    turn: Turn,
    sends: Sends,
}

impl ClaudeMapper {
    /// A mapper for a chat in `project` (touched paths are relative to it).
    pub fn new(project: &Path) -> Self {
        Self {
            session_id: None,
            replies: Replies::default(),
            tools: Tools::new(project.to_path_buf()),
            tasks: Tasks::default(),
            turn: Turn::default(),
            sends: Sends::default(),
        }
    }

    /// The session id the CLI reported last.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Reads one stdout line and maps it; a line that is not a frame is a `frameError`.
    pub fn on_line(&mut self, line: &str) -> Vec<Mapped> {
        match decode(line) {
            Ok(frame) => self.on_frame(&frame),
            Err(error) => vec![Mapped::event(error.event())],
        }
    }

    /// What `frame` means for the chat, in order.
    pub fn on_frame(&mut self, frame: &Inbound) -> Vec<Mapped> {
        let mut out = Out::default();
        self.note_session(frame, &mut out);

        match frame {
            Inbound::System(system) => self.system(&system.kind, &mut out),
            Inbound::Assistant(assistant) => self.assistant(assistant, &mut out),
            Inbound::User(user) => self.user(user, &mut out),
            Inbound::StreamEvent(stream) => self.stream(stream, &mut out),
            Inbound::Result(result) => self.result(result, &mut out),
            Inbound::ControlRequest(request) => request_hint(request, &mut out),
            Inbound::ControlResponse(response) => out.hint(Hint::Reply(response.clone())),
            Inbound::ControlCancel { request_id } => out.hint(Hint::Withdrawn(request_id.clone())),
            Inbound::CommandLifecycle(lifecycle) => self.lifecycle(lifecycle, &mut out),
            Inbound::RateLimit(limit) => out.hint(Hint::RateLimit(limit.rate_limit_info.clone())),
            Inbound::ConversationReset(reset) => out.hint(Hint::ConversationReset(reset.clone())),
            Inbound::KeepAlive | Inbound::Unknown { .. } => {}
        }

        out.finish()
    }

    /// The engine is about to send user message `uuid` as `kind`; call before writing it.
    pub fn sent(&mut self, uuid: &str, kind: SendKind) {
        self.sends.sent(uuid, kind);
    }

    /// User message `uuid` will not run (it could not be written, or `cancel_async_message`
    /// withdrew it): the chat settles if nothing else waits.
    pub fn withdrawn(&mut self, uuid: &str) -> Vec<Mapped> {
        let mut out = Out::default();
        self.sends.done(uuid);
        self.settle_if_idle(&mut out);
        out.finish()
    }

    /// The user asked to stop the running turn (or the prompt on its way): when it then fails, it
    /// ends as aborted. Nothing while the chat is idle.
    pub fn abort_requested(&mut self) {
        if self.is_working() {
            self.turn.abort_requested();
        }
    }

    /// Whether subagent `id` runs: `None` for one the chat never saw.
    pub fn subagent_active(&self, id: &str) -> Option<bool> {
        self.tasks.is_active(id)
    }

    /// True while a compaction runs.
    pub fn is_compacting(&self) -> bool {
        self.turn.is_compacting()
    }

    /// True while a turn runs, a compaction runs or a sent message waits in the CLI's queue.
    pub fn is_working(&self) -> bool {
        self.turn.is_active() || self.turn.is_compacting() || self.sends.waiting()
    }

    /// Notes the session id `frame` names; one that is not a Claude Code session id (a UUID) is
    /// ignored, as it names files and folders of the session.
    fn note_session(&mut self, frame: &Inbound, out: &mut Out) {
        let Some(id) = frame.session_id().filter(|id| is_session_id(id)) else {
            return;
        };

        if self.session_id.as_deref() != Some(id) {
            self.session_id = Some(id.to_owned());
            out.hint(Hint::SessionId(id.to_owned()));
        }
    }

    fn system(&mut self, kind: &System, out: &mut Out) {
        match kind {
            System::Init(init) => out.hint(Hint::Init(init.clone())),
            System::Status(status) => self.status(status, out),
            System::CompactBoundary(boundary) => self.compact_boundary(&boundary.compact_metadata, out),
            System::TaskStarted(task) => self.tasks.started(task, &mut out.events),
            System::TaskProgress(task) => self.tasks.progress(task, &mut out.events),
            System::TaskUpdated(task) => self.tasks.updated(task, &mut out.events),
            System::TaskNotification(task) => self.tasks.ended(task, &mut out.events),
            System::SessionTitleChanged { title } => out.hint(Hint::Title(title.clone())),
            System::CommandsChanged { commands } => out.hint(Hint::Commands(commands.clone())),
            System::ApiRetry(retry) => self.retry_started(retry, out),
            System::SessionStateChanged { state } => out.hint(Hint::SessionState(state.clone())),
            System::LocalCommandOutput { content } => notes::command_output(content, out),
            System::Notification(notification) => notes::notification(notification, out),
            System::Hook(_)
            | System::ThinkingTokens { .. }
            | System::PermissionDenied { .. }
            | System::BackgroundTasksChanged
            | System::Unknown { .. } => {}
        }
    }

    fn stream(&mut self, frame: &StreamEventFrame, out: &mut Out) {
        if frame.parent_tool_use_id.is_some() {
            return;
        }

        let id = match (&frame.event, &frame.api_message_id) {
            (StreamEvent::MessageStart { message }, _) if !message.id.is_empty() => message.id.clone(),
            (_, Some(id)) => id.clone(),
            _ => self.replies.current().unwrap_or_default().to_owned(),
        };

        let events = &mut out.events;

        match &frame.event {
            StreamEvent::MessageStart { message } => {
                self.step(&id, events);
                self.replies.started(&id, message.model.as_deref(), events);
            }
            StreamEvent::ContentBlockStart { index, content_block } => {
                self.step(&id, events);
                self.replies
                    .block_started(&id, *index, content_block, &mut self.tools, events);
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                self.replies.delta(&id, *index, delta, &mut self.tools, events);
            }
            StreamEvent::ContentBlockStop { index } => {
                self.replies.block_stopped(&id, *index, &mut self.tools, events);
            }
            StreamEvent::MessageDelta { delta, usage } => {
                self.replies.finishing(&id, delta.stop_reason.as_deref(), *usage);
            }
            StreamEvent::MessageStop => self.replies.stopped(&id, &self.tools, events),
            StreamEvent::Other => {}
        }
    }

    fn assistant(&mut self, frame: &AssistantFrame, out: &mut Out) {
        if let Some(parent) = &frame.parent_tool_use_id {
            self.tasks.child_assistant(parent, frame, &mut out.events);
            return;
        }

        if frame.is_api_error_message {
            self.turn.ensure_started(&mut out.events);
            self.turn.api_error(frame);
            return;
        }

        self.step(&frame.message.id, &mut out.events);
        self.replies.assistant(frame, &mut self.tools, &mut out.events);
    }

    fn user(&mut self, frame: &UserFrame, out: &mut Out) {
        if let Some(parent) = &frame.parent_tool_use_id {
            self.tasks.child_user(parent, frame, &mut out.events);
            return;
        }

        if frame.is_synthetic {
            return;
        }

        for block in frame.message.content.blocks() {
            if let ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } = block
            {
                self.tools.result(tool_use_id, content, *is_error, &mut out.events);
            }
        }

        let Some(text) = frame.message.content.text() else {
            return;
        };

        if notes::is_interruption(&text) {
            self.turn.interrupted();
            return;
        }

        if !self.turn.is_compacting_run() {
            notes::local_command(&text, out);
        }
    }

    fn lifecycle(&mut self, lifecycle: &CommandLifecycle, out: &mut Out) {
        let uuid = lifecycle.command_uuid.clone();

        match lifecycle.state {
            LifecycleState::Queued => {
                self.sends.queued(&uuid);
                out.hint(Hint::PromptQueued(uuid));
            }
            LifecycleState::Started => {
                self.sends.started(&uuid);
                out.hint(Hint::PromptStarted(uuid));
            }
            LifecycleState::Completed => {
                self.sends.done(&uuid);
                out.hint(Hint::PromptCompleted(uuid));
                self.settle_if_idle(out);
            }
            LifecycleState::Cancelled => {
                self.sends.done(&uuid);
                out.hint(Hint::PromptCancelled(uuid));
                self.settle_if_idle(out);
            }
            LifecycleState::Other => {}
        }
    }

    /// A reply frame of API message `id`: the turn and the model step run.
    fn step(&mut self, id: &str, events: &mut Vec<AgentEvent>) {
        self.turn.ensure_started(events);
        self.turn.step(id, events);
    }
}

/// The hint of a request the CLI makes.
fn request_hint(request: &ControlRequestFrame, out: &mut Out) {
    let request_id = request.request_id.clone();

    if let Some(ask) = asks::ask(&request_id, &request.request) {
        out.hint(Hint::Ask(ask));
        return;
    }

    match &request.request {
        CliRequest::HookCallback(callback) => out.hint(Hint::HookCallback {
            request_id,
            callback: callback.clone(),
        }),
        CliRequest::Unknown { subtype } => out.hint(Hint::Unsupported {
            request_id,
            subtype: subtype.clone(),
        }),
        _ => {}
    }
}
