//! The subagent sessions of a chat: the sessions started from its own session, at any depth, whose
//! asks reach the chat ([`super::asks`]) and whose work shows in the subagent panel
//! ([`super::children`]). Their messages and tool calls are not part of the chat.
//!
//! A subagent session is known from any of:
//!
//! - `session.created` with a known `parentID`;
//! - [`Mapper::adopt`]: the engine lists the children of the chat's sessions when it catches up
//!   or resumes, and when a `subagent` call of the chat's own session names a session the chat
//!   does not count ([`Mapper::take_unlisted`]): its progress metadata names the session the call
//!   runs in (`{sessionID, status}`), so a child whose start the chat missed (the stream was
//!   opened again meanwhile) still counts. The name alone counts for nothing: any tool can put
//!   any session there, and only OpenCode's list of the children confirms one.

use serde_json::Value;

use super::{Mapped, Mapper, events};
use crate::model::AgentEvent;
use crate::opencode_server_v2::events::{Event, SessionCreated, ToolProgress};

/// The tool that runs a subagent in a session of its own.
pub(super) const SUBAGENT_TOOL: &str = "subagent";

impl Mapper {
    /// Counts `session` (started from `parent`) as one of the chat's subagent sessions, working
    /// in `directory`; nothing when `parent` is not one of the chat's sessions.
    pub fn adopt(&mut self, session: &str, parent: &str, directory: Option<String>) {
        if session.is_empty() || session == self.session || !self.is_chat_session(parent) {
            return;
        }

        let known = self.children.entry(session.to_owned()).or_default();

        if known.is_none() {
            *known = directory;
        }
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

    /// The subagent sessions the chat knows.
    pub fn subagent_sessions(&self) -> Vec<String> {
        let mut sessions: Vec<String> = self.children.keys().cloned().collect();
        sessions.sort();
        sessions
    }

    /// The sessions `subagent` calls of the chat's session named that the chat does not count,
    /// each handed out once: the engine lists the chat's subagent sessions again to confirm them
    /// (see the module docs).
    pub fn take_unlisted(&mut self) -> Vec<String> {
        std::mem::take(&mut self.unlisted)
    }

    /// Tracks a session started from a tracked one.
    pub(super) fn note_child(&mut self, created: &SessionCreated, event: &Event, out: &mut Vec<AgentEvent>) {
        let Some(parent) = created.parent_id.as_deref() else {
            return;
        };

        self.adopt(&created.session_id, parent, event_directory(event));

        if self.children.contains_key(&created.session_id) {
            self.child_started(&created.session_id, parent, out);
        }
    }

    /// Notes the session a `subagent` call of the chat's session names, for the engine to confirm.
    pub(super) fn note_progress(&mut self, progress: &ToolProgress) {
        if self.tools.name(&progress.id) != Some(SUBAGENT_TOOL) {
            return;
        }

        let Some(child) = progress.metadata.get("sessionID").and_then(Value::as_str) else {
            return;
        };

        if child.is_empty() || self.is_chat_session(child) || !self.named.insert(child.to_owned()) {
            return;
        }

        self.unlisted.push(child.to_owned());
    }

    /// An event of another session: the asks of the chat's subagents reach the chat, the rest
    /// the subagent panel ([`super::children`]).
    pub(super) fn subagent_event(&mut self, event: &Event) -> Vec<Mapped> {
        let Some(session) = event
            .session_id
            .as_deref()
            .filter(|session| self.children.contains_key(*session))
        else {
            return Vec::new();
        };

        let mut out = Vec::new();
        self.child_event(session, event, &mut out);
        self.ask_event(&event.kind, true, &mut out);

        events(out).collect()
    }
}

/// The directory an event is about, when it says.
fn event_directory(event: &Event) -> Option<String> {
    event.location.as_ref().map(|location| location.directory.clone())
}
