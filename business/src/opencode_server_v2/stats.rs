//! Statistics of an OpenCode 2 session for `getSessionStats`, the context meter and the last reply,
//! shaped like the OpenCode 1 ones ([`crate::opencode_server::stats`]). Pure: no I/O.
//!
//! - **Counts** follow the chat messages ([`history`]): user and assistant messages with something
//!   to show, a tool call per tool item, and in the total also the tool results, `!command`s and
//!   compaction summaries.
//! - **Tokens and cost** are the session's running totals (`Session.cost`, `Session.tokens`, which
//!   `session.usage.updated` reports as they grow): every model step, compactions included.
//!   Reasoning tokens count as output, and the total is the sum.
//! - **Context**, as OpenCode's own sidebar shows it: the input, output, reasoning and cache tokens
//!   of the newest reply with output, against its model's context window. After a newer compaction
//!   it is unknown until the next reply.

use super::mapper::{ContextNote, history};
use super::messages::{AssistantMessage, SessionMessage};
use super::wire::{ModelRef, Tokens};
use crate::model::{ChatBlock, MessageRole, SessionStats, TokenUsage};
use crate::opencode_server::stats::usage;

/// The statistics of a session with the totals `cost` and `tokens` and `messages` (oldest first);
/// `window` gives a model's context window.
pub fn session_stats(
    cost: f64,
    tokens: &Tokens,
    messages: &[SessionMessage],
    window: &dyn Fn(&ModelRef) -> Option<u64>,
) -> SessionStats {
    let mut stats = SessionStats {
        user_messages: 0,
        assistant_messages: 0,
        tool_calls: 0,
        total_messages: 0,
        tokens: token_usage(tokens),
        cost_usd: Some(cost.max(0.0)),
        context_usage: None,
    };

    for message in history(messages) {
        count_message(&mut stats, message.role, &message.blocks);
    }

    stats.context_usage = match last_context(messages) {
        Some(ContextNote::Tokens { tokens, model }) => usage(tokens, model.as_ref().and_then(window)),
        _ => None,
    };

    stats
}

/// The session totals `tokens` as the chat's usage: reasoning counts as output.
pub fn token_usage(tokens: &Tokens) -> TokenUsage {
    let input = tokens.input;
    let output = tokens.output + tokens.reasoning;
    let cache_read = tokens.cache.read;
    let cache_write = tokens.cache.write;

    TokenUsage {
        input,
        output,
        cache_read,
        cache_write,
        total: input + output + cache_read + cache_write,
    }
}

/// What the newest reply with output (or a newer compaction) of `messages` (oldest first) tells
/// about the context's size.
pub fn last_context(messages: &[SessionMessage]) -> Option<ContextNote> {
    messages.iter().rev().find_map(|message| match message {
        SessionMessage::Compaction(_) => Some(ContextNote::Compacted),
        SessionMessage::Assistant(reply) => reply_context(reply),
        _ => None,
    })
}

/// The context `reply` reports; `None` without output.
fn reply_context(reply: &AssistantMessage) -> Option<ContextNote> {
    let tokens = reply.tokens.filter(|tokens| tokens.output > 0)?;
    let model = reply.model.as_ref().map(|model| ModelRef {
        id: model.id.clone(),
        provider_id: model.provider_id.clone(),
        variant: None,
    });

    Some(ContextNote::Tokens {
        tokens: tokens.usage().total,
        model,
    })
}

/// The text of the newest reply with text (its text items joined) in `messages` (oldest first);
/// `None` before the first one.
pub fn last_reply_text(messages: &[SessionMessage]) -> Option<String> {
    messages.iter().rev().find_map(|message| {
        let SessionMessage::Assistant(_) = message else {
            return None;
        };

        let reply = message.chat_messages().into_iter().next()?;
        let texts: Vec<String> = reply
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
