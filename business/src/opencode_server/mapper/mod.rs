//! `opencode serve` events as the chat's [`AgentEvent`]s, and the OpenCode message conversion the
//! CLI transcript shares. Pure: no I/O, one [`Mapper`] per chat.
//!
//! - **Sessions:** only events of the chat's own session count. Subagent sessions started from it
//!   (`session.created` with a known `parentID`, at any depth) are tracked for their asks; their
//!   messages and tool calls belong to the `task` call that started them and are not shown.
//!   Durable events repeat as `sync` and server events (`server.*`) carry no session: both are
//!   ignored, as is every type the chat does not show (catalog, plugin, file and diff events).
//! - **Messages and tool calls:** see [`messages`].
//! - **Turns, steering and the follow-up queue:** see [`turns`].
//! - **Session info:** `session.updated` of the chat's session is passed on as
//!   [`Mapped::Session`].

mod message;
mod messages;
mod touched;
mod turns;

use std::collections::{HashSet, VecDeque};
use std::path::Path;

use serde_json::Value;

pub(crate) use self::message::{part_type, record_messages};
use self::messages::Messages;
pub(crate) use self::touched::{part_touched, project_dir};
use self::turns::Turn;
use super::wire::{Envelope, Outgoing, SessionInfo, session_info};
use crate::model::AgentEvent;
use crate::security::redact_text;

/// What one event (or one change the engine reports) means for the chat.
#[derive(Debug, Clone, PartialEq)]
pub enum Mapped {
    /// An event for the chat.
    Event(AgentEvent),
    /// The agent took the prompt on its way: run the prompt-taken call now, before what follows.
    PromptTaken,
    /// The chat's session changed (title, agent, model).
    Session(SessionInfo),
    /// Send this queued follow-up now (it already counts as a prompt on its way).
    Send(Outgoing),
    /// The turn went idle with these steering messages unanswered: check whether OpenCode left the
    /// newest one without a reply, then answer with [`Mapper::resend_stranded`] or [`Mapper::stranded_checked`].
    CheckStranded(Vec<Outgoing>),
}

/// Maps the events of one chat (see the module docs).
pub struct Mapper {
    session: String,
    children: HashSet<String>,
    messages: Messages,
    turn: Turn,
    /// The error the last turn ended with, until the next turn starts.
    last_error: Option<String>,
    /// Steering sent into the running turn, until a reply to it appears.
    steering: Vec<Outgoing>,
    /// Follow-ups held until the chat is idle.
    follow_ups: VecDeque<Outgoing>,
    /// Steering a stranded check already covered (each is sent again at most once).
    checked: HashSet<String>,
    /// The turn's end waits for the answer to [`Mapped::CheckStranded`].
    checking: bool,
    /// Steering sent and not yet taken by OpenCode.
    undelivered: HashSet<String>,
    /// [`Mapped::CheckStranded`] is due once [`Mapper::undelivered`] is empty.
    check_due: bool,
}

impl Mapper {
    /// A mapper for session `session` (`ses_…`) working in `directory` within `project` (both
    /// canonical).
    pub fn new(session: &str, project: &Path, directory: &Path) -> Self {
        Self {
            session: session.to_owned(),
            children: HashSet::new(),
            messages: Messages::new(project.to_path_buf(), directory.to_path_buf()),
            turn: Turn::default(),
            last_error: None,
            steering: Vec::new(),
            follow_ups: VecDeque::new(),
            checked: HashSet::new(),
            checking: false,
            undelivered: HashSet::new(),
            check_due: false,
        }
    }

    /// What `envelope` means for the chat, in order.
    pub fn map(&mut self, envelope: &Envelope) -> Vec<Mapped> {
        let Some(session) = envelope.session_id() else {
            return Vec::new();
        };

        if envelope.kind == "session.created" {
            self.note_child(&envelope.properties);
            return Vec::new();
        }

        if session != self.session {
            return Vec::new();
        }

        let mut out = Vec::new();
        let properties = &envelope.properties;
        self.end_pending_user(envelope, &mut out);

        match envelope.kind.as_str() {
            "message.updated" => {
                let info = properties.get("info").unwrap_or(&Value::Null);
                self.note_message_error(info);
                self.note_reply(info, &mut out);
                self.messages.message_updated(info, &mut out);
            }
            "message.part.updated" => {
                let part = properties.get("part").unwrap_or(&Value::Null);
                self.messages.part_updated(part, &mut out);
            }
            "message.part.delta" => self.messages.part_delta(properties, &mut out),
            "session.status" => return self.status(properties, out),
            "session.error" => self.error(properties, &mut out),
            "session.updated" => {
                let info = properties.get("info").and_then(session_info);
                return events(out).chain(info.map(Mapped::Session)).collect();
            }
            _ => {}
        }

        events(out).collect()
    }

    /// Tracks a session started from a tracked one.
    fn note_child(&mut self, properties: &Value) {
        let Some(info) = properties.get("info").and_then(session_info) else {
            return;
        };

        let parent = info.parent_id.as_deref();
        let known = parent.is_some_and(|parent| parent == self.session || self.children.contains(parent));

        if known {
            self.children.insert(info.id);
        }
    }

    /// Ends the open user message unless `envelope` is about it (its parts come right after it).
    fn end_pending_user(&mut self, envelope: &Envelope, out: &mut Vec<AgentEvent>) {
        let properties = &envelope.properties;
        let message = properties
            .pointer("/part/messageID")
            .or_else(|| properties.get("messageID"))
            .or_else(|| properties.pointer("/info/id"))
            .and_then(Value::as_str);

        if let Some(id) = self.messages.pending_user(message) {
            self.messages.end_user(&id, out);
        }
    }
}

/// The first line of an error's redacted `data.message` (later lines are a stack trace), or its
/// name.
fn error_message(error: &Value) -> String {
    let message = error.pointer("/data/message").and_then(Value::as_str);
    let name = error
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("OpenCode reported an error");

    first_line(&redact_text(message.unwrap_or(name)))
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().trim().to_owned()
}

fn events(events: Vec<AgentEvent>) -> impl Iterator<Item = Mapped> {
    events.into_iter().map(Mapped::Event)
}
