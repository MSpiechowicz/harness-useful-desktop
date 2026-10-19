//! Statistics of an OpenCode session from its messages (the `{info, parts}` records of
//! `GET /session/{id}/message`): `getSessionStats`, the context meter and the last reply. Pure: no
//! I/O.
//!
//! - **Counts** follow the chat's messages ([`record_messages`]): user and assistant messages
//!   with something to show, a tool call per tool part, and in the total also the tool results
//!   and compaction summaries.
//! - **Tokens and cost** add up every assistant message (OpenCode writes one per model step),
//!   compaction summaries included. Reasoning tokens count as output, and the total is the sum.
//! - **Context**, as OpenCode's own sidebar shows it: the input, output, reasoning and cache
//!   tokens of the newest assistant message with output, against its model's context window.
//!   After a compaction it is unknown until the next reply.

use serde_json::Value;

use super::mapper::{is_summary, record_messages, text};
use super::wire::ModelRef;
use crate::model::{ChatBlock, ContextUsage, MessageRole, SessionStats, TokenUsage};

/// What an assistant message tells about the context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMark {
    /// A compaction replaced the context: its size is unknown until the next reply.
    Compacted,
    /// The context holds `tokens` of `model`.
    Tokens {
        /// Input, output, reasoning and cache tokens of the message.
        tokens: u64,
        /// The model that wrote the message.
        model: Option<ModelRef>,
    },
}

/// The context mark of message `info`; `None` unless it is an assistant message with output.
pub fn context_mark(info: &Value) -> Option<ContextMark> {
    if info.get("role").and_then(Value::as_str) != Some("assistant") {
        return None;
    }

    let tokens = info.get("tokens")?;
    let count = |pointer: &str| whole(tokens, pointer);

    if count("/output") == 0 {
        return None;
    }

    if is_summary(info) {
        return Some(ContextMark::Compacted);
    }

    let model = text(info, "providerID")
        .zip(text(info, "modelID"))
        .map(|(provider, id)| ModelRef { provider, id });

    Some(ContextMark::Tokens {
        tokens: ["/input", "/output", "/reasoning", "/cache/read", "/cache/write"]
            .into_iter()
            .map(count)
            .sum(),
        model,
    })
}

/// `tokens` of a context window of `window` tokens; `None` without a window.
pub fn usage(tokens: u64, window: Option<u64>) -> Option<ContextUsage> {
    let window = window.filter(|window| *window > 0)?;

    Some(ContextUsage {
        tokens,
        context_window: window,
        percent: tokens as f64 * 100.0 / window as f64,
    })
}

/// The newest context mark of `records` (oldest first).
pub fn last_context(records: &[Value]) -> Option<ContextMark> {
    records
        .iter()
        .rev()
        .find_map(|record| record.get("info").and_then(context_mark))
}

/// The statistics of `records` (oldest first); `window` gives a model's context window.
pub fn session_stats(records: &[Value], window: &dyn Fn(&ModelRef) -> Option<u64>) -> SessionStats {
    let mut stats = SessionStats {
        user_messages: 0,
        assistant_messages: 0,
        tool_calls: 0,
        total_messages: 0,
        tokens: TokenUsage::default(),
        cost_usd: Some(0.0),
        context_usage: None,
    };

    for record in records {
        let info = record.get("info").unwrap_or(&Value::Null);

        if info.get("role").and_then(Value::as_str) == Some("assistant") {
            add_usage(&mut stats, info);
        }

        for message in record_messages(record) {
            count_message(&mut stats, message.role, &message.blocks);
        }
    }

    stats.context_usage = match last_context(records) {
        Some(ContextMark::Tokens { tokens, model }) => usage(tokens, model.as_ref().and_then(window)),
        _ => None,
    };

    stats
}

/// The non-negative whole number at `pointer` in `value`; 0 when absent.
fn whole(value: &Value, pointer: &str) -> u64 {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .filter(|number| *number > 0.0)
        .map_or(0, |number| number as u64)
}

fn add_usage(stats: &mut SessionStats, info: &Value) {
    let number = |pointer: &str| whole(info, pointer);

    let tokens = &mut stats.tokens;
    tokens.input += number("/tokens/input");
    tokens.output += number("/tokens/output") + number("/tokens/reasoning");
    tokens.cache_read += number("/tokens/cache/read");
    tokens.cache_write += number("/tokens/cache/write");
    tokens.total = tokens.input + tokens.output + tokens.cache_read + tokens.cache_write;

    let cost = info.get("cost").and_then(Value::as_f64).filter(|cost| *cost > 0.0);
    stats.cost_usd = Some(stats.cost_usd.unwrap_or(0.0) + cost.unwrap_or(0.0));
}

fn count_message(stats: &mut SessionStats, role: MessageRole, blocks: &[ChatBlock]) {
    stats.total_messages = stats.total_messages.saturating_add(1);

    match role {
        MessageRole::User => stats.user_messages = stats.user_messages.saturating_add(1),
        MessageRole::Assistant => {
            stats.assistant_messages = stats.assistant_messages.saturating_add(1);

            let calls = blocks
                .iter()
                .filter(|block| matches!(block, ChatBlock::ToolCall { .. }))
                .count();
            stats.tool_calls = stats
                .tool_calls
                .saturating_add(u32::try_from(calls).unwrap_or(u32::MAX));
        }
        _ => {}
    }
}

/// The text of the newest assistant reply with text (its text parts, OpenCode's own context
/// left out); `None` before the first one.
pub fn last_reply_text(records: &[Value]) -> Option<String> {
    records.iter().rev().find_map(|record| {
        let message = record_messages(record).into_iter().next()?;

        if message.role != MessageRole::Assistant {
            return None;
        }

        let texts: Vec<String> = message
            .blocks
            .into_iter()
            .filter_map(|block| match block {
                ChatBlock::Text { text } => Some(text),
                _ => None,
            })
            .collect();

        (!texts.is_empty()).then(|| texts.join("\n\n"))
    })
}
