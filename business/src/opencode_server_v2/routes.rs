//! The closed allowlist of OpenCode 2 server routes (`/api/...`) and the checked ids they take.
//!
//! - Every request names a [`RouteV2`]; the client never builds a path from free text.
//! - Ids are checked when they are parsed ([`Id::parse`]): the kind's prefix (`ses_`, `msg_`,
//!   `per_`, `frm_`), then only `[A-Za-z0-9]`, at most [`MAX_ID_LEN`] bytes. A valid id can never
//!   add a path segment or a query.
//! - Every route but [`RouteV2::Info`], [`RouteV2::Event`], [`RouteV2::PermissionSaved`] (a
//!   project query) and [`RouteV2::SessionChildren`] (a parent query) carries the project directory as the `location[directory]` query,
//!   percent-encoded and last, as recorded against `opencode` 2.1.2. Callers pass the absolute
//!   project directory.
//! - Left out on purpose (standing rules): share, pairing, credentials, config writes
//!   (`/api/experimental/config`), plugin and MCP management, PTYs, session archive, move, revert,
//!   environment, import and saved permission changes. `PATCH /api/session/{id}` is only ever sent
//!   with a title ([`super::wire::session::TitleBody`]), never with permissions.

use std::fmt;
use std::marker::PhantomData;

use serde::{Serialize, Serializer};

use crate::opencode_server::http::sealed::Sealed;
use crate::opencode_server::http::{ApiRoute, Method};
use crate::opencode_server::routes::{RouteError, percent_encode};

/// Longest accepted id, prefix included (OpenCode 2's are about 30 bytes).
pub const MAX_ID_LEN: usize = 128;

/// Largest message page the server serves (`SessionMessagesQuery.limit` is 1 to 200).
pub const MAX_MESSAGE_PAGE: u32 = 200;

/// Largest page of sessions the chat asks for (`SessionsQuery.limit` has no upper bound).
pub const MAX_SESSION_PAGE: u32 = 200;

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

/// Marker of message ids (`msg_…`, also inbox items).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageKind {}

/// Marker of permission request ids (`per_…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionKind {}

/// Marker of form ids (`frm_…`, the questions a tool asks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormKind {}

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

impl IdKind for FormKind {
    const PREFIX: &'static str = "frm_";
    const LABEL: &'static str = "form";
}

/// An OpenCode 2 session id.
pub type SessionId = Id<SessionKind>;

/// An OpenCode 2 message id (also the id of an inbox item).
pub type MessageId = Id<MessageKind>;

/// An OpenCode 2 permission request id.
pub type PermissionId = Id<PermissionKind>;

/// An OpenCode 2 form id.
pub type FormId = Id<FormKind>;

/// A checked OpenCode 2 id of kind `K` (see the module docs). Serializes as the plain string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id<K> {
    value: String,
    kind: PhantomData<K>,
}

impl<K: IdKind> Id<K> {
    /// Checks `value`: the kind's prefix, a non-empty rest of `[A-Za-z0-9]`, at most
    /// [`MAX_ID_LEN`] bytes.
    pub fn parse(value: &str) -> Result<Self, RouteError> {
        let rest = value.strip_prefix(K::PREFIX).unwrap_or_default();

        let valid =
            !rest.is_empty() && value.len() <= MAX_ID_LEN && rest.bytes().all(|byte| byte.is_ascii_alphanumeric());

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

impl<K> Serialize for Id<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.value)
    }
}

/// The order of the first message page (`order=asc|desc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageOrder {
    /// Oldest first.
    Ascending,
    /// Newest first.
    Descending,
}

impl MessageOrder {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ascending => "asc",
            Self::Descending => "desc",
        }
    }
}

