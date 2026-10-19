//! `opencode serve` events as the chat's [`AgentEvent`]s, and the OpenCode message conversion the
//! CLI transcript shares. Pure: no I/O, one [`Mapper`] per chat.
//!
//! - **Sessions:** only events of the chat's session and of the subagent sessions started from it
//!   (`session.created` with a known `parentID`, at any depth) count. Durable events repeat as
//!   `sync` and server events (`server.*`) carry no session: both are ignored, as is every type the
//!   chat does not show (catalog, plugin, file and diff events).
//! - **Messages and tool calls:** see [`messages`]. A subagent session reports only its tool
//!   calls: its messages belong to the `task` call that started it.
//! - **Turns:** the chat's session going `busy` starts a turn (`agentStart`, preceded by
//!   [`Mapped::PromptTaken`] while a prompt is on its way), going `idle` ends it: what is still
//!   open ends, then `agentEnd`, a `promptResult` for each prompt of the turn (the first
//!   `session.error`, or an assistant message's `error`; an abort makes it `aborted`) and
//!   `settled`. A `session.error` outside a turn becomes an error notice, unless it repeats the
//!   error the last turn ended with (OpenCode reports some errors twice) or reports an abort.
//! - **Session info:** `session.updated` of the chat's session is passed on as
//!   [`Mapped::Session`].

mod message;
mod messages;
mod touched;

use std::collections::HashSet;
use std::path::Path;

use serde_json::Value;

pub(crate) use self::message::{assistant_message, part_type, user_message};
use self::message::{error_text, is_abort};
use self::messages::Messages;
pub(crate) use self::touched::{part_touched, project_dir};
use super::wire::{Envelope, SessionInfo, session_info};
use crate::model::{AgentEvent, NoticeLevel, PromptStatus};
use crate::security::redact_text;

/// What one event means for the chat.
#[derive(Debug, Clone, PartialEq)]
pub enum Mapped {
    /// An event for the chat.
    Event(AgentEvent),
    /// The agent took the prompt on its way: run the prompt-taken call now, before what follows.
    PromptTaken,
    /// The chat's session changed (title, agent, model).
    Session(SessionInfo),
}

/// Maps the events of one chat (see the module docs).
pub struct Mapper {
    session: String,
    children: HashSet<String>,
    messages: Messages,
    turn: Turn,
    /// The error the last turn ended with, until the next turn starts.
    last_error: Option<String>,
}

/// The running turn.
#[derive(Default)]
struct Turn {
    /// `agentStart` was emitted.
    active: bool,
    /// Prompts sent and not yet answered by a `promptResult`.
    prompts: u32,
    error: Option<TurnError>,
    aborted: bool,
}

struct TurnError {
    message: String,
    retryable: bool,
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
        }
    }

    /// True while a turn runs or a prompt is on its way.
    pub fn is_working(&self) -> bool {
        self.turn.active || self.turn.prompts > 0
    }

    /// A prompt is about to be sent.
    pub fn prompt_sent(&mut self) {
        self.turn.prompts += 1;
    }

    /// The prompt announced by [`Mapper::prompt_sent`] was not delivered.
    pub fn prompt_failed(&mut self) {
        self.turn.prompts = self.turn.prompts.saturating_sub(1);
    }

    /// What `envelope` means for the chat, in order.
    pub fn map(&mut self, envelope: &Envelope) -> Vec<Mapped> {
        let Some(session) = envelope.session_id().map(str::to_owned) else {
            return Vec::new();
        };

        if envelope.kind == "session.created" {
            self.note_child(&envelope.properties);
            return Vec::new();
        }

        let own = session == self.session;
        if !own && !self.children.contains(&session) {
            return Vec::new();
        }

        let mut out = Vec::new();
        let properties = &envelope.properties;

        if own {
            self.end_pending_user(envelope, &mut out);
        }

        match envelope.kind.as_str() {
            "message.updated" if own => {
                let info = properties.get("info").unwrap_or(&Value::Null);
                self.note_message_error(info);
                self.messages.message_updated(info, &mut out);
            }
            "message.part.updated" => {
                let part = properties.get("part").unwrap_or(&Value::Null);
                self.messages.part_updated(part, own, &mut out);
            }
            "message.part.delta" if own => self.messages.part_delta(properties, &mut out),
            "session.status" if own => return self.status(properties, out),
            "session.error" if own => self.error(properties, &mut out),
            "session.updated" if own => {
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

    fn status(&mut self, properties: &Value, mut out: Vec<AgentEvent>) -> Vec<Mapped> {
        let status = properties.pointer("/status/type").and_then(Value::as_str);
        let mut mapped: Vec<Mapped> = Vec::new();

        match status {
            Some("busy") if !self.turn.active => {
                self.turn.active = true;
                self.last_error = None;

                mapped.extend(events(std::mem::take(&mut out)));
                if self.turn.prompts > 0 {
                    mapped.push(Mapped::PromptTaken);
                }

                out.push(AgentEvent::AgentStart);
            }
            Some("idle") if self.turn.active => self.end_turn(&mut out),
            _ => {}
        }

        mapped.extend(events(out));
        mapped
    }

    fn end_turn(&mut self, out: &mut Vec<AgentEvent>) {
        let turn = std::mem::take(&mut self.turn);
        self.messages.end_all(turn.aborted, out);
        out.push(AgentEvent::AgentEnd);

        let (status, error, retryable) = match (&turn.error, turn.aborted) {
            (_, true) => (PromptStatus::Aborted, None, false),
            (Some(error), false) => (PromptStatus::Error, Some(error.message.clone()), error.retryable),
            (None, false) => (PromptStatus::Completed, None, false),
        };

        for _ in 0..turn.prompts {
            out.push(AgentEvent::PromptResult {
                request_id: None,
                status,
                error: error.clone(),
                retryable,
            });
        }

        out.push(AgentEvent::Settled {
            aborted: Some(turn.aborted),
        });

        self.last_error = turn.error.map(|error| error.message);
    }

    fn error(&mut self, properties: &Value, out: &mut Vec<AgentEvent>) {
        let error = properties.get("error").unwrap_or(&Value::Null);

        if is_abort(error) {
            self.turn.aborted |= self.turn.active;
            return;
        }

        let message = error_message(error);

        if self.turn.active {
            self.note_turn_error(message, error);
            return;
        }

        let repeated = self.last_error.as_deref().is_some_and(|last| message.contains(last));

        if repeated {
            self.last_error = None;
            return;
        }

        out.push(AgentEvent::Notice {
            level: NoticeLevel::Error,
            message,
        });
    }

    /// A failed assistant message fails its turn.
    fn note_message_error(&mut self, info: &Value) {
        let Some(error) = info.get("error").filter(|_| self.turn.active) else {
            return;
        };

        if is_abort(error) {
            self.turn.aborted = true;
        } else if let Some(message) = error_text(info) {
            self.note_turn_error(first_line(&message), error);
        }
    }

    /// Keeps the first error of the running turn.
    fn note_turn_error(&mut self, message: String, error: &Value) {
        if self.turn.error.is_some() {
            return;
        }

        let retryable = error
            .pointer("/data/isRetryable")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        self.turn.error = Some(TurnError { message, retryable });
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
