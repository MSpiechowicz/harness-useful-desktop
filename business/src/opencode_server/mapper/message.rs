//! One OpenCode message (`{info, parts}`, the shapes of `packages/schema/src/v1/session.ts`) as
//! chat messages: shared by the live chat ([`super::Mapper`]) and the CLI transcript of
//! `opencode export`.
//!
//! - A user message keeps its typed text (synthetic and ignored text is OpenCode's own context),
//!   image attachments (MIME type only), attached files as `@name` (a file mentioned in the text is
//!   already there) and subtasks as `@agent prompt`.
//! - An assistant message keeps text, reasoning and tool calls in order; each finished tool call
//!   gets a tool-result message with its redacted output or error.
//! - A compaction's summary (an assistant message marked `summary`) is a compaction-summary
//!   message of its text.
//! - Every other part (steps, snapshots, patches, retries, compaction markers) is skipped.

use serde_json::{Value, json};

use crate::model::{ChatBlock, ChatMessage, MessageRole, TokenUsage};
use crate::security::redact_text;

/// The chat messages of one `{info, parts}` record: a user message, or an assistant message
/// followed by the results of its finished tool calls; none when nothing in it is shown.
pub(crate) fn record_messages(record: &Value) -> Vec<ChatMessage> {
    let info = record.get("info").unwrap_or(&Value::Null);
    let parts = record
        .get("parts")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);

    match info.get("role").and_then(Value::as_str) {
        Some("user") => user_message(info, parts).into_iter().collect(),
        Some("assistant") if is_summary(info) => summary_message(info, parts).into_iter().collect(),
        Some("assistant") => assistant_message(info, parts)
            .map(|(message, results)| std::iter::once(message).chain(results).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// True for the summary a compaction wrote (an assistant message OpenCode marks `summary`).
pub(crate) fn is_summary(info: &Value) -> bool {
    info.get("summary").and_then(Value::as_bool) == Some(true)
}

/// A compaction's summary as a compaction-summary message of its text; `None` without text.
fn summary_message(info: &Value, parts: &[Value]) -> Option<ChatMessage> {
    let blocks: Vec<ChatBlock> = parts
        .iter()
        .filter(|part| part_type(part) == "text")
        .filter_map(text_block)
        .collect();

    if blocks.is_empty() {
        return None;
    }

    let mut message = blank_message(info, MessageRole::CompactionSummary);
    message.blocks = blocks;

    Some(message)
}

/// The user message of `info` with `parts`; `None` when nothing in it is shown.
pub(crate) fn user_message(info: &Value, parts: &[Value]) -> Option<ChatMessage> {
    let blocks: Vec<ChatBlock> = parts.iter().filter_map(user_block).collect();

    if blocks.is_empty() {
        return None;
    }

    let mut message = blank_message(info, MessageRole::User);
    message.blocks = blocks;

    Some(message)
}

/// The assistant message of `info` with `parts` and the tool-result messages of its finished tool
/// calls; `None` when it has neither content nor an error.
pub(crate) fn assistant_message(info: &Value, parts: &[Value]) -> Option<(ChatMessage, Vec<ChatMessage>)> {
    let mut message = blank_message(info, MessageRole::Assistant);
    let mut results = Vec::new();

    for part in parts {
        match part_type(part) {
            "text" => message.blocks.extend(text_block(part)),
            "reasoning" => message.blocks.push(reasoning_block(part)),
            "tool" => {
                let (call, result) = tool_call(part);
                message.blocks.push(call);
                results.extend(result);
            }
            _ => {}
        }
    }

    let error = info.get("error");
    message.error_message = error_text(info);

    if message.blocks.is_empty() && message.error_message.is_none() {
        return None;
    }

    message.model = text(info, "modelID");
    message.provider = text(info, "providerID");
    message.stop_reason = text(info, "finish").or_else(|| error.map(stop_reason_of));
    message.cost_usd = info.get("cost").and_then(Value::as_f64);
    message.usage = info.get("tokens").map(token_usage);

    Some((message, results))
}

/// The redacted error text of a failed assistant message.
pub(crate) fn error_text(info: &Value) -> Option<String> {
    info.pointer("/error/data/message")
        .and_then(Value::as_str)
        .map(redact_text)
}

/// The part's `type`.
pub(crate) fn part_type(part: &Value) -> &str {
    part.get("type").and_then(Value::as_str).unwrap_or_default()
}

/// A non-empty string field.
pub(crate) fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn number(value: &Value, pointer: &str) -> u64 {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .filter(|number| *number >= 0.0)
        .map_or(0, |number| number as u64)
}

/// A message of `role` with the id and creation time of `info` and no content yet.
pub(crate) fn blank_message(info: &Value, role: MessageRole) -> ChatMessage {
    ChatMessage {
        id: text(info, "id"),
        role,
        blocks: Vec::new(),
        model: None,
        provider: None,
        stop_reason: None,
        error_message: None,
        usage: None,
        cost_usd: None,
        timestamp: info.pointer("/time/created").and_then(Value::as_i64),
        tool_call_id: None,
        tool_name: None,
        is_error: None,
    }
}

/// A user-visible text part; synthetic and ignored text is context OpenCode added itself.
fn text_block(part: &Value) -> Option<ChatBlock> {
    let hidden = ["synthetic", "ignored"]
        .iter()
        .any(|key| part.get(key).and_then(Value::as_bool) == Some(true));

    if hidden {
        return None;
    }

    text(part, "text").map(|text| ChatBlock::Text { text })
}

/// Reasoning; OpenCode keeps an empty text when the provider encrypted it.
fn reasoning_block(part: &Value) -> ChatBlock {
    let text = text(part, "text").unwrap_or_default();
    let redacted = text.is_empty();

    ChatBlock::Thinking { text, redacted }
}

fn user_block(part: &Value) -> Option<ChatBlock> {
    match part_type(part) {
        "text" => text_block(part),
        "file" => file_block(part),
        "subtask" => {
            let agent = text(part, "agent").unwrap_or_default();
            let prompt = text(part, "prompt").or_else(|| text(part, "description"))?;

            Some(ChatBlock::Text {
                text: format!("@{agent} {prompt}"),
            })
        }
        _ => None,
    }
}

/// An attachment: images by MIME type, other files as `@name`. The URL (possibly a `data:` URL
/// holding the content) is never used, and a file mentioned in the prompt text is skipped.
fn file_block(part: &Value) -> Option<ChatBlock> {
    let mime = text(part, "mime").unwrap_or_default();

    if mime.starts_with("image/") {
        return Some(ChatBlock::Image { mime_type: mime });
    }

    if part.get("source").is_some() {
        return None;
    }

    text(part, "filename").map(|name| ChatBlock::Text {
        text: format!("@{name}"),
    })
}

/// The id a `tool` part's call goes by: its `callID`, else the part id.
pub(crate) fn tool_call_id(part: &Value) -> String {
    text(part, "callID").or_else(|| text(part, "id")).unwrap_or_default()
}

/// The redacted output of a finished `tool` part and whether it failed; `None` while it runs.
pub(crate) fn tool_outcome(state: &Value) -> Option<(String, bool)> {
    let (output, is_error) = match state.get("status").and_then(Value::as_str) {
        Some("completed") => (text(state, "output").unwrap_or_default(), false),
        Some("error") => (text(state, "error").unwrap_or_default(), true),
        _ => return None,
    };

    Some((redact_text(&output), is_error))
}

/// The tool call of a `tool` part, and its result message once the call finished.
fn tool_call(part: &Value) -> (ChatBlock, Option<ChatMessage>) {
    let id = tool_call_id(part);
    let name = text(part, "tool").unwrap_or_default();
    let state = part.get("state").unwrap_or(&Value::Null);

    let call = ChatBlock::ToolCall {
        id: id.clone(),
        name: name.clone(),
        args: state.get("input").cloned().unwrap_or_else(|| json!({})),
        intent: text(state, "title"),
    };

    let Some((output, is_error)) = tool_outcome(state) else {
        return (call, None);
    };

    let result = ChatMessage {
        id: text(part, "id"),
        role: MessageRole::ToolResult,
        blocks: vec![ChatBlock::Text { text: output }],
        model: None,
        provider: None,
        stop_reason: None,
        error_message: None,
        usage: None,
        cost_usd: None,
        timestamp: state.pointer("/time/end").and_then(Value::as_i64),
        tool_call_id: Some(id),
        tool_name: Some(name),
        is_error: Some(is_error),
    };

    (call, Some(result))
}

/// `tokens` of an assistant message; the total includes reasoning tokens.
fn token_usage(tokens: &Value) -> TokenUsage {
    let input = number(tokens, "/input");
    let output = number(tokens, "/output");
    let reasoning = number(tokens, "/reasoning");
    let cache_read = number(tokens, "/cache/read");
    let cache_write = number(tokens, "/cache/write");

    let summed = input + output + reasoning + cache_read + cache_write;
    let total = tokens
        .get("total")
        .and_then(Value::as_f64)
        .map_or(summed, |total| total.max(0.0) as u64);

    TokenUsage {
        input,
        output,
        cache_read,
        cache_write,
        total,
    }
}

/// True for the error OpenCode records when the user aborted the reply.
pub(crate) fn is_abort(error: &Value) -> bool {
    error.get("name").and_then(Value::as_str) == Some("MessageAbortedError")
}

/// The stop reason of a failed reply without `finish`.
fn stop_reason_of(error: &Value) -> String {
    if is_abort(error) { "aborted" } else { "error" }.to_owned()
}
