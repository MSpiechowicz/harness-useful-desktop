//! The closed allowlist of `opencode serve` routes the app calls.
//!
//! - Every request names a [`Route`]; the client never builds a path from free text.
//! - Ids are checked when they are parsed ([`Id::parse`]): the kind's prefix (`ses_`, `msg_`,
//!   `per_`, `que_`), then only `[A-Za-z0-9_]`, at most [`MAX_ID_LEN`] bytes. A valid id can never
//!   add a path segment or a query.
//! - `directory` and `before` values are percent-encoded; only RFC 3986 unreserved bytes stay as
//!   they are.
//! - Left out on purpose (plan "Standing rules"): share, `/global/dispose`, `/global/upgrade`,
//!   config writes, `/auth/*`, OAuth, `/tui/*`, `/pty/*` and `/experimental/*`.
//!
//! `/session/{id}/…` routes carry no `directory`: OpenCode routes them by the session's stored
//! directory. `/permission/*` and `/question/*` are not session-scoped, so their replies carry the
//! session's directory.

use std::fmt;
use std::marker::PhantomData;

use super::http::Method;
use crate::error::BusinessError;

/// Longest accepted id, prefix included (OpenCode's are about 30 bytes).
pub const MAX_ID_LEN: usize = 128;

/// What an [`Id`] names: its prefix and a label for errors.
pub trait IdKind {
    /// The prefix every id of this kind starts with.
    const PREFIX: &'static str;

    /// What the id names, for error messages (the value itself is never shown).
    const LABEL: &'static str;
}

/// Marker of session ids (`ses_…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionKind {}

/// Marker of message ids (`msg_…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageKind {}

/// Marker of permission request ids (`per_…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionKind {}

/// Marker of question ids (`que_…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestionKind {}

impl IdKind for SessionKind {
    const PREFIX: &'static str = "ses_";
    const LABEL: &'static str = "session";
}

impl IdKind for MessageKind {
    const PREFIX: &'static str = "msg_";
    const LABEL: &'static str = "message";
}

impl IdKind for PermissionKind {
    const PREFIX: &'static str = "per_";
    const LABEL: &'static str = "permission";
}

impl IdKind for QuestionKind {
    const PREFIX: &'static str = "que_";
    const LABEL: &'static str = "question";
}

/// An OpenCode session id.
pub type SessionId = Id<SessionKind>;

/// An OpenCode message id.
pub type MessageId = Id<MessageKind>;

/// An OpenCode permission request id.
pub type PermissionId = Id<PermissionKind>;

/// An OpenCode question id.
pub type QuestionId = Id<QuestionKind>;

/// A checked OpenCode id of kind `K` (see the module docs).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id<K> {
    value: String,
    kind: PhantomData<K>,
}

impl<K: IdKind> Id<K> {
    /// Checks `value`: the kind's prefix, a non-empty rest of `[A-Za-z0-9_]`, at most
    /// [`MAX_ID_LEN`] bytes.
    pub fn parse(value: &str) -> Result<Self, RouteError> {
        let rest = value.strip_prefix(K::PREFIX).unwrap_or_default();

        let valid = !rest.is_empty()
            && value.len() <= MAX_ID_LEN
            && rest.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');

        if !valid {
            return Err(RouteError::InvalidId { kind: K::LABEL });
        }

        Ok(Self {
            value: value.to_owned(),
            kind: PhantomData,
        })
    }

    /// The id as OpenCode writes it.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl<K> fmt::Display for Id<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.value)
    }
}

/// A value that cannot be used in a route. Never carries the value.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RouteError {
    /// The id has the wrong prefix, a byte outside `[A-Za-z0-9_]` or is too long.
    #[error("invalid OpenCode {kind} id")]
    InvalidId {
        /// What the id names (`session`, `message`, `permission`, `question`).
        kind: &'static str,
    },
}

impl From<RouteError> for BusinessError {
    fn from(error: RouteError) -> Self {
        Self::InvalidInput(error.to_string())
    }
}

