//! Normalized chat data shared by agent events and replies: messages, models, session state and
//! usage. Harness payloads are mapped onto these types field by field; nothing is passed through
//! verbatim, so provider configuration (headers, base URLs, credentials) never reaches the webview.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use ts_rs::TS;

/// A model the agent can switch to.
///
/// Deliberately limited to display fields: provider payloads such as `headers`, `baseUrl`,
/// `compat` or `api` are never copied because they can hold credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ModelOption {
    /// Provider id (e.g. `"anthropic"`).
    pub provider: String,
    /// Model id within the provider (e.g. `"claude-sonnet-4-5"`).
    pub id: String,
    /// Display name, when the harness reports one.
    pub name: Option<String>,
    /// True when the model supports a thinking level.
    pub reasoning: bool,
    /// Context window in tokens.
    #[ts(type = "number | null")]
    pub context_window: Option<u64>,
    /// Maximum output tokens.
    #[ts(type = "number | null")]
    pub max_tokens: Option<u64>,
    /// Accepted input kinds (e.g. `["text", "image"]`).
    pub input: Vec<String>,
}

/// A primary agent the chat can switch to (OpenCode `build`, `plan` or a custom one).
///
/// Display fields only: the agent's prompt, tools and permission rules stay in the harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentOption {
    /// Agent id, sent back in `setAgent` (e.g. `"plan"`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// One-line description, when the agent has one.
    pub description: Option<String>,
}

/// How full the context window is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContextUsage {
    /// Tokens currently in context.
    #[ts(type = "number")]
    pub tokens: u64,
    /// Size of the context window in tokens.
    #[ts(type = "number")]
    pub context_window: u64,
    /// `tokens / contextWindow` as a percentage (0–100).
    pub percent: f64,
}

/// Snapshot of a running agent session (no session file path is exposed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionState {
    /// Harness session id (opaque).
    pub session_id: String,
    /// User-assigned session name.
    pub session_name: Option<String>,
    /// The active model.
    pub model: Option<ModelOption>,
    /// The active thinking level (e.g. `"medium"`).
    pub thinking_level: Option<String>,
    /// The active agent id, for harnesses with an agent picker (see `Capabilities.agent_picker`).
    pub agent: Option<String>,
    /// True while the agent is producing a response.
    pub is_streaming: bool,
    /// True while the context is being compacted.
    pub is_compacting: bool,
    /// Messages in the session.
    pub message_count: u32,
    /// Steering and follow-up messages waiting to be delivered.
    pub queued_count: u32,
    /// Context window usage, when known.
    pub context_usage: Option<ContextUsage>,
    /// Whether the chat's own memory guards apply now, from a harness that can learn it only
    /// later (Codex reads its MCP servers again for every new thread). Without it the session's
    /// `AgentSessionInfo.memory_guards` holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub memory_guards: Option<bool>,
}

/// Token counts of one message or a whole session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TokenUsage {
    /// Input (prompt) tokens.
    #[ts(type = "number")]
    pub input: u64,
    /// Output (completion) tokens.
    #[ts(type = "number")]
    pub output: u64,
    /// Tokens read from the prompt cache.
    #[ts(type = "number")]
    pub cache_read: u64,
    /// Tokens written to the prompt cache.
    #[ts(type = "number")]
    pub cache_write: u64,
    /// Sum of all of the above.
    #[ts(type = "number")]
    pub total: u64,
}

/// Session-wide statistics (`/session` stats).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionStats {
    /// Messages sent by the user.
    pub user_messages: u32,
    /// Messages produced by the assistant.
    pub assistant_messages: u32,
    /// Tool calls made by the assistant.
    pub tool_calls: u32,
    /// All messages in the session.
    pub total_messages: u32,
    /// Accumulated token usage.
    pub tokens: TokenUsage,
    /// Accumulated API-equivalent cost in US dollars, when the harness reports it.
    pub cost_usd: Option<f64>,
    /// Context window usage, when known.
    pub context_usage: Option<ContextUsage>,
}

/// Who or what produced a chat message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MessageRole {
    /// The user's prompt.
    User,
    /// The model's reply.
    Assistant,
    /// The result of a tool call.
    ToolResult,
    /// A `!command` run by the user.
    Bash,
    /// A message injected by an extension.
    Custom,
    /// A system message.
    System,
    /// Summary of an abandoned branch.
    BranchSummary,
    /// Summary that replaced compacted context.
    CompactionSummary,
    /// A role this app does not know yet.
    Other,
}

/// One content block of a chat message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum ChatBlock {
    /// Markdown text.
    Text {
        /// The text.
        text: String,
    },
    /// Model reasoning.
    Thinking {
        /// The reasoning text (empty when redacted).
        text: String,
        /// True when the provider withheld the reasoning.
        redacted: bool,
    },
    /// A tool call made by the assistant.
    ToolCall {
        /// Tool call id; matches `toolStart`/`toolEnd` events and the tool result message.
        id: String,
        /// Tool name (e.g. `"read"`).
        name: String,
        /// Tool arguments as sent by the model.
        args: JsonValue,
        /// Short human-readable intent, when the harness provides one.
        intent: Option<String>,
    },
    /// An image; the data is never sent to the webview.
    Image {
        /// MIME type (e.g. `"image/png"`).
        mime_type: String,
    },
    /// A block kind this app does not render.
    Other {
        /// The original block type.
        kind: String,
    },
}

/// A normalized chat message.
///
/// For tool results the textual output is carried as text blocks; tool `details` are dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatMessage {
    /// Session entry id, when known (used to fork from this message).
    pub id: Option<String>,
    /// Who produced the message.
    pub role: MessageRole,
    /// Content blocks in order.
    pub blocks: Vec<ChatBlock>,
    /// Model id that produced an assistant message.
    pub model: Option<String>,
    /// Provider of `model`.
    pub provider: Option<String>,
    /// Why generation stopped (e.g. `"stop"`, `"toolUse"`, `"aborted"`, `"error"`).
    pub stop_reason: Option<String>,
    /// Provider error text of a failed assistant message.
    pub error_message: Option<String>,
    /// Token usage of an assistant message.
    pub usage: Option<TokenUsage>,
    /// API-equivalent cost of an assistant message in US dollars.
    pub cost_usd: Option<f64>,
    /// Creation time in milliseconds since the Unix epoch.
    #[ts(type = "number | null")]
    pub timestamp: Option<i64>,
    /// For tool results: the tool call this answers.
    pub tool_call_id: Option<String>,
    /// For tool results: the tool name.
    pub tool_name: Option<String>,
    /// For tool results: true when the tool failed.
    pub is_error: Option<bool>,
}

/// How a prompt finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PromptStatus {
    /// The agent finished normally.
    Completed,
    /// The user aborted the turn.
    Aborted,
    /// The turn failed (see the accompanying error).
    Error,
    /// The harness handled the prompt itself (e.g. a slash command) without invoking the model.
    Handled,
}
