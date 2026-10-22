//! `assistant`, `user` and `stream_event` frames: Anthropic Messages API messages, content blocks
//! and streaming events, wrapped by Claude Code with the session id, the frame's entry `uuid` and
//! `parent_tool_use_id` (set on the frames of a subagent).

use serde::Deserialize;
use serde_json::Value;

use super::nullable;

/// An `assistant` frame: one content block of an API message (several frames share `message.id`).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct AssistantFrame {
    #[serde(deserialize_with = "nullable")]
    pub message: ApiMessage,
    /// The `Task` call whose subagent wrote this frame; `None` for the chat's own frames.
    pub parent_tool_use_id: Option<String>,
    /// The session entry id (a fork point).
    pub uuid: Option<String>,
    pub session_id: Option<String>,
    /// When the CLI wrote it (RFC 3339).
    pub timestamp: Option<String>,
    /// The kind of API error a synthetic error message reports (`authentication_failed`,
    /// `rate_limit`, `overloaded`, …).
    pub error: Option<String>,
    /// True for a synthetic message that reports an API error instead of a reply.
    #[serde(deserialize_with = "nullable")]
    pub is_api_error_message: bool,
    /// True when the block was cut off by an interrupt.
    #[serde(deserialize_with = "nullable")]
    pub aborted: bool,
}

/// An API message (`message` of an `assistant` frame, of `message_start`).
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct ApiMessage {
    /// `msg_…` (a UUID on synthetic messages).
    #[serde(deserialize_with = "nullable")]
    pub id: String,
    /// The model id (`<synthetic>` on synthetic messages).
    pub model: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
    pub usage: Option<ApiUsage>,
}

/// Token counts of one API call (or of a turn's main loop, on `result`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct ApiUsage {
    #[serde(deserialize_with = "nullable")]
    pub input_tokens: u64,
    #[serde(deserialize_with = "nullable")]
    pub output_tokens: u64,
    #[serde(deserialize_with = "nullable")]
    pub cache_read_input_tokens: u64,
    #[serde(deserialize_with = "nullable")]
    pub cache_creation_input_tokens: u64,
}

impl ApiUsage {
    /// Every token the call read or wrote.
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_input_tokens + self.cache_creation_input_tokens
    }
}

/// A content block. Never fails to decode: a kind this app does not know is [`ContentBlock::Other`].
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(from = "Value")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    /// Reasoning; empty when the API withheld it (Claude Code streams only its size then).
    Thinking {
        thinking: String,
    },
    /// Reasoning the API sent encrypted.
    RedactedThinking,
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    /// The result of a tool call (in `user` frames). `content` is a string or a list of blocks.
    ToolResult {
        tool_use_id: String,
        content: Value,
        is_error: bool,
    },
    Image {
        media_type: Option<String>,
    },
    /// A block kind this app does not read (`server_tool_use`, `document`, …).
    Other {
        kind: String,
    },
}

impl From<Value> for ContentBlock {
    fn from(block: Value) -> Self {
        let text = |key: &str| block.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        let kind = text("type");

        match kind.as_str() {
            "text" => Self::Text { text: text("text") },
            "thinking" => Self::Thinking {
                thinking: text("thinking"),
            },
            "redacted_thinking" => Self::RedactedThinking,
            "tool_use" => Self::ToolUse {
                id: text("id"),
                name: text("name"),
                input: block.get("input").cloned().unwrap_or(Value::Null),
            },
            "tool_result" => Self::ToolResult {
                tool_use_id: text("tool_use_id"),
                content: block.get("content").cloned().unwrap_or(Value::Null),
                is_error: block.get("is_error").and_then(Value::as_bool).unwrap_or(false),
            },
            "image" => Self::Image {
                media_type: block
                    .pointer("/source/media_type")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            },
            _ => Self::Other { kind },
        }
    }
}

/// A `user` frame.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct UserFrame {
    #[serde(deserialize_with = "nullable")]
    pub message: UserBody,
    pub parent_tool_use_id: Option<String>,
    pub uuid: Option<String>,
    pub session_id: Option<String>,
    /// True for a message the CLI made up (a compaction summary).
    #[serde(rename = "isSynthetic", deserialize_with = "nullable")]
    pub is_synthetic: bool,
    /// True for an echo of something the CLI already handled (local command output).
    #[serde(rename = "isReplay", deserialize_with = "nullable")]
    pub is_replay: bool,
}

/// The `message` of a `user` frame.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default)]
pub struct UserBody {
    #[serde(deserialize_with = "nullable")]
    pub content: UserContent,
}

/// A user message's content: plain text or blocks.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

impl Default for UserContent {
    fn default() -> Self {
        Self::Blocks(Vec::new())
    }
}

impl UserContent {
    /// The text of a plain message, or of its text blocks joined; `None` without text.
    pub fn text(&self) -> Option<String> {
        match self {
            Self::Text(text) => Some(text.clone()),
            Self::Blocks(blocks) => {
                let texts: Vec<&str> = blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();

                (!texts.is_empty()).then(|| texts.join("\n"))
            }
        }
    }

    /// The content blocks (none for plain text).
    pub fn blocks(&self) -> &[ContentBlock] {
        match self {
            Self::Text(_) => &[],
            Self::Blocks(blocks) => blocks,
        }
    }
}

/// A `stream_event` frame.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StreamEventFrame {
    pub event: StreamEvent,
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    /// The API message the event belongs to.
    #[serde(default)]
    pub api_message_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// A streaming event of the Messages API.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart {
        #[serde(default, deserialize_with = "nullable")]
        message: ApiMessage,
    },
    ContentBlockStart {
        #[serde(default, deserialize_with = "nullable")]
        index: u32,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        #[serde(default, deserialize_with = "nullable")]
        index: u32,
        delta: Delta,
    },
    ContentBlockStop {
        #[serde(default, deserialize_with = "nullable")]
        index: u32,
    },
    MessageDelta {
        #[serde(default, deserialize_with = "nullable")]
        delta: MessageDeltaBody,
        #[serde(default)]
        usage: Option<ApiUsage>,
    },
    MessageStop,
    /// `ping`, `error` and kinds this app does not know.
    #[serde(other)]
    Other,
}

/// The `delta` of `message_delta`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(default)]
pub struct MessageDeltaBody {
    pub stop_reason: Option<String>,
}

/// The `delta` of `content_block_delta`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Delta {
    TextDelta {
        #[serde(default, deserialize_with = "nullable")]
        text: String,
    },
    ThinkingDelta {
        #[serde(default, deserialize_with = "nullable")]
        thinking: String,
    },
    InputJsonDelta {
        #[serde(default, deserialize_with = "nullable")]
        partial_json: String,
    },
    /// `signature_delta`, `citations_delta` and kinds this app does not know.
    #[serde(other)]
    Other,
}
