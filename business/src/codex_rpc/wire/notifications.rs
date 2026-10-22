//! What Codex reports (`ServerNotification`), by `method`. Every one carries `emittedAtMs`, which
//! the app does not read.
//!
//! Noise the chat does not show is [`Notification::Folded`] without its params being read: MCP
//! server startup, moderation metadata, the turn's running diff, settings and rate-limit echoes,
//! remote control and the deprecated `thread/compacted`. A method this app does not know is
//! [`Notification::Unknown`].

use serde::Deserialize;
use serde_json::Value;

use super::items::{FileUpdateChange, ThreadItem, Turn, TurnError};
use super::results::{Thread, ThreadStatus};
use super::{RequestId, params};
use crate::claude_stream::wire::{lenient, lenient_option, nullable};

/// Notifications the chat folds away unread (see the module docs). The app may also opt out of
/// them in `initialize`.
pub const FOLDED: [&str; 8] = [
    "mcpServer/startupStatus/updated",
    "turn/moderationMetadata",
    "turn/diff/updated",
    "thread/settings/updated",
    "account/rateLimits/updated",
    "remoteControl/status/changed",
    "thread/compacted",
    "turn/plan/updated",
];

/// One notification.
#[derive(Debug, Clone, PartialEq)]
pub enum Notification {
    /// A thread was started, resumed or forked in this process.
    ThreadStarted(Box<Thread>),
    ThreadStatusChanged(StatusChanged),
    /// The thread was renamed (`thread/name/set`, or by Codex).
    ThreadNameUpdated(NameUpdated),
    /// The thread's token counts after a model call (and its context window).
    TokenUsageUpdated(Box<TokenUsageUpdated>),
    TurnStarted(TurnNotification),
    /// A turn ended: completed, interrupted or failed.
    TurnCompleted(TurnNotification),
    ItemStarted(ItemNotification),
    ItemCompleted(ItemNotification),
    /// Text streamed into an agent message.
    AgentMessageDelta(ItemDelta),
    /// Raw reasoning text streamed into a reasoning item.
    ReasoningTextDelta(ItemDelta),
    /// Summary text streamed into a reasoning item.
    ReasoningSummaryTextDelta(ItemDelta),
    /// A new summary paragraph of a reasoning item begins.
    ReasoningSummaryPartAdded(ItemDelta),
    /// Output of a running command.
    CommandOutputDelta(ItemDelta),
    /// A file change's patch changed before it was applied.
    FileChangePatchUpdated(PatchUpdated),
    /// Progress of an MCP tool call (its `message`).
    McpToolCallProgress(ItemDelta),
    /// A model call failed: Codex retries it, or the turn fails.
    Error(ErrorNotification),
    Warning(Warning),
    /// An API the app uses is deprecated.
    DeprecationNotice(DeprecationNotice),
    /// A request Codex asked was answered or withdrawn: its dialog closes.
    ServerRequestResolved(Resolved),
    /// Known noise (see [`FOLDED`]).
    Folded {
        method: String,
    },
    /// A method this app does not know.
    Unknown {
        method: String,
    },
}

impl Notification {
    /// The thread the notification is about, when it names one.
    pub fn thread_id(&self) -> Option<&str> {
        let id = match self {
            Self::ThreadStarted(started) => &started.id,
            Self::ThreadStatusChanged(changed) => &changed.thread_id,
            Self::ThreadNameUpdated(updated) => &updated.thread_id,
            Self::TokenUsageUpdated(updated) => &updated.thread_id,
            Self::TurnStarted(turn) | Self::TurnCompleted(turn) => &turn.thread_id,
            Self::ItemStarted(item) | Self::ItemCompleted(item) => &item.thread_id,
            Self::AgentMessageDelta(delta)
            | Self::ReasoningTextDelta(delta)
            | Self::ReasoningSummaryTextDelta(delta)
            | Self::ReasoningSummaryPartAdded(delta)
            | Self::CommandOutputDelta(delta)
            | Self::McpToolCallProgress(delta) => &delta.thread_id,
            Self::FileChangePatchUpdated(updated) => &updated.thread_id,
            Self::Error(error) => &error.thread_id,
            Self::Warning(warning) => return warning.thread_id.as_deref().filter(|id| !id.is_empty()),
            Self::ServerRequestResolved(resolved) => &resolved.thread_id,
            Self::DeprecationNotice(_) | Self::Folded { .. } | Self::Unknown { .. } => return None,
        };

        (!id.is_empty()).then_some(id.as_str())
    }
}

