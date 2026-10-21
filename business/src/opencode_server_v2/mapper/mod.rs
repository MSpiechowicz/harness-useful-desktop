//! OpenCode 2 server events ([`Event`]) as the chat's [`AgentEvent`]s, with the same observable
//! output the OpenCode 1 mapper ([`crate::opencode_server::Mapper`]) gives. Pure: no I/O, one
//! [`Mapper`] per chat.
//!
//! - **Sessions:** only events of the chat's own session count. Subagent sessions started from it
//!   (`session.created` with a known `parentID`, at any depth) are tracked for their asks
//!   ([`asks`]); their messages and tool calls are not shown. Forks are other chats. Events
//!   without a session (`server.*`, catalog and project events) are ignored.
//! - **Turns, steering and the follow-up queue:** see [`turns`]. A turn is one execution
//!   (`session.execution.started` … `succeeded | failed | interrupted`); `session.status`
//!   (`busy`, `retry`, `idle`), which OpenCode 2.1.2 does not send, works too.
//! - **Replies:** one assistant message per model step, streamed from the text, reasoning and
//!   tool events, and the user's prompts once the run takes them (`session.inbox.delivered`). See
//!   [`replies`].
//! - **Tool calls:** see [`tools`]. **Asks** (permission requests and forms): see [`asks`].
//! - **Session changes** (title, agent, model), the step usage for the context meter and the
//!   session's running totals come out as [`Mapped::Session`], [`Mapped::Context`] and
//!   [`Mapped::Usage`].
//! - **Quiet runs** (`!command`s and compactions the app starts): see [`quiet`]. A compaction
//!   OpenCode starts on its own during a turn shows as `compaction` `start` and `end`.

mod asks;
mod form;
mod quiet;
mod replies;
mod tools;
mod turns;

use std::collections::{HashMap, VecDeque};
use std::path::Path;

use serde_json::Value;

use self::asks::Asks;
pub use self::asks::{Answer, PERMISSION_OPTIONS, Reply, ReplyKind};
use self::quiet::QuietRun;
pub use self::quiet::{QuietKind, ShellResult, compaction_started};
use self::replies::{PartKind, Replies};
pub use self::tools::ShellCall;
use self::tools::Tools;
use self::turns::{Outcome, Turn};
use super::events::{
    CompactionEnded, CompactionFailed, CompactionStarted, Event, EventKind, InboxEnqueued, SessionCreated,
    SessionStatus, Usage,
};
use super::messages::SessionMessage;
use super::wire::{Form, ModelRef, PermissionRequest};
use crate::error::Result;
use crate::model::{AgentEvent, ChatMessage, UiResponse};
use crate::opencode_server::wire::Outgoing;
use crate::security::redact_text;

/// Shown for an error OpenCode sent without a message.
const UNNAMED_ERROR: &str = "OpenCode reported an error";

/// What one event (or one change the engine reports) means for the chat.
#[derive(Debug, Clone, PartialEq)]
pub enum Mapped {
    /// An event for the chat (boxed: it is far larger than the other outputs).
    Event(Box<AgentEvent>),
    /// The agent took the prompt on its way: run the prompt-taken call now, before what follows.
    PromptTaken,
    /// The chat's session changed.
    Session(SessionChange),
    /// Send this held follow-up now (it already counts as a prompt on its way).
    Send(Outgoing),
    /// The user aborted before the turn started: interrupt the running turn now.
    Abort,
    /// What a finished step or a compaction tells about the context's size.
    Context(ContextNote),
    /// The session's running cost and token totals.
    Usage(Usage),
    /// A `!command` ended with this output (the end of its quiet run).
    ShellDone(ShellResult),
}

impl Mapped {
    /// [`Mapped::Event`] of `event`.
    pub fn event(event: AgentEvent) -> Self {
        Self::Event(Box::new(event))
    }
}

/// A change to the chat's session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionChange {
    /// Renamed (also the automatic title).
    Title(String),
    /// Another agent was selected.
    Agent(String),
    /// Another model (or variant) was selected.
    Model(ModelRef),
}

/// What the context holds, for the context meter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextNote {
    /// A step with output: the context holds `tokens` (input, output, reasoning and cache) of
    /// `model`.
    Tokens {
        /// The step's tokens.
        tokens: u64,
        /// The step's model.
        model: Option<ModelRef>,
    },
    /// A compaction replaced the context: its size is unknown until the next step.
    Compacted,
}