/// One allowed OpenCode 2 server call (see the module docs). `directory` is the absolute project
/// directory, sent as `location[directory]`.
///
/// Bodies: [`super::wire::session`] (create, title, fork, agent, model) and
/// [`super::wire::prompt`] (prompt, command, shell, compact, replies).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteV2<'a> {
    /// `GET /api/info`: readiness (`503` while starting) and the server version (not enveloped).
    Info,
    /// `GET /api/event`, the SSE stream (read with `OpenCodeClient::events`).
    Event,
    /// `POST /api/session` → `{data: session}`.
    SessionCreate {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/session?limit=&parentID=` → `{data: [session], cursor}`, newest change first.
    SessionChildren {
        /// The parent session.
        parent: &'a SessionId,
        /// Sessions per page, clamped to 1..=[`MAX_SESSION_PAGE`].
        limit: u32,
    },
    /// `GET /api/session/{id}` → `{data: session}`.
    SessionGet {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `PATCH /api/session/{id}` with a title only; `204`.
    SessionUpdate {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `DELETE /api/session/{id}`; `204`.
    SessionDelete {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /api/session/{id}/fork` (`{before?}`) → `{data: session}`.
    SessionFork {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /api/session/{id}/agent` (`{agent}`); `204`.
    SessionAgent {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /api/session/{id}/model` (`{model: {id, providerID, variant?}}`); `204`.
    SessionModel {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /api/session/{id}/prompt` → `{data: inbox item}`, once the prompt is admitted.
    Prompt {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /api/session/{id}/interrupt` → `{interrupted}` (not enveloped).
    Interrupt {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /api/session/{id}/command` (`{name, text}`); `204` once the command is admitted.
    Command {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /api/session/{id}/shell` (`{command}`); `204` once the command has ended.
    Shell {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /api/session/{id}/compact` (`{}`) → `{data: inbox item}`, once admitted.
    Compact {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/session/{id}/message?limit=&order=&cursor=` → `{data, cursor}`.
    Messages {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
        /// Messages per page, clamped to 1..=[`MAX_MESSAGE_PAGE`].
        limit: u32,
        /// The page to read: a previous `cursor.next` or `cursor.previous`. The server refuses a
        /// cursor together with `order`.
        cursor: Option<&'a str>,
        /// The order of the first page.
        order: Option<MessageOrder>,
    },
    /// `GET /api/session/{id}/permission` → `{data: [request]}`, the pending requests.
    PermissionList {
        /// The session.
        session: &'a SessionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/permission/request` → `{data: [request], location}`, the directory's pending ones.
    PermissionRequests {
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /api/session/{id}/permission/{requestID}/reply` (`{decision, message?}`); `204`.
    PermissionReply {
        /// The session.
        session: &'a SessionId,
        /// The permission request.
        permission: &'a PermissionId,
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/permission/saved?projectID=` → `{data: [rule]}`, the project's saved "Always
    /// allow" rules (read only).
    PermissionSaved {
        /// The project id (`projectID` of a session).
        project: &'a str,
    },
    /// `GET /api/form` → `{data: [form]}`, the directory's pending forms.
    FormList {
        /// The project directory.
        directory: &'a str,
    },
    /// `POST /api/session/{id}/form/{formID}/reply` (`{answer: {key: value}}`); `204`.
    FormReply {
        /// The session.
        session: &'a SessionId,
        /// The form.
        form: &'a FormId,
        /// The project directory.
        directory: &'a str,
    },
    /// `DELETE /api/session/{id}/form/{formID}`: cancels a pending form; `204`.
    FormCancel {
        /// The session.
        session: &'a SessionId,
        /// The form.
        form: &'a FormId,
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/agent` → `{data: [agent], location}`.
    AgentList {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/command` → `{data: [command], location}`.
    CommandList {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/skill` → `{data: [skill], location}` (each with its instructions, which the chat
    /// does not keep).
    SkillList {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/config` → `[source]` (read only, not enveloped).
    ConfigGet {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/model` → `{data: [model], location}`.
    ModelList {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/model/default` → `{data: model?, location}`: the model a session without its own
    /// selection uses.
    ModelDefault {
        /// The project directory.
        directory: &'a str,
    },
    /// `GET /api/provider` → `{data: [provider], location}`.
    ProviderList {
        /// The project directory.
        directory: &'a str,
    },
}

impl RouteV2<'_> {
    /// The request method.
    pub fn method(&self) -> Method {
        match self {
            Self::Info
            | Self::Event
            | Self::SessionChildren { .. }
            | Self::SessionGet { .. }
            | Self::Messages { .. }
            | Self::PermissionList { .. }
            | Self::PermissionRequests { .. }
            | Self::PermissionSaved { .. }
            | Self::FormList { .. }
            | Self::AgentList { .. }
            | Self::CommandList { .. }
            | Self::SkillList { .. }
            | Self::ConfigGet { .. }
            | Self::ModelList { .. }
            | Self::ModelDefault { .. }
            | Self::ProviderList { .. } => Method::Get,
            Self::SessionUpdate { .. } => Method::Patch,
            Self::SessionDelete { .. } | Self::FormCancel { .. } => Method::Delete,
            _ => Method::Post,
        }
    }

    /// True for the calls that answer only when their work is done: `shell` waits for the command
    /// to end. Prompt, command and compact only admit their input and answer at once (recorded).
    pub fn blocks(&self) -> bool {
        matches!(self, Self::Shell { .. })
    }

    /// The request target: path and query, with every value encoded.
    pub fn target(&self) -> String {
        match *self {
            Self::Info => "/api/info".to_owned(),
            Self::Event => "/api/event".to_owned(),
            Self::SessionCreate { directory } => located("/api/session", Vec::new(), directory),
            Self::SessionChildren { parent, limit } => format!(
                "/api/session?limit={}&parentID={parent}",
                limit.clamp(1, MAX_SESSION_PAGE)
            ),
            Self::SessionGet { session, directory }
            | Self::SessionUpdate { session, directory }
            | Self::SessionDelete { session, directory } => {
                located(&format!("/api/session/{session}"), Vec::new(), directory)
            }
            Self::SessionFork { session, directory } => in_session(session, "fork", directory),
            Self::SessionAgent { session, directory } => in_session(session, "agent", directory),
            Self::SessionModel { session, directory } => in_session(session, "model", directory),
            Self::Prompt { session, directory } => in_session(session, "prompt", directory),
            Self::Interrupt { session, directory } => in_session(session, "interrupt", directory),
            Self::Command { session, directory } => in_session(session, "command", directory),
            Self::Shell { session, directory } => in_session(session, "shell", directory),
            Self::Compact { session, directory } => in_session(session, "compact", directory),
            Self::Messages {
                session,
                directory,
                limit,
                cursor,
                order,
            } => messages(session, directory, limit, cursor, order),
            Self::PermissionList { session, directory } => in_session(session, "permission", directory),
            Self::PermissionRequests { directory } => located("/api/permission/request", Vec::new(), directory),
            Self::PermissionReply {
                session,
                permission,
                directory,
            } => in_session(session, &format!("permission/{permission}/reply"), directory),
            Self::PermissionSaved { project } => {
                format!("/api/permission/saved?projectID={}", percent_encode(project))
            }
            Self::FormList { directory } => located("/api/form", Vec::new(), directory),
            Self::FormReply {
                session,
                form,
                directory,
            } => in_session(session, &format!("form/{form}/reply"), directory),
            Self::FormCancel {
                session,
                form,
                directory,
            } => in_session(session, &format!("form/{form}"), directory),
            Self::AgentList { directory } => located("/api/agent", Vec::new(), directory),
            Self::CommandList { directory } => located("/api/command", Vec::new(), directory),
            Self::SkillList { directory } => located("/api/skill", Vec::new(), directory),
            Self::ConfigGet { directory } => located("/api/config", Vec::new(), directory),
            Self::ModelList { directory } => located("/api/model", Vec::new(), directory),
            Self::ModelDefault { directory } => located("/api/model/default", Vec::new(), directory),
            Self::ProviderList { directory } => located("/api/provider", Vec::new(), directory),
        }
    }
}

impl Sealed for RouteV2<'_> {}

impl ApiRoute for RouteV2<'_> {
    fn method(&self) -> Method {
        RouteV2::method(self)
    }

    fn target(&self) -> String {
        RouteV2::target(self)
    }

    fn blocks(&self) -> bool {
        RouteV2::blocks(self)
    }

    fn is_stream(&self) -> bool {
        matches!(self, Self::Event)
    }
}

/// `path?<query…>&location%5Bdirectory%5D=<directory>`: the location goes last, as recorded.
fn located(path: &str, mut query: Vec<String>, directory: &str) -> String {
    query.push(format!("location%5Bdirectory%5D={}", percent_encode(directory)));

    format!("{path}?{}", query.join("&"))
}

fn in_session(session: &SessionId, rest: &str, directory: &str) -> String {
    located(&format!("/api/session/{session}/{rest}"), Vec::new(), directory)
}

fn messages(
    session: &SessionId,
    directory: &str,
    limit: u32,
    cursor: Option<&str>,
    order: Option<MessageOrder>,
) -> String {
    let mut query = vec![format!("limit={}", limit.clamp(1, MAX_MESSAGE_PAGE))];

    if let Some(order) = order {
        query.push(format!("order={}", order.as_str()));
    }

    if let Some(cursor) = cursor {
        query.push(format!("cursor={}", percent_encode(cursor)));
    }

    located(&format!("/api/session/{session}/message"), query, directory)
}