/// Reads notification `method` with `params`; `Err(())` when known params do not fit.
pub(super) fn decode(method: &str, raw: Value) -> Result<Notification, ()> {
    let notification = match method {
        "thread/started" => Notification::ThreadStarted(Box::new(params::<Started>(raw)?.thread)),
        "thread/status/changed" => Notification::ThreadStatusChanged(params(raw)?),
        "thread/name/updated" => Notification::ThreadNameUpdated(params(raw)?),
        "thread/tokenUsage/updated" => Notification::TokenUsageUpdated(Box::new(params(raw)?)),
        "turn/started" => Notification::TurnStarted(params(raw)?),
        "turn/completed" => Notification::TurnCompleted(params(raw)?),
        "item/started" => Notification::ItemStarted(params(raw)?),
        "item/completed" => Notification::ItemCompleted(params(raw)?),
        "item/agentMessage/delta" => Notification::AgentMessageDelta(params(raw)?),
        "item/reasoning/textDelta" => Notification::ReasoningTextDelta(params(raw)?),
        "item/reasoning/summaryTextDelta" => Notification::ReasoningSummaryTextDelta(params(raw)?),
        "item/reasoning/summaryPartAdded" => Notification::ReasoningSummaryPartAdded(params(raw)?),
        "item/commandExecution/outputDelta" => Notification::CommandOutputDelta(params(raw)?),
        "item/fileChange/patchUpdated" => Notification::FileChangePatchUpdated(params(raw)?),
        "item/mcpToolCall/progress" => Notification::McpToolCallProgress(params(raw)?),
        "error" => Notification::Error(params(raw)?),
        "warning" => Notification::Warning(params(raw)?),
        "deprecationNotice" => Notification::DeprecationNotice(params(raw)?),
        "serverRequest/resolved" => Notification::ServerRequestResolved(params(raw)?),
        folded if FOLDED.contains(&folded) => Notification::Folded {
            method: folded.to_owned(),
        },
        other => Notification::Unknown {
            method: other.to_owned(),
        },
    };

    Ok(notification)
}

#[derive(Deserialize)]
struct Started {
    thread: Thread,
}

/// `thread/status/changed`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StatusChanged {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    #[serde(deserialize_with = "nullable")]
    pub status: ThreadStatus,
}

/// `thread/name/updated`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NameUpdated {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    /// `None` when the name was cleared.
    pub thread_name: Option<String>,
}

/// `thread/tokenUsage/updated`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TokenUsageUpdated {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    #[serde(deserialize_with = "nullable")]
    pub turn_id: String,
    pub token_usage: ThreadTokenUsage,
}

/// A thread's token counts: in total, of the newest model call, and its model's context window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThreadTokenUsage {
    pub total: TokenBreakdown,
    pub last: TokenBreakdown,
    #[serde(deserialize_with = "lenient_option")]
    pub model_context_window: Option<u64>,
}

/// Token counts. `input_tokens` includes the cached ones, `output_tokens` the reasoning ones,
/// and `total_tokens` is input plus output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TokenBreakdown {
    #[serde(deserialize_with = "lenient")]
    pub total_tokens: u64,
    #[serde(deserialize_with = "lenient")]
    pub input_tokens: u64,
    #[serde(deserialize_with = "lenient")]
    pub cached_input_tokens: u64,
    #[serde(deserialize_with = "lenient")]
    pub cache_write_input_tokens: u64,
    #[serde(deserialize_with = "lenient")]
    pub output_tokens: u64,
    #[serde(deserialize_with = "lenient")]
    pub reasoning_output_tokens: u64,
}

/// `turn/started` and `turn/completed`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TurnNotification {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    pub turn: Turn,
}

/// `item/started` and `item/completed`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemNotification {
    pub item: ThreadItem,
    #[serde(default, deserialize_with = "nullable")]
    pub thread_id: String,
    #[serde(default, deserialize_with = "nullable")]
    pub turn_id: String,
    /// When the item started or completed, in milliseconds since the epoch.
    #[serde(default, alias = "startedAtMs", alias = "completedAtMs")]
    pub at_ms: Option<i64>,
}

/// A delta of an item (`delta`), or a progress `message`, or a new summary part.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ItemDelta {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    #[serde(deserialize_with = "nullable")]
    pub turn_id: String,
    #[serde(deserialize_with = "nullable")]
    pub item_id: String,
    #[serde(deserialize_with = "nullable", alias = "message")]
    pub delta: String,
    /// The summary paragraph a summary delta or part belongs to.
    #[serde(deserialize_with = "lenient")]
    pub summary_index: u32,
}

/// `item/fileChange/patchUpdated`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PatchUpdated {
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    #[serde(deserialize_with = "nullable")]
    pub item_id: String,
    #[serde(deserialize_with = "nullable")]
    pub changes: Vec<FileUpdateChange>,
}

/// `error`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ErrorNotification {
    pub error: TurnError,
    /// True when Codex tries the model call again.
    pub will_retry: bool,
    #[serde(deserialize_with = "nullable")]
    pub thread_id: String,
    #[serde(deserialize_with = "nullable")]
    pub turn_id: String,
}

/// `warning`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Warning {
    pub thread_id: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub message: String,
}

/// `deprecationNotice`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct DeprecationNotice {
    #[serde(deserialize_with = "nullable")]
    pub summary: String,
}

/// `serverRequest/resolved`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    #[serde(default, deserialize_with = "nullable")]
    pub thread_id: String,
    pub request_id: RequestId,
}