/// An ask still open on OpenCode's side, as it came (and whether a subagent asked).
#[derive(Debug, Clone)]
enum Asked {
    Permission(PermissionRequest, bool),
    Form(Form),
}

/// Maps the events of one chat (see the module docs).
pub struct Mapper {
    session: String,
    /// Subagent sessions and their directories.
    children: HashMap<String, Option<String>>,
    replies: Replies,
    tools: Tools,
    asks: Asks,
    /// Each ask still open on OpenCode's side, so a dialog whose answer did not reach OpenCode
    /// can be shown again ([`Mapper::reopen`]).
    asked: HashMap<String, Asked>,
    turn: Turn,
    /// The error the last turn ended with, until the next turn starts.
    last_error: Option<String>,
    /// Steering sent into the running turn, until the run takes it.
    steering: Vec<Outgoing>,
    /// Follow-ups held until the chat is idle.
    follow_ups: VecDeque<Outgoing>,
    /// Inbox items admitted and not yet taken by the run, with when they were admitted.
    inbox: HashMap<String, (InboxEnqueued, Option<u64>)>,
    /// The running quiet run.
    quiet: Option<QuietRun>,
    /// The generation of the last quiet run.
    quiet_generation: u64,
}

impl Mapper {
    /// A mapper for session `session` (`ses_…`) working in `directory` within `project` (both
    /// canonical).
    pub fn new(session: &str, project: &Path, directory: &Path) -> Self {
        Self {
            session: session.to_owned(),
            children: HashMap::new(),
            replies: Replies::default(),
            tools: Tools::new(project.to_path_buf(), directory.to_path_buf()),
            asks: Asks::new(directory.to_path_buf()),
            asked: HashMap::new(),
            turn: Turn::default(),
            last_error: None,
            steering: Vec::new(),
            follow_ups: VecDeque::new(),
            inbox: HashMap::new(),
            quiet: None,
            quiet_generation: 0,
        }
    }

    /// What `event` means for the chat, in order.
    pub fn map(&mut self, event: &Event) -> Vec<Mapped> {
        let Some(session) = event.session_id.as_deref() else {
            return Vec::new();
        };

        if let EventKind::SessionCreated(created) = &event.kind {
            self.note_child(created, event);
            return Vec::new();
        }

        if session != self.session {
            return self.subagent_event(event);
        }

        if let Some(hidden) = self.quiet_event(event) {
            return hidden;
        }

        let mut out = Vec::new();
        let created = event.created;

        match &event.kind {
            EventKind::ExecutionStarted(_) => return self.turn_started(out),
            EventKind::ExecutionSucceeded(_) => return self.execution_ended(Outcome::Succeeded),
            EventKind::ExecutionFailed(failed) => return self.execution_ended(Outcome::Failed(failed.error.clone())),
            EventKind::ExecutionInterrupted(stopped) => {
                return self.execution_ended(Outcome::Interrupted(stopped.reason.clone()));
            }
            EventKind::Status(changed) => return self.status(&changed.status, out),
            EventKind::Idle(_) => return self.status(&SessionStatus::Idle, out),
            EventKind::RetryScheduled(retry) => out.push(self.retry_scheduled(retry)),
            EventKind::InboxEnqueued(enqueued) => self.inbox_enqueued(enqueued, created),
            EventKind::InboxDelivered(delivered) => self.inbox_delivered(&delivered.inbox_id, &mut out),
            EventKind::InboxCancelled(cancelled) => self.inbox_cancelled(&cancelled.inbox_id, &mut out),
            EventKind::StepStarted(step) => {
                self.retry_settled(&mut out);
                self.replies.step_started(step, created);
            }
            EventKind::StepEnded(step) => {
                let context = self.replies.step_ended(step, &self.tools, &mut out);
                return events(out).chain(context.map(Mapped::Context)).collect();
            }
            EventKind::StepFailed(step) => {
                self.note_step_error(&step.error);
                self.replies.step_failed(step, &self.tools, &mut out);
            }
            EventKind::UsageUpdated(usage) => return vec![Mapped::Usage(usage.clone())],
            EventKind::CompactionStarted(started) => out.push(auto_compaction_started(started)),
            EventKind::CompactionEnded(ended) => return auto_compaction_ended(ended),
            EventKind::CompactionFailed(failed) => out.push(auto_compaction_failed(failed)),
            EventKind::Renamed(renamed) => return session_change(SessionChange::Title(renamed.title.clone())),
            EventKind::AgentSelected(selected) => return session_change(SessionChange::Agent(selected.agent.clone())),
            EventKind::ModelSelected(selected) => return session_change(SessionChange::Model(selected.model.clone())),
            kind => {
                if !self.reply_event(kind, created, &mut out) {
                    self.ask_event(kind, false, &mut out);
                }
            }
        }

        events(out).collect()
    }

