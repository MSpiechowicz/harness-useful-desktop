//! Payloads of the session lifecycle events: creation, naming, forks, status, runs, inbox, usage,
//! compaction, user shell commands, permissions and forms.

use serde::Deserialize;
use serde_json::Value;

use crate::opencode_server_v2::wire::prompt::{Form, InboxItem};
use crate::opencode_server_v2::wire::session::{ErrorInfo, ForkBoundary, ModelRef, Tokens, count};

/// `{sessionID}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SessionRef {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
}

/// `session.created`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SessionCreated {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The parent of a child session.
    #[serde(default, rename = "parentID")]
    pub parent_id: Option<String>,
    /// The title, when set at creation.
    #[serde(default)]
    pub title: Option<String>,
    /// The model.
    #[serde(default)]
    pub model: Option<ModelRef>,
}

/// `session.renamed`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Renamed {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The new title.
    pub title: String,
}

/// `session.forked`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Forked {
    /// The new session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The session it was forked from.
    #[serde(rename = "parentID")]
    pub parent_id: String,
    /// Where the copy stops.
    pub boundary: ForkBoundary,
}

/// `session.agent.selected`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AgentSelected {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The agent id.
    pub agent: String,
}

/// `session.model.selected`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ModelSelected {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The model.
    pub model: ModelRef,
}

/// `session.status`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StatusChanged {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The new status.
    pub status: SessionStatus,
}

/// A session's status: `{type: idle | busy | retry}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SessionStatus {
    /// Not running.
    Idle,
    /// Running.
    Busy,
    /// Waiting to retry a failed model call.
    Retry {
        /// The attempt number.
        #[serde(default, deserialize_with = "count")]
        attempt: u64,
        /// Why.
        #[serde(default)]
        message: String,
        /// When the next attempt starts, in milliseconds since the epoch.
        #[serde(default, deserialize_with = "count")]
        next: u64,
    },
    /// A status this app does not know.
    #[serde(other)]
    Other,
}

/// `session.execution.failed`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExecutionFailed {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// What went wrong.
    #[serde(default)]
    pub error: ErrorInfo,
}

/// `session.execution.interrupted`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExecutionInterrupted {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// `user`, `shutdown`, `superseded` or `inactivity`.
    #[serde(default)]
    pub reason: String,
}

/// `session.inbox.enqueued`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct InboxEnqueued {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The item's id (`msg_…`, also the delivered message's id).
    #[serde(rename = "inboxID")]
    pub inbox_id: String,
    /// The item.
    pub item: InboxItem,
}

/// `{sessionID, inboxID}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct InboxRef {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The item.
    #[serde(rename = "inboxID")]
    pub inbox_id: String,
}

/// `session.usage.updated`: the session's totals.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Usage {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// Total cost in USD.
    #[serde(default)]
    pub cost: f64,
    /// Total tokens.
    #[serde(default)]
    pub tokens: Tokens,
}

/// `session.compaction.started`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CompactionStarted {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// `auto` or `manual`.
    #[serde(default)]
    pub reason: String,
}

/// `session.compaction.delta`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CompactionDelta {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// A fragment of the summary.
    #[serde(default)]
    pub text: String,
}

/// `session.compaction.ended`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CompactionEnded {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// `auto` or `manual`.
    #[serde(default)]
    pub reason: String,
    /// The summary.
    #[serde(default)]
    pub text: String,
}

/// `session.compaction.failed`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CompactionFailed {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// `auto` or `manual`.
    #[serde(default)]
    pub reason: String,
    /// What went wrong.
    #[serde(default)]
    pub error: ErrorInfo,
}

/// `session.shell.started`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShellStarted {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The command.
    pub shell: ShellInfo,
}

/// `session.shell.ended`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShellEnded {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The command.
    pub shell: ShellInfo,
    /// Its output.
    #[serde(default)]
    pub output: ShellOutput,
}

/// A user shell command: `{id, status, command, cwd, exit?}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ShellInfo {
    /// `sh_…`.
    pub id: String,
    /// `running`, `exited`, …
    #[serde(default)]
    pub status: String,
    /// The command line.
    #[serde(default)]
    pub command: String,
    /// Where it runs.
    #[serde(default)]
    pub cwd: String,
    /// The exit code, once it ended.
    #[serde(default)]
    pub exit: Option<i64>,
}

/// A shell command's merged output (a preview when `truncated`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ShellOutput {
    /// The output.
    #[serde(default)]
    pub output: String,
    /// True when only the start is here.
    #[serde(default)]
    pub truncated: bool,
}

/// `permission.replied`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PermissionReplied {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The request.
    #[serde(rename = "requestID")]
    pub request_id: String,
    /// `once`, `always` or `reject`.
    #[serde(default)]
    pub reply: String,
}

/// `form.created`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormCreated {
    /// The form.
    pub form: Form,
}

/// `form.replied` and `form.cancelled`: `{id, sessionID, answer?}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct FormSettled {
    /// The form.
    pub id: String,
    /// The session.
    #[serde(default, rename = "sessionID")]
    pub session_id: String,
    /// The answers by field key (replies only).
    #[serde(default)]
    pub answer: serde_json::Map<String, Value>,
}
