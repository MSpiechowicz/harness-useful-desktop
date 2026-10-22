//! Codex's frames ([`Inbound`]) as the chat's [`AgentEvent`]s, with [`Hint`]s next to them for what
//! the engine acts on itself. Pure and deterministic: no I/O, one [`CodexMapper`] per chat process.
//!
//! - **Turns** (see [`turns`]): `turn/started` is `agentStart` and one `turnStart` (Codex does not
//!   report its model calls one by one), `turn/completed` ends what is still open, then `turnEnd`,
//!   `agentEnd`, a `promptResult` (`completed`, `aborted` when interrupted, `error` with the
//!   failure's first line) and `settled` once the chat is idle. A compaction or a `!command` the
//!   engine started ([`StartKind`]) runs as a quiet turn: its compaction or tool card shows, no
//!   `agentStart` … `settled`.
//! - **Replies** (see [`replies`]): each agent message is one assistant message streamed from its
//!   deltas, confirmed by its `item/completed` (its `phase`, `commentary` or `final_answer`, is the
//!   message's stop reason). Reasoning with text is a message with one thinking block; reasoning
//!   without text (Codex's default) shows nothing. Both stop growing at [`MAX_STREAMED_BYTES`].
//!   An assistant message's `id` is its turn's id: a fork at it keeps that turn.
//! - **Tool cards** (see [`tools`]): commands as `shell`, file changes as `apply_patch` (with the
//!   project files they touched), MCP calls as `mcp__<server>__<tool>`, web searches as
//!   `web_search`.
//! - **User messages** are never shown (the engine shows what it sent): their `item/completed` is
//!   a [`Hint::UserMessage`], which tells the engine a prompt or steer was taken.
//! - **Requests** of Codex are [`Hint::Ask`] ([`asks`]; a file change's files come from its item),
//!   or [`Hint::Unsupported`] to refuse.
//! - **Thread filter:** once the engine names its thread ([`CodexMapper::follow_thread`]),
//!   notifications about other threads are dropped.

mod asks;
mod replies;
mod tools;
mod turns;

use std::collections::VecDeque;
use std::path::Path;

pub use self::asks::{Ask, AskKind};
use self::replies::Replies;
use self::tools::Tools;
pub(crate) use self::tools::saved_card;
pub use self::tools::{COMMAND_DECLINED, FILE_CHANGE_DECLINED, UNFINISHED_TOOL};
use self::turns::Turn;
pub use self::turns::{SIGN_IN_ERROR, TurnEnd};
use super::wire::items::ThreadItem;
use super::wire::notifications::{ItemDelta, ItemNotification, Notification, ThreadTokenUsage};
use super::wire::results::{Thread, ThreadStatus};
use super::wire::{Inbound, RequestId, Response, ServerRequest, decode};
use crate::model::{AgentEvent, NoticeLevel};
use crate::pi_family::normalize::plain_text;
use crate::security::redact_text;

/// The most bytes of streamed text one reply holds, and of live output one tool card shows.
pub const MAX_STREAMED_BYTES: usize = 8 * 1024 * 1024;

/// One output of the mapper, in order: a chat event (boxed, it is far larger than a hint) or a
/// hint for the engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Mapped {
    Event(Box<AgentEvent>),
    Hint(Hint),
}

/// The chat events among `mapped`, in order.
pub fn events(mapped: &[Mapped]) -> Vec<AgentEvent> {
    let mut events = Vec::new();

    for item in mapped {
        if let Mapped::Event(event) = item {
            events.push(event.as_ref().clone());
        }
    }

    events
}

/// What the engine learns from a frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Hint {
    /// The answer to one of the engine's requests.
    Response(Response),
    /// A thread was started, resumed or forked in this process.
    ThreadStarted(Box<Thread>),
    /// What the thread does (`active` with what it waits for, `idle`, `systemError`).
    ThreadStatus(ThreadStatus),
    /// The thread was renamed.
    Title(String),
    /// The thread's token counts (statistics, the context meter).
    TokenUsage(ThreadTokenUsage),
    /// Turn `0` began.
    TurnStarted(String),
    /// A turn ended.
    TurnEnded(TurnEnd),
    /// Codex took what the user sent (a prompt's or a steer's `userMessage`).
    UserMessage(UserMessage),
    /// Codex asks the user something; answer under its request id.
    Ask(Ask),
    /// A request the app refuses (answer with `answers::refusal`).
    Unsupported(Box<ServerRequest>),
    /// The request was answered or withdrawn: its dialog closes.
    Resolved(RequestId),
    /// An API the app uses is deprecated (for the log, never the user).
    Deprecation(String),
}

/// A `userMessage` item Codex took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserMessage {
    pub turn_id: String,
    pub item_id: String,
    /// The `clientUserMessageId` the engine sent it with.
    pub client_id: Option<String>,
    pub text: String,
}

