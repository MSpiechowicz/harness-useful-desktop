//! OpenCode 2 session messages (`SessionMessage.Info` of `packages/schema/src/session-message.ts`:
//! the `messages` of `opencode session export` and of the server's message list) as a typed union,
//! and as the chat messages of the transcript and the chat history.
//!
//! Parsing is lenient ([`SessionMessage::parse`]): a message of an unknown type, or one whose shape
//! does not match, is [`SessionMessage::Other`]; absent fields take defaults. Attachment data
//! (base64) is never kept.
//!
//! Chat messages ([`SessionMessage::chat_messages`]):
//! - `user`: its text, images (MIME type only), and attached files and agents the text does not
//!   mention as `@name`.
//! - `assistant`: text, reasoning and tool calls in order, then a tool-result message per finished
//!   call with its redacted output or error.
//! - `shell` (a `!command`): a command message `$ command` with the redacted output.
//! - `compaction`: a compaction-summary message of its summary.
//! - Nothing else is shown: `synthetic` and `system` context, loaded skills, agent, model and
//!   location switches, `idle` markers.

use serde::Deserialize;
use serde_json::Value;

pub use super::tools::{ToolCall, ToolContent, ToolState};
use crate::model::{ChatBlock, ChatMessage, MessageRole, TokenUsage};
use crate::security::redact_text;

/// One OpenCode 2 session message.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SessionMessage {
    User(UserMessage),
    Assistant(AssistantMessage),
    Shell(ShellMessage),
    Compaction(CompactionMessage),
    Synthetic(ContextMessage),
    System(ContextMessage),
    Skill(SkillMessage),
    AgentSwitched(AgentSwitched),
    ModelSwitched(ModelSwitched),
    LocationSwitched(Marker),
    Idle(IdleMessage),
    /// A type this app does not know, or a message it cannot read.
    #[serde(other)]
    Other,
}

/// Creation and completion times (Unix milliseconds); other times are not kept.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Times {
    pub created: Option<i64>,
    pub completed: Option<i64>,
}

/// A model reference (`{id, providerID, variant?}`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModelRef {
    pub id: String,
    #[serde(rename = "providerID")]
    pub provider_id: String,
    pub variant: Option<String>,
}

/// A structured error (`{type, message}`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct SessionError {
    #[serde(rename = "type")]
    pub kind: String,
    pub message: String,
}

impl SessionError {
    /// True when the user (or a shutdown) interrupted the work.
    pub fn is_abort(&self) -> bool {
        self.kind == "aborted"
    }
}

/// Token counts of a reply or a compaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub reasoning: u64,
    pub cache: CacheTokens,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct CacheTokens {
    pub read: u64,
    pub write: u64,
}

