//! Maps PI-family RPC payloads onto the chat DTOs, field by field.
//!
//! Nothing is copied wholesale: model descriptors lose `headers`, `baseUrl`, `compat`, `api` and
//! every provider payload (they can hold credentials), images keep only their MIME type, tool
//! `details` are dropped, and session file paths are never exposed.

use serde_json::{Value, json};

use crate::model::{
    ChatBlock, ChatMessage, ContextUsage, MessageRole, ModelOption, SessionState, SessionStats, TokenUsage,
};

/// A string field.
pub(crate) fn string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// `text` without terminal escape sequences (colors, cursor moves, OSC links): extensions format
/// for the TUI, the desktop shows plain text.
pub(crate) fn plain_text(text: &str) -> String {
    const ESC: char = '\u{1b}';
    const BEL: char = '\u{7}';

    let mut plain = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(char) = chars.next() {
        if char != ESC {
            plain.push(char);
            continue;
        }

        match chars.next() {
            // CSI: parameters and intermediates up to a final byte in `@`..=`~`.
            Some('[') => {
                for char in chars.by_ref() {
                    if ('@'..='~').contains(&char) {
                        break;
                    }
                }
            }
            // OSC: up to BEL or the ESC `\` string terminator.
            Some(']') => {
                while let Some(char) = chars.next() {
                    if char == BEL || (char == ESC && chars.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            // Any other escape: intermediates in ` `..=`/` (as in the charset switch `ESC ( B`), then one final.
            Some(' '..='/') => while chars.next().is_some_and(|char| (' '..='/').contains(&char)) {},
            _ => {}
        }
    }

    plain
}

/// A non-negative integer field (floats are truncated).
pub(crate) fn unsigned(value: &Value, key: &str) -> Option<u64> {
    let field = value.get(key)?;

    field.as_u64().or_else(|| {
        field
            .as_f64()
            .filter(|number| *number >= 0.0)
            .map(|number| number as u64)
    })
}

/// A `u32` field, saturating.
pub(crate) fn count(value: &Value, key: &str) -> u32 {
    unsigned(value, key).map_or(0, |number| u32::try_from(number).unwrap_or(u32::MAX))
}

/// A boolean field, `false` when absent.
pub(crate) fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// The strings of an array field.
pub(crate) fn strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

/// A model descriptor reduced to display fields; `None` without `provider` and `id`.
pub fn model_option(value: &Value) -> Option<ModelOption> {
    Some(ModelOption {
        provider: string(value, "provider")?,
        id: string(value, "id")?,
        name: string(value, "name"),
        reasoning: flag(value, "reasoning"),
        context_window: unsigned(value, "contextWindow"),
        max_tokens: unsigned(value, "maxTokens"),
        input: strings(value, "input"),
    })
}

/// Context-window usage; `None` while the token count is unknown (e.g. right after compaction).
pub fn context_usage(value: Option<&Value>) -> Option<ContextUsage> {
    let value = value?;
    let tokens = unsigned(value, "tokens")?;
    let context_window = unsigned(value, "contextWindow")?;

    let percent = value
        .get("percent")
        .and_then(Value::as_f64)
        .unwrap_or_else(|| percent_of(tokens, context_window));

    Some(ContextUsage {
        tokens,
        context_window,
        percent,
    })
}

fn percent_of(tokens: u64, window: u64) -> f64 {
    if window == 0 {
        return 0.0;
    }

    tokens as f64 * 100.0 / window as f64
}

/// `get_state` data. OMP reports `queuedMessageCount`, PI `pendingMessageCount`.
pub fn session_state(value: &Value) -> SessionState {
    let queued = unsigned(value, "queuedMessageCount").or_else(|| unsigned(value, "pendingMessageCount"));

    SessionState {
        session_id: string(value, "sessionId").unwrap_or_default(),
        session_name: string(value, "sessionName"),
        model: value.get("model").and_then(model_option),
        thinking_level: string(value, "thinkingLevel"),
        agent: None,
        is_streaming: flag(value, "isStreaming"),
        is_compacting: flag(value, "isCompacting"),
        message_count: count(value, "messageCount"),
        queued_count: queued.map_or(0, |number| u32::try_from(number).unwrap_or(u32::MAX)),
        context_usage: context_usage(value.get("contextUsage")),
    }
}

/// `get_session_stats` data.
pub fn session_stats(value: &Value) -> SessionStats {
    let tokens = value.get("tokens").cloned().unwrap_or(Value::Null);

    SessionStats {
        user_messages: count(value, "userMessages"),
        assistant_messages: count(value, "assistantMessages"),
        tool_calls: count(value, "toolCalls"),
        total_messages: count(value, "totalMessages"),
        tokens: token_usage(&tokens, "total"),
        cost_usd: value.get("cost").and_then(Value::as_f64),
        context_usage: context_usage(value.get("contextUsage")),
    }
}

/// Token counts; `total_key` names the total (`total` in stats, `totalTokens` in message usage).
fn token_usage(value: &Value, total_key: &str) -> TokenUsage {
    let input = unsigned(value, "input").unwrap_or(0);
    let output = unsigned(value, "output").unwrap_or(0);
    let cache_read = unsigned(value, "cacheRead").unwrap_or(0);
    let cache_write = unsigned(value, "cacheWrite").unwrap_or(0);

    TokenUsage {
        input,
        output,
        cache_read,
        cache_write,
        total: unsigned(value, total_key).unwrap_or(input + output + cache_read + cache_write),
    }
}

/// The role of a message (`role` of an `AgentMessage`).
pub fn message_role(value: &Value) -> MessageRole {
    match value.get("role").and_then(Value::as_str).unwrap_or_default() {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "toolResult" => MessageRole::ToolResult,
        "bashExecution" => MessageRole::Bash,
        "custom" | "hookMessage" => MessageRole::Custom,
        "system" | "developer" => MessageRole::System,
        "branchSummary" => MessageRole::BranchSummary,
        "compactionSummary" => MessageRole::CompactionSummary,
        _ => MessageRole::Other,
    }
}

/// A message the harness keeps out of its own chat view: a `custom` message with `display: false`,
/// such as OMP's eval-state reminder, which only the model reads.
pub fn hidden_message(value: &Value) -> bool {
    message_role(value) == MessageRole::Custom && value.get("display").and_then(Value::as_bool) == Some(false)
}

/// One content block.
pub fn chat_block(value: &Value) -> ChatBlock {
    let kind = value.get("type").and_then(Value::as_str).unwrap_or_default();

    match kind {
        "text" => ChatBlock::Text {
            text: string(value, "text").unwrap_or_default(),
        },
        "thinking" => ChatBlock::Thinking {
            text: string(value, "thinking").unwrap_or_default(),
            redacted: flag(value, "redacted"),
        },
        "redactedThinking" => ChatBlock::Thinking {
            text: String::new(),
            redacted: true,
        },
        "toolCall" => ChatBlock::ToolCall {
            id: string(value, "id").unwrap_or_default(),
            name: string(value, "name").unwrap_or_default(),
            args: value.get("arguments").cloned().unwrap_or_else(|| json!({})),
            intent: string(value, "intent"),
        },
        "image" => ChatBlock::Image {
            mime_type: string(value, "mimeType").unwrap_or_default(),
        },
        other => ChatBlock::Other { kind: other.to_owned() },
    }
}

/// Message content: plain text or an array of blocks.
pub fn content_blocks(content: Option<&Value>) -> Vec<ChatBlock> {
    match content {
        Some(Value::String(text)) => vec![ChatBlock::Text { text: text.clone() }],
        Some(Value::Array(blocks)) => blocks.iter().map(chat_block).collect(),
        _ => Vec::new(),
    }
}

fn text_block(text: String) -> ChatBlock {
    ChatBlock::Text { text }
}

fn timestamp(value: &Value) -> Option<i64> {
    let field = value.get("timestamp")?;

    field.as_i64().or_else(|| field.as_f64().map(|number| number as i64))
}

/// A normalized `AgentMessage`.
pub fn chat_message(value: &Value) -> ChatMessage {
    let role = message_role(value);
    let raw_role = value.get("role").and_then(Value::as_str).unwrap_or_default();

    let mut message = ChatMessage {
        id: None,
        role,
        blocks: Vec::new(),
        model: None,
        provider: None,
        stop_reason: None,
        error_message: None,
        usage: None,
        cost_usd: None,
        timestamp: timestamp(value),
        tool_call_id: None,
        tool_name: None,
        is_error: None,
    };

    match role {
        MessageRole::Assistant => {
            let usage = value.get("usage");

            message.blocks = content_blocks(value.get("content"));
            message.model = string(value, "model");
            message.provider = string(value, "provider");
            message.stop_reason = string(value, "stopReason");
            message.error_message = string(value, "errorMessage");
            message.usage = usage.map(|usage| token_usage(usage, "totalTokens"));
            message.cost_usd = usage
                .and_then(|usage| usage.pointer("/cost/total"))
                .and_then(Value::as_f64);
        }
        MessageRole::ToolResult => {
            message.blocks = content_blocks(value.get("content"));
            message.tool_call_id = string(value, "toolCallId");
            message.tool_name = string(value, "toolName");
            message.is_error = Some(flag(value, "isError"));
        }
        MessageRole::Bash => {
            let command = string(value, "command").unwrap_or_default();
            let output = string(value, "output").unwrap_or_default();
            message.blocks = vec![text_block(format!("$ {command}\n{output}"))];
        }
        MessageRole::BranchSummary | MessageRole::CompactionSummary => {
            message.blocks = string(value, "summary").map(text_block).into_iter().collect();
        }
        MessageRole::User | MessageRole::Custom | MessageRole::System => {
            message.blocks = content_blocks(value.get("content"));
        }
        MessageRole::Other => {
            message.blocks = vec![ChatBlock::Other {
                kind: raw_role.to_owned(),
            }];
        }
    }

    message
}

/// A tool result (or partial result) as `{ "content": [ChatBlock…] }`: text and image blocks are
/// normalized like message content, `details` and every other field are dropped. A bare string
/// becomes one text block.
pub fn tool_result(value: Option<&Value>) -> Value {
    let blocks = match value {
        Some(Value::String(text)) => vec![text_block(text.clone())],
        Some(result) => content_blocks(result.get("content")),
        None => Vec::new(),
    };

    json!({ "content": blocks })
}

#[cfg(test)]
mod tests {
    use super::plain_text;

    #[test]
    fn plain_text_drops_terminal_escapes() {
        assert_eq!(
            plain_text("\u{1b}[38;5;215mUsage Dashboard\u{1b}[39m ready"),
            "Usage Dashboard ready"
        );
        assert_eq!(
            plain_text("\u{1b}]8;;https://example.com\u{7}link\u{1b}]8;;\u{1b}\\ done"),
            "link done"
        );
        assert_eq!(plain_text("\u{1b}(Bplain \u{1b}[1;2Htext"), "plain text");
        assert_eq!(plain_text("no escapes: 50% [ok]"), "no escapes: 50% [ok]");
    }
}