/// One allowed `opencode serve` call (see the module docs).
///
/// POST routes whose body is optional on the server (`SessionCreate`, `Fork`, `Abort`) are sent with
/// `{}` by the engine, as recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route<'a> {
    /// `GET /global/health`.
    Health,
    /// `GET /global/event`, the SSE stream (read with `OpenCodeClient::events`).
    GlobalEvent,
    /// `POST /session?directory=`.
    SessionCreate {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /session/{id}`.
    SessionGet {
        /// The session.
        session: &'a SessionId,
    },
    /// `PATCH /session/{id}` (`{title?, permission?}`).
    SessionPatch {
        /// The session.
        session: &'a SessionId,
    },
    /// `DELETE /session/{id}`.
    SessionDelete {
        /// The session.
        session: &'a SessionId,
    },
    /// `POST /session/{id}/prompt_async` (answers `204`).
    PromptAsync {
        /// The session.
        session: &'a SessionId,
    },
    /// `POST /session/{id}/command` (blocks until the turn ends).
    Command {
        /// The session.
        session: &'a SessionId,
    },
    /// `POST /session/{id}/shell` (blocks until the command ends).
    Shell {
        /// The session.
        session: &'a SessionId,
    },
    /// `POST /session/{id}/fork` (`{messageID?}`).
    Fork {
        /// The session.
        session: &'a SessionId,
    },
    /// `POST /session/{id}/abort`.
    Abort {
        /// The session.
        session: &'a SessionId,
    },
    /// `POST /session/{id}/summarize` (blocks until the summary is written).
    Summarize {
        /// The session.
        session: &'a SessionId,
    },
    /// `GET /session/{id}/message?limit=&before=`; the next page's cursor is the `x-next-cursor`
    /// header.
    Messages {
        /// The session.
        session: &'a SessionId,
        /// Messages per page; `None` returns all of them.
        limit: Option<u32>,
        /// The cursor of the page to read (a previous `x-next-cursor`).
        before: Option<&'a str>,
    },
    /// `GET /session/status?directory=`.
    SessionStatus {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /config/providers?directory=` → `{providers, default}`.
    ConfigProviders {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /agent?directory=`.
    Agents {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /command?directory=`.
    Commands {
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /permission/{id}/reply?directory=` (`{reply: once|always|reject, message?}`).
    PermissionReply {
        /// The permission request.
        id: &'a PermissionId,
        /// The session's directory.
        directory: &'a str,
    },
    /// `POST /question/{id}/reply?directory=` (`{answers: [[label…]]}`).
    QuestionReply {
        /// The question.
        id: &'a QuestionId,
        /// The session's directory.
        directory: &'a str,
    },
    /// `POST /question/{id}/reject?directory=`.
    QuestionReject {
        /// The question.
        id: &'a QuestionId,
        /// The session's directory.
        directory: &'a str,
    },
}

impl Route<'_> {
    /// The request method.
    pub fn method(&self) -> Method {
        match self {
            Self::Health
            | Self::GlobalEvent
            | Self::SessionGet { .. }
            | Self::Messages { .. }
            | Self::SessionStatus { .. }
            | Self::ConfigProviders { .. }
            | Self::Agents { .. }
            | Self::Commands { .. } => Method::Get,
            Self::SessionPatch { .. } => Method::Patch,
            Self::SessionDelete { .. } => Method::Delete,
            _ => Method::Post,
        }
    }

    /// True for the calls that answer only when their work is done (`command`, `shell`,
    /// `summarize`); they get the client's long timeout.
    pub fn blocks(&self) -> bool {
        matches!(self, Self::Command { .. } | Self::Shell { .. } | Self::Summarize { .. })
    }

    /// The request target: path and query, with every value encoded.
    pub fn target(&self) -> String {
        match *self {
            Self::Health => "/global/health".to_owned(),
            Self::GlobalEvent => "/global/event".to_owned(),
            Self::SessionCreate { directory } => in_directory("/session", directory),
            Self::SessionGet { session } | Self::SessionPatch { session } | Self::SessionDelete { session } => {
                format!("/session/{session}")
            }
            Self::PromptAsync { session } => format!("/session/{session}/prompt_async"),
            Self::Command { session } => format!("/session/{session}/command"),
            Self::Shell { session } => format!("/session/{session}/shell"),
            Self::Fork { session } => format!("/session/{session}/fork"),
            Self::Abort { session } => format!("/session/{session}/abort"),
            Self::Summarize { session } => format!("/session/{session}/summarize"),
            Self::Messages { session, limit, before } => messages(session, limit, before),
            Self::SessionStatus { directory } => in_directory("/session/status", directory),
            Self::ConfigProviders { directory } => in_directory("/config/providers", directory),
            Self::Agents { directory } => in_directory("/agent", directory),
            Self::Commands { directory } => in_directory("/command", directory),
            Self::PermissionReply { id, directory } => in_directory(&format!("/permission/{id}/reply"), directory),
            Self::QuestionReply { id, directory } => in_directory(&format!("/question/{id}/reply"), directory),
            Self::QuestionReject { id, directory } => in_directory(&format!("/question/{id}/reject"), directory),
        }
    }
}

fn in_directory(path: &str, directory: &str) -> String {
    format!("{path}?directory={}", percent_encode(directory))
}

fn messages(session: &SessionId, limit: Option<u32>, before: Option<&str>) -> String {
    let mut query = Vec::new();

    if let Some(limit) = limit {
        query.push(format!("limit={limit}"));
    }

    if let Some(before) = before {
        query.push(format!("before={}", percent_encode(before)));
    }

    let path = format!("/session/{session}/message");

    if query.is_empty() {
        return path;
    }

    format!("{path}?{}", query.join("&"))
}

/// Percent-encodes every byte of `value` except RFC 3986 unreserved ones (`A-Z a-z 0-9 - . _ ~`).
pub fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }

    encoded
}
