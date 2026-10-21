//! Payloads of the events of one model step: step, text, reasoning and tool events.

use serde::Deserialize;
use serde_json::Value;

use crate::opencode_server_v2::wire::session::{ErrorInfo, ModelRef, Tokens, count};

/// `session.step.started`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StepStarted {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message the step writes.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
    /// The agent.
    #[serde(default)]
    pub agent: String,
    /// The model.
    #[serde(default)]
    pub model: Option<ModelRef>,
}

/// `{sessionID, assistantMessageID}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StepRef {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
}

/// `session.step.ended`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StepEnded {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
    /// `stop`, `tool-calls`, `length`, …
    #[serde(default)]
    pub finish: String,
    /// The step's cost in USD.
    #[serde(default)]
    pub cost: f64,
    /// The step's tokens.
    #[serde(default)]
    pub tokens: Tokens,
    /// Files the step changed, relative to the project.
    #[serde(default)]
    pub files: Vec<String>,
}

/// `session.step.failed`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StepFailed {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
    /// What went wrong (`aborted` for an interruption).
    #[serde(default)]
    pub error: ErrorInfo,
    /// Files the step changed.
    #[serde(default)]
    pub files: Vec<String>,
}

/// The start of a text or reasoning part: `{sessionID, assistantMessageID, ordinal}`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PartRef {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
    /// The part's position among the message's parts of its kind.
    #[serde(default, deserialize_with = "count")]
    pub ordinal: u64,
}

/// A text or reasoning fragment.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PartDelta {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
    /// The part.
    #[serde(default, deserialize_with = "count")]
    pub ordinal: u64,
    /// The fragment.
    pub delta: String,
}

/// The end of a text or reasoning part, with its full text.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PartEnded {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
    /// The part.
    #[serde(default, deserialize_with = "count")]
    pub ordinal: u64,
    /// The full text.
    pub text: String,
}

/// `session.tool.input.started`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ToolInputStarted {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
    /// The tool call id (`call_…`).
    pub id: String,
    /// The tool (`shell`, `edit`, `write`, `read`, `question`, …).
    pub name: String,
}

/// `session.tool.input.delta`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ToolInputDelta {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The tool call id.
    pub id: String,
    /// A fragment of the raw input.
    pub delta: String,
}

/// `session.tool.input.ended`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ToolInputEnded {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The tool call id.
    pub id: String,
    /// The raw input text (JSON).
    #[serde(default)]
    pub text: String,
}

/// `session.tool.called`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolCalled {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
    /// The tool call id.
    pub id: String,
    /// The parsed input (`{path, …}` for edit/write/read, `{command}` for shell).
    #[serde(default)]
    pub input: Value,
}

/// `session.tool.progress`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolProgress {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The tool call id.
    pub id: String,
    /// Live metadata (`{shellID}` for shell).
    #[serde(default)]
    pub metadata: Value,
}

/// `session.tool.success`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolSuccess {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
    /// The tool call id.
    pub id: String,
    /// What the model reads.
    #[serde(default)]
    pub content: Vec<ToolContent>,
    /// UI metadata (`{exit, truncated}` for shell, …).
    #[serde(default)]
    pub metadata: Value,
}

/// `session.tool.failed`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolFailed {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The assistant message.
    #[serde(rename = "assistantMessageID")]
    pub assistant_message_id: String,
    /// The tool call id.
    pub id: String,
    /// What went wrong.
    #[serde(default)]
    pub error: ErrorInfo,
    /// Partial output.
    #[serde(default)]
    pub content: Vec<ToolContent>,
}

/// One piece of a tool result.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    /// Text.
    Text {
        /// The text.
        #[serde(default)]
        text: String,
    },
    /// A file.
    File {
        /// Its URI.
        #[serde(default)]
        uri: String,
        /// Its media type.
        #[serde(default)]
        mime: String,
    },
    /// A content kind this app does not know.
    #[serde(other)]
    Other,
}

/// `session.retry.scheduled`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RetryScheduled {
    /// The session.
    #[serde(rename = "sessionID")]
    pub session_id: String,
    /// The attempt number.
    #[serde(default, deserialize_with = "count")]
    pub attempt: u64,
    /// When it runs, in milliseconds since the epoch.
    #[serde(default, deserialize_with = "count")]
    pub at: u64,
    /// What failed.
    #[serde(default)]
    pub error: ErrorInfo,
}
