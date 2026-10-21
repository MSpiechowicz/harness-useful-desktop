//! The subagent sessions of a chat: the sessions started from its own session, at any depth, whose
//! asks reach the chat ([`super::asks`]). Their messages and tool calls are not shown.
//!
//! A subagent session is known from any of:
//!
//! - `session.created` with a known `parentID`;
//! - `session.tool.progress` of a known session whose metadata names the session its subagent
//!   call runs in (`{sessionID, status}`), so a child whose start the chat missed (the stream was
//!   opened again meanwhile) still counts;
//! - [`Mapper::adopt`]: the engine lists the children of the chat's sessions when it catches up
//!   or resumes.

use serde_json::Value;

use super::{Mapped, Mapper, events};
use crate::opencode_server_v2::events::{Event, EventKind, SessionCreated, ToolProgress};

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

    /// Tracks a session started from a tracked one.
    pub(super) fn note_child(&mut self, created: &SessionCreated, event: &Event) {
        let Some(parent) = created.parent_id.as_deref() else {
            return;
        };

        self.adopt(&created.session_id, parent, event_directory(event));
    }

    /// Tracks the session a subagent call of a tracked session runs in.
    pub(super) fn note_progress(&mut self, progress: &ToolProgress, event: &Event) {
        let child = progress.metadata.get("sessionID").and_then(Value::as_str);

        if let Some(child) = child {
            self.adopt(child, &progress.session_id, event_directory(event));
        }
    }

    /// An event of another session: only the asks of the chat's subagents reach the chat.
    pub(super) fn subagent_event(&mut self, event: &Event) -> Vec<Mapped> {
        let known = event
            .session_id
            .as_deref()
            .is_some_and(|session| self.children.contains_key(session));

        if !known {
            return Vec::new();
        }

        if let EventKind::ToolProgress(progress) = &event.kind {
            self.note_progress(progress, event);
            return Vec::new();
        }

        let mut out = Vec::new();
        self.ask_event(&event.kind, true, &mut out);

        events(out).collect()
    }
}

/// The directory an event is about, when it says.
fn event_directory(event: &Event) -> Option<String> {
    event.location.as_ref().map(|location| location.directory.clone())
}