    /// Whether the chat's server applies an approval mode to every agent (shown on `subagent`
    /// asks).
    pub fn set_mode_applied(&mut self, applied: bool) {
        self.asks.set_mode_applied(applied);
    }

    /// The answer to dialog `response.id`; the engine sends it (or shows the next question).
    pub fn answer(&mut self, response: &UiResponse) -> Result<Answer> {
        self.asks.answer(response)
    }

    /// The directory of subagent session `session`, when the chat knows it; `None` for the
    /// chat's own session or one the chat does not know.
    pub fn subagent_directory(&self, session: &str) -> Option<&str> {
        self.children.get(session).and_then(Option::as_deref)
    }

    /// True for the chat's own session and the subagent sessions started from it.
    pub fn is_chat_session(&self, session: &str) -> bool {
        session == self.session || self.children.contains_key(session)
    }

    /// The permission requests still waiting for an answer on OpenCode's side, shown or not, by id.
    pub fn pending_permissions(&self) -> Vec<PermissionRequest> {
        let mut pending: Vec<PermissionRequest> = self
            .asked
            .values()
            .filter_map(|asked| match asked {
                Asked::Permission(permission, _) => Some(permission.clone()),
                Asked::Form(_) => None,
            })
            .collect();

        pending.sort_by(|a, b| a.id.cmp(&b.id));
        pending
    }

    /// Closes every open dialog (the chat stops).
    pub fn cancel_asks(&mut self) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        self.close_asks(&mut out);
        out
    }

    /// Shows ask `request` again: the answer to its dialog did not reach OpenCode, which still
    /// waits for one. A form starts over at its first field. Nothing when the ask is no longer
    /// open (answered elsewhere, or its turn ended).
    pub fn reopen(&mut self, request: &str) -> Vec<AgentEvent> {
        let Some(asked) = self.asked.get(request).cloned() else {
            return Vec::new();
        };

        let mut out = Vec::new();

        match asked {
            Asked::Permission(permission, subagent) => {
                let input = self.source_input(&permission, subagent);
                self.asks
                    .permission_asked(&permission, subagent, input.as_ref(), &mut out);
            }
            Asked::Form(form) => self.asks.form_created(&form, &mut out),
        }

        out
    }

    /// The session already holds `messages`: later events about them are ignored (a fork's
    /// copies, a resumed session's history).
    pub fn seen(&mut self, messages: &[SessionMessage]) {
        for message in messages {
            match message {
                SessionMessage::User(user) => self.replies.mark_done(&user.id),
                SessionMessage::Assistant(reply) => self.replies.mark_done(&reply.id),
                _ => {}
            }

            for call in message.tool_calls() {
                self.tools.mark_done(&call.id);
            }
        }
    }

    /// True when message `id` (a prompt or a reply) is in the chat: shown, still streaming, or one
    /// the session already had ([`Mapper::seen`]).
    pub fn has_message(&self, id: &str) -> bool {
        self.replies.knows(id)
    }

    /// Closes every open dialog.
    fn close_asks(&mut self, out: &mut Vec<AgentEvent>) {
        self.asks.cancel_all(out);
        self.asked.clear();
    }

    /// Text, reasoning and tool events of the chat's session; false for any other kind.
    fn reply_event(&mut self, kind: &EventKind, created: Option<u64>, out: &mut Vec<AgentEvent>) -> bool {
        match kind {
            EventKind::TextStarted(part) => self.replies.part_started(PartKind::Text, part, created, out),
            EventKind::TextDelta(delta) => self.replies.part_delta(PartKind::Text, delta, created, out),
            EventKind::TextEnded(ended) => self.replies.part_ended(PartKind::Text, ended, created, out),
            EventKind::ReasoningStarted(part) => self.replies.part_started(PartKind::Reasoning, part, created, out),
            EventKind::ReasoningDelta(delta) => self.replies.part_delta(PartKind::Reasoning, delta, created, out),
            EventKind::ReasoningEnded(ended) => self.replies.part_ended(PartKind::Reasoning, ended, created, out),
            EventKind::ToolInputStarted(started) => {
                if self.tools.input_started(started) {
                    let message = &started.assistant_message_id;
                    self.replies.tool_started(message, &started.id, created, out);
                }
            }
            EventKind::ToolInputDelta(delta) => self.tools.input_delta(delta),
            EventKind::ToolInputEnded(ended) => self.tools.input_ended(ended),
            EventKind::ToolCalled(called) => self.tools.called(called, out),
            EventKind::ToolProgress(progress) => self.tools.progress(progress, out),
            EventKind::ToolSuccess(success) => self.tools.success(success, out),
            EventKind::ToolFailed(failed) => self.tools.failed(failed, out),
            _ => return false,
        }

        true
    }

    /// An event of another session: only the asks of the chat's subagents reach the chat.
    fn subagent_event(&mut self, event: &Event) -> Vec<Mapped> {
        let known = event
            .session_id
            .as_deref()
            .is_some_and(|session| self.children.contains_key(session));

        if !known {
            return Vec::new();
        }

        let mut out = Vec::new();
        self.ask_event(&event.kind, true, &mut out);

        events(out).collect()
    }

    /// Permission and form events.
    fn ask_event(&mut self, kind: &EventKind, subagent: bool, out: &mut Vec<AgentEvent>) {
        match kind {
            EventKind::PermissionAsked(permission) => {
                let asked = Asked::Permission(permission.clone(), subagent);
                self.asked.insert(permission.id.clone(), asked);

                let input = self.source_input(permission, subagent);
                self.asks.permission_asked(permission, subagent, input.as_ref(), out);
            }
            EventKind::FormCreated(created) => {
                self.asked
                    .insert(created.form.id.clone(), Asked::Form(created.form.clone()));
                self.asks.form_created(&created.form, out);
            }
            EventKind::PermissionReplied(replied) => {
                self.asked.remove(&replied.request_id);
                self.asks.answered_elsewhere(&replied.request_id, out);
            }
            EventKind::FormReplied(settled) | EventKind::FormCancelled(settled) => {
                self.asked.remove(&settled.id);
                self.asks.answered_elsewhere(&settled.id, out);
            }
            _ => {}
        }
    }

    /// The input of the chat's own tool call a permission request comes from.
    fn source_input(&self, permission: &PermissionRequest, subagent: bool) -> Option<Value> {
        let source = permission.source.as_ref().filter(|_| !subagent)?;

        self.tools.input(&source.id).cloned()
    }

    /// Tracks a session started from a tracked one.
    fn note_child(&mut self, created: &SessionCreated, event: &Event) {
        let parent = created.parent_id.as_deref();
        let known = parent.is_some_and(|parent| parent == self.session || self.children.contains_key(parent));

        if known {
            let directory = event.location.as_ref().map(|location| location.directory.clone());
            self.children.insert(created.session_id.clone(), directory);
        }
    }
}