impl Tokens {
    /// As the chat model's usage; the total includes reasoning tokens.
    pub fn usage(&self) -> TokenUsage {
        TokenUsage {
            input: self.input,
            output: self.output,
            cache_read: self.cache.read,
            cache_write: self.cache.write,
            total: self.input + self.output + self.reasoning + self.cache.read + self.cache.write,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UserMessage {
    pub id: String,
    pub time: Times,
    pub text: String,
    pub files: Vec<FileAttachment>,
    pub agents: Vec<AgentAttachment>,
}

/// An attached file; its data is not kept. `mention` is set when the prompt text names it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FileAttachment {
    pub mime: String,
    pub name: Option<String>,
    pub mention: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentAttachment {
    pub name: String,
    pub mention: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AssistantMessage {
    pub id: String,
    pub time: Times,
    pub agent: Option<String>,
    pub model: Option<ModelRef>,
    pub content: Vec<AssistantContent>,
    pub finish: Option<String>,
    pub cost: Option<f64>,
    pub tokens: Option<Tokens>,
    pub error: Option<SessionError>,
}

/// One item of an assistant message.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum AssistantContent {
    Text {
        #[serde(default)]
        text: String,
    },
    /// Reasoning; the text is empty when the provider withheld it.
    Reasoning {
        #[serde(default)]
        text: String,
    },
    Tool(ToolCall),
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShellMessage {
    pub id: String,
    pub time: Times,
    pub command: String,
    /// `running`, `exited`, `timeout` or `killed`.
    pub status: String,
    pub exit: Option<f64>,
    pub output: Option<ShellOutput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShellOutput {
    pub output: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CompactionMessage {
    pub id: String,
    pub time: Times,
    /// `running`, `completed` or `failed`.
    pub status: String,
    /// `auto` or `manual`.
    pub reason: String,
    pub summary: String,
    pub error: Option<SessionError>,
}

/// Context OpenCode added for the model (`synthetic`, `system`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ContextMessage {
    pub id: String,
    pub time: Times,
    pub text: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SkillMessage {
    pub id: String,
    pub time: Times,
    pub name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentSwitched {
    pub id: String,
    pub time: Times,
    pub agent: String,
    pub previous: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ModelSwitched {
    pub id: String,
    pub time: Times,
    pub model: ModelRef,
    pub previous: Option<ModelRef>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Marker {
    pub id: String,
    pub time: Times,
}

/// The session went idle: the end of a turn.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct IdleMessage {
    pub id: String,
    pub time: Times,
    /// `succeeded`, `failed` or `interrupted`.
    pub outcome: String,
    pub error: Option<SessionError>,
}

impl SessionMessage {
    /// The message in `value`; [`Self::Other`] when it is not one this app reads.
    pub fn parse(value: &Value) -> Self {
        Self::deserialize(value).unwrap_or(Self::Other)
    }

    /// The chat messages of this message (see the module docs).
    pub fn chat_messages(&self) -> Vec<ChatMessage> {
        match self {
            Self::User(user) => user_chat(user).into_iter().collect(),
            Self::Assistant(reply) => assistant_chat(reply),
            Self::Shell(shell) => vec![shell_chat(shell)],
            Self::Compaction(compaction) => compaction_chat(compaction).into_iter().collect(),
            _ => Vec::new(),
        }
    }

    /// The tool calls of an assistant message.
    pub fn tool_calls(&self) -> impl Iterator<Item = &ToolCall> {
        let content = match self {
            Self::Assistant(reply) => reply.content.as_slice(),
            _ => &[],
        };

        content.iter().filter_map(|item| match item {
            AssistantContent::Tool(call) => Some(call),
            _ => None,
        })
    }
}

/// The result message of a finished tool call.
pub fn tool_result_message(call: &ToolCall) -> Option<ChatMessage> {
    let (output, is_error) = call.outcome()?;

    let mut result = blank(&call.id, MessageRole::ToolResult, call.time.completed);
    result.blocks = vec![text_block(output)];
    result.tool_call_id = Some(call.id.clone());
    result.tool_name = Some(call.name.clone());
    result.is_error = Some(is_error);

    Some(result)
}

/// A message of `role` with no content yet.
fn blank(id: &str, role: MessageRole, timestamp: Option<i64>) -> ChatMessage {
    ChatMessage {
        id: (!id.is_empty()).then(|| id.to_owned()),
        role,
        blocks: Vec::new(),
        model: None,
        provider: None,
        stop_reason: None,
        error_message: None,
        usage: None,
        cost_usd: None,
        timestamp,
        tool_call_id: None,
        tool_name: None,
        is_error: None,
    }
}

fn text_block(text: impl Into<String>) -> ChatBlock {
    ChatBlock::Text { text: text.into() }
}

/// The prompt; `None` when nothing in it is shown.
fn user_chat(user: &UserMessage) -> Option<ChatMessage> {
    let mut blocks = Vec::new();

    if !user.text.trim().is_empty() {
        blocks.push(text_block(&user.text));
    }

    for file in &user.files {
        if file.mime.starts_with("image/") {
            blocks.push(ChatBlock::Image {
                mime_type: file.mime.clone(),
            });
        } else if file.mention.is_none()
            && let Some(name) = file.name.as_deref().filter(|name| !name.is_empty())
        {
            blocks.push(text_block(format!("@{name}")));
        }
    }

    for agent in user.agents.iter().filter(|agent| agent.mention.is_none()) {
        blocks.push(text_block(format!("@{}", agent.name)));
    }

    if blocks.is_empty() {
        return None;
    }

    let mut message = blank(&user.id, MessageRole::User, user.time.created);
    message.blocks = blocks;

    Some(message)
}

/// The reply followed by the results of its finished tool calls; nothing when it has neither
/// content nor an error.
fn assistant_chat(reply: &AssistantMessage) -> Vec<ChatMessage> {
    let mut message = blank(&reply.id, MessageRole::Assistant, reply.time.created);
    let mut results = Vec::new();

    for item in &reply.content {
        match item {
            AssistantContent::Text { text } if !text.is_empty() => message.blocks.push(text_block(text)),
            AssistantContent::Reasoning { text } => message.blocks.push(ChatBlock::Thinking {
                text: text.clone(),
                redacted: text.is_empty(),
            }),
            AssistantContent::Tool(call) => {
                message.blocks.push(call.block());
                results.extend(tool_result_message(call));
            }
            _ => {}
        }
    }

    let error = reply.error.as_ref();
    message.error_message = error
        .filter(|error| !error.message.is_empty())
        .map(|error| redact_text(&error.message));

    if message.blocks.is_empty() && message.error_message.is_none() {
        return Vec::new();
    }

    let model = reply.model.as_ref();
    message.model = model.map(|model| model.id.clone()).filter(|id| !id.is_empty());
    message.provider = model.map(|model| model.provider_id.clone()).filter(|id| !id.is_empty());

    // A failed or interrupted reply says so even though OpenCode records `finish: "error"`.
    message.stop_reason = match error {
        Some(error) if error.is_abort() => Some("aborted".to_owned()),
        Some(_) => Some("error".to_owned()),
        None => reply.finish.clone(),
    };

    message.cost_usd = reply.cost;
    message.usage = reply.tokens.map(|tokens| tokens.usage());

    std::iter::once(message).chain(results).collect()
}

/// A `!command` with its redacted output.
fn shell_chat(shell: &ShellMessage) -> ChatMessage {
    let output = shell
        .output
        .as_ref()
        .map(|output| redact_text(&output.output))
        .unwrap_or_default();

    let text = if output.is_empty() {
        format!("$ {}", shell.command)
    } else {
        format!("$ {}\n{output}", shell.command)
    };

    let mut message = blank(&shell.id, MessageRole::Bash, shell.time.created);
    message.blocks = vec![text_block(text)];

    message
}

/// The summary a compaction wrote; `None` without one.
fn compaction_chat(compaction: &CompactionMessage) -> Option<ChatMessage> {
    if compaction.summary.trim().is_empty() {
        return None;
    }

    let mut message = blank(&compaction.id, MessageRole::CompactionSummary, compaction.time.created);
    message.blocks = vec![text_block(&compaction.summary)];

    Some(message)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{SessionMessage, ToolState};

    #[test]
    fn unknown_and_unreadable_messages_are_other() {
        for value in [
            json!({ "type": "future-kind", "id": "msg_1" }),
            json!({ "type": "user", "text": 42 }),
            json!("not a message"),
        ] {
            assert!(
                matches!(SessionMessage::parse(&value), SessionMessage::Other),
                "{value}"
            );
        }
    }

    #[test]
    fn a_streaming_call_has_no_arguments_or_result_yet() {
        let value = json!({
            "type": "assistant", "id": "msg_1", "time": { "created": 1 },
            "content": [
                { "type": "tool", "id": "call_1", "name": "write", "state": { "status": "streaming", "input": "{\"pa" }, "time": { "created": 1 } },
                { "type": "future-part" }
            ]
        });

        let message = SessionMessage::parse(&value);
        let call = message.tool_calls().next().expect("the call");

        assert!(matches!(call.state, ToolState::Streaming));
        assert!(call.input().is_null());
        assert_eq!(call.outcome(), None);
        assert_eq!(message.chat_messages().len(), 1);
    }
}