/// What the engine started a turn with (see [`CodexMapper::sent`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StartKind {
    /// `turn/start`.
    #[default]
    Prompt,
    /// `thread/compact/start`: a quiet turn, its compaction is `manual`.
    Compact,
    /// `thread/shellCommand`: a quiet turn, its result is shown only when it fails.
    Shell,
}

/// The outputs of one frame: events the parts of the mapper add, each hint after the events
/// before it.
#[derive(Default)]
struct Out {
    events: Vec<AgentEvent>,
    ordered: Vec<Mapped>,
}

impl Out {
    fn event(&mut self, event: AgentEvent) {
        self.events.push(event);
    }

    fn hint(&mut self, hint: Hint) {
        self.ordered = self.drained();
        self.ordered.push(Mapped::Hint(hint));
    }

    /// Everything so far, in order.
    fn drained(&mut self) -> Vec<Mapped> {
        let mut ordered = std::mem::take(&mut self.ordered);
        ordered.extend(self.events.drain(..).map(Box::new).map(Mapped::Event));
        ordered
    }

    fn finish(mut self) -> Vec<Mapped> {
        self.drained()
    }
}

/// Maps the frames of one chat process (see the module docs).
pub struct CodexMapper {
    /// The thread the chat follows, once the engine named it.
    thread: Option<String>,
    turn: Turn,
    /// Turns the engine started that have not begun yet, oldest first.
    pending: VecDeque<StartKind>,
    /// The engine's follow-up queue holds messages: a turn's end does not settle the chat.
    held: bool,
    /// A turn ended while the chat was not idle: settle (aborted or not) once it is.
    deferred: Option<bool>,
    replies: Replies,
    tools: Tools,
}

impl CodexMapper {
    /// A mapper for a chat in `project` (touched paths are relative to it).
    pub fn new(project: &Path) -> Self {
        Self {
            thread: None,
            turn: Turn::default(),
            pending: VecDeque::new(),
            held: false,
            deferred: None,
            replies: Replies::default(),
            tools: Tools::new(project.to_path_buf()),
        }
    }

    /// The chat follows thread `id` (after `thread/start`, `resume` or `fork`); notifications about
    /// other threads are dropped from now on.
    pub fn follow_thread(&mut self, id: &str) {
        self.thread = Some(id.to_owned()).filter(|id| !id.is_empty());
    }

    /// The thread the chat follows.
    pub fn thread_id(&self) -> Option<&str> {
        self.thread.as_deref()
    }

    /// The running turn (the one `turn/steer` and `turn/interrupt` name).
    pub fn turn_id(&self) -> Option<&str> {
        self.turn.id()
    }

    /// Reads one stdout line and maps it; a line that is not a frame is a `frameError`.
    pub fn on_line(&mut self, line: &str) -> Vec<Mapped> {
        match decode(line) {
            Ok(frame) => self.on_frame(&frame),
            Err(error) => vec![Mapped::Event(error.event().into())],
        }
    }

    /// What `frame` means for the chat, in order.
    pub fn on_frame(&mut self, frame: &Inbound) -> Vec<Mapped> {
        let mut out = Out::default();

        match frame {
            Inbound::Response(response) => out.hint(Hint::Response(response.clone())),
            Inbound::Request(request) => self.request(request, &mut out),
            Inbound::Notification(notification) if self.follows(notification) => {
                self.notification(notification, &mut out);
            }
            Inbound::Notification(_) => {}
        }

        out.finish()
    }

    /// The engine is about to start a turn of `kind`; call before writing its request.
    pub fn sent(&mut self, kind: StartKind) {
        self.pending.push_back(kind);
    }

    /// The newest turn the engine started will not begin (its request failed or could not be
    /// written): the chat settles if nothing else runs.
    pub fn withdrawn(&mut self) -> Vec<Mapped> {
        self.pending.pop_back();

        if self.pending.is_empty() {
            self.turn.forget_stop();
        }

        let mut out = Out::default();
        self.settle_if_idle(&mut out);
        out.finish()
    }

    /// Whether the engine's follow-up queue holds messages. While it does, a turn's end (unless
    /// aborted) does not settle the chat: the next follow-up's turn follows. Once it is empty
    /// again the chat settles if idle.
    pub fn follow_ups_waiting(&mut self, waiting: bool) -> Vec<Mapped> {
        self.held = waiting;

        let mut out = Out::default();
        self.settle_if_idle(&mut out);
        out.finish()
    }

    /// The user asked to stop the running turn: when it then fails, it ends as aborted.
    pub fn abort_requested(&mut self) {
        if self.is_working() {
            self.turn.abort_requested();
        }
    }

    /// True while a turn runs or one the engine started has not begun.
    pub fn is_working(&self) -> bool {
        self.turn.is_active() || !self.pending.is_empty()
    }

    /// True while a compaction runs.
    pub fn is_compacting(&self) -> bool {
        self.turn.is_compacting()
    }

    fn follows(&self, notification: &Notification) -> bool {
        match (&self.thread, notification.thread_id()) {
            (Some(followed), Some(id)) => followed == id,
            _ => true,
        }
    }