/// The chat messages of `messages` (a session's messages, oldest first) as the chat shows them on
/// resume: the conversion the CLI transcript uses.
pub fn history(messages: &[SessionMessage]) -> Vec<ChatMessage> {
    messages.iter().flat_map(SessionMessage::chat_messages).collect()
}

fn session_change(change: SessionChange) -> Vec<Mapped> {
    vec![Mapped::Session(change)]
}

/// A compaction OpenCode started on its own during a turn.
fn auto_compaction_started(started: &CompactionStarted) -> AgentEvent {
    compaction_event("start", &started.reason, None)
}

fn auto_compaction_ended(ended: &CompactionEnded) -> Vec<Mapped> {
    vec![
        Mapped::event(compaction_event("end", &ended.reason, None)),
        Mapped::Context(ContextNote::Compacted),
    ]
}

fn auto_compaction_failed(failed: &CompactionFailed) -> AgentEvent {
    let error = error_message(&failed.error.message);

    compaction_event("end", &failed.reason, Some(error))
}

fn compaction_event(phase: &str, reason: &str, error: Option<String>) -> AgentEvent {
    AgentEvent::Compaction {
        phase: phase.to_owned(),
        reason: (!reason.is_empty()).then(|| reason.to_owned()),
        error,
    }
}

/// The first line of a redacted error message (later lines are a stack trace), or
/// [`UNNAMED_ERROR`].
fn error_message(message: &str) -> String {
    let line = first_line(&redact_text(message));

    if line.is_empty() {
        return UNNAMED_ERROR.to_owned();
    }

    line
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().trim().to_owned()
}

fn events(events: Vec<AgentEvent>) -> impl Iterator<Item = Mapped> {
    events.into_iter().map(Mapped::event)
}