    fn request(&mut self, request: &ServerRequest, out: &mut Out) {
        match asks::ask(request, &self.tools) {
            Some(ask) => out.hint(Hint::Ask(ask)),
            None => out.hint(Hint::Unsupported(Box::new(request.clone()))),
        }
    }

    fn notification(&mut self, notification: &Notification, out: &mut Out) {
        match notification {
            Notification::ThreadStarted(thread) => out.hint(Hint::ThreadStarted(thread.clone())),
            Notification::ThreadStatusChanged(changed) => out.hint(Hint::ThreadStatus(changed.status.clone())),
            Notification::ThreadNameUpdated(updated) => {
                let name = updated.thread_name.as_deref().map(str::trim).unwrap_or_default();

                if !name.is_empty() {
                    out.hint(Hint::Title(name.to_owned()));
                }
            }
            Notification::TokenUsageUpdated(updated) => out.hint(Hint::TokenUsage(updated.token_usage)),
            Notification::TurnStarted(started) => self.turn_started(&started.turn.id, out),
            Notification::TurnCompleted(completed) => self.turn_completed(&completed.turn, out),
            Notification::ItemStarted(item) => self.item_started(item, out),
            Notification::ItemCompleted(item) => self.item_completed(item, out),
            Notification::AgentMessageDelta(delta) => {
                if self.step(&delta.turn_id, out) {
                    self.replies
                        .text_delta(&delta.item_id, &delta.turn_id, &delta.delta, &mut out.events);
                }
            }
            Notification::ReasoningTextDelta(delta)
            | Notification::ReasoningSummaryTextDelta(delta)
            | Notification::ReasoningSummaryPartAdded(delta) => self.thinking(notification, delta, out),
            Notification::CommandOutputDelta(delta) => self.tools.output(&delta.item_id, &delta.delta, &mut out.events),
            Notification::McpToolCallProgress(delta) => {
                self.tools.progress(&delta.item_id, &delta.delta, &mut out.events);
            }
            Notification::FileChangePatchUpdated(updated) => {
                self.tools.patch_updated(&updated.item_id, &updated.changes)
            }
            Notification::Error(error) => self.error(error, out),
            Notification::Warning(warning) => notice(NoticeLevel::Warning, &warning.message, out),
            Notification::DeprecationNotice(notice) => out.hint(Hint::Deprecation(notice.summary.clone())),
            Notification::ServerRequestResolved(resolved) => out.hint(Hint::Resolved(resolved.request_id.clone())),
            Notification::Folded { .. } | Notification::Unknown { .. } => {}
        }
    }

    fn thinking(&mut self, notification: &Notification, delta: &ItemDelta, out: &mut Out) {
        if !self.step(&delta.turn_id, out) {
            return;
        }

        let events = &mut out.events;

        match notification {
            Notification::ReasoningSummaryPartAdded(_) => {
                self.replies.thinking_part(&delta.item_id, delta.summary_index)
            }
            Notification::ReasoningSummaryTextDelta(_) => {
                self.replies.thinking_part(&delta.item_id, delta.summary_index);
                self.replies
                    .thinking_delta(&delta.item_id, &delta.turn_id, &delta.delta, events);
            }
            _ => self
                .replies
                .thinking_delta(&delta.item_id, &delta.turn_id, &delta.delta, events),
        }
    }

    fn item_started(&mut self, notification: &ItemNotification, out: &mut Out) {
        if !self.step(&notification.turn_id, out) {
            return;
        }

        let turn_id = &notification.turn_id;
        let events = &mut out.events;

        match &notification.item {
            ThreadItem::AgentMessage(message) => self.replies.message_started(message, turn_id, events),
            ThreadItem::ContextCompaction { .. } => self.compaction_started(out),
            item => self.tools.started(item, events),
        }
    }

    fn item_completed(&mut self, notification: &ItemNotification, out: &mut Out) {
        let turn_id = &notification.turn_id;

        if !self.step(turn_id, out) {
            return;
        }

        let at = notification.at_ms;

        match &notification.item {
            ThreadItem::UserMessage(message) => {
                self.turn.user_message(message.client_id.as_deref());
                out.hint(Hint::UserMessage(UserMessage {
                    turn_id: turn_id.clone(),
                    item_id: message.id.clone(),
                    client_id: message.client_id.clone(),
                    text: message.text(),
                }));
            }
            ThreadItem::AgentMessage(message) => {
                self.replies.message_completed(message, turn_id, at, &mut out.events);
            }
            ThreadItem::Reasoning(reasoning) => {
                self.replies
                    .reasoning_completed(reasoning, turn_id, at, &mut out.events);
            }
            ThreadItem::ContextCompaction { .. } => self.compaction_completed(out),
            item => self.tools.completed(item, &mut out.events),
        }
    }
}

/// A redacted notice of Codex's `text`, unless it is empty.
fn notice(level: NoticeLevel, text: &str, out: &mut Out) {
    let message = redact_text(plain_text(text).trim());

    if !message.is_empty() {
        out.event(AgentEvent::Notice { level, message });
    }
}
