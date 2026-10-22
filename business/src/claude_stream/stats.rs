//! Statistics of a Claude Code chat for `getSessionStats` and the context meter, shaped like the
//! other engines' ([`SessionStats`], [`ContextUsage`]). Pure: no I/O.
//!
//! - **Tokens and cost** come from the newest `result` with usage: `modelUsage` holds the process's
//!   running totals per model (main loop, subagents, compaction), and `total_cost_usd` the
//!   API-equivalent cost so far (also on a subscription). Results without usage (an interrupted
//!   turn, a quiet run before any model call) change nothing. The totals cover this process only:
//!   a resumed session adds what its history used.
//! - **Context**, as Claude Code's `/context` shows it: the `get_context_usage` reply when asked
//!   (its `totalTokens` of `maxTokens`), else the newest model call of a turn (its input, cache and
//!   output tokens) against the context window the turn's result names for the session's model. A
//!   turn whose result names no window for that model hides the meter (never a stale one). A
//!   compaction sets it to the tokens left.
//! - **Counts** are the chat's messages, which the engine counts ([`MessageCounts`]).

use std::collections::BTreeMap;

use crate::claude_stream::wire::control::ContextUsageReply;
use crate::claude_stream::wire::result::{ModelUsage, ResultFrame};
use crate::claude_stream::wire::system::CompactMetadata;
use crate::model::{ContextUsage, SessionStats, TokenUsage};
use crate::opencode_server::stats::usage;

/// The chat's message counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MessageCounts {
    pub user: u32,
    pub assistant: u32,
    pub tool_calls: u32,
    /// Every message, tool results and `!command`s included.
    pub total: u32,
}

/// What a chat process used so far (see the module docs).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClaudeStats {
    tokens: TokenUsage,
    cost_usd: Option<f64>,
    context: Option<ContextUsage>,
    /// The context window of the session's model, once a result named it.
    window: Option<u64>,
}

impl ClaudeStats {
    /// A turn's `result`; `model` is the session's model (`system/init`), whose context window the
    /// meter uses.
    pub fn on_result(&mut self, result: &ResultFrame, model: Option<&str>) {
        if result.model_usage.is_empty() {
            return;
        }

        let (tokens, cost) = totals(&result.model_usage);
        self.tokens = tokens;
        self.cost_usd = result.total_cost_usd.filter(|cost| *cost > 0.0).or(cost);

        let known = window(&result.model_usage, model);

        // A turn without a model call (a quiet run) leaves the meter as it was.
        let Some(used) = last_call_tokens(result) else {
            self.window = known.or(self.window);
            return;
        };

        // The window of an earlier model (or none at all) would show a wrong meter: it hides.
        self.window = known;
        self.context = usage(used, known);
    }

    /// The `get_context_usage` reply.
    pub fn on_context_usage(&mut self, reply: &ContextUsageReply) {
        if let Some(context) = context_usage(reply) {
            self.window = Some(context.context_window);
            self.context = Some(context);
        }
    }

    /// A compaction: the context holds the tokens it left (unknown while the window is).
    pub fn on_compacted(&mut self, metadata: &CompactMetadata) {
        self.context = metadata.post_tokens.and_then(|tokens| usage(tokens, self.window));
    }

    pub fn tokens(&self) -> TokenUsage {
        self.tokens
    }

    pub fn cost_usd(&self) -> Option<f64> {
        self.cost_usd
    }

    /// The context meter, when known.
    pub fn context(&self) -> Option<&ContextUsage> {
        self.context.as_ref()
    }

    /// The statistics with the chat's message `counts`.
    pub fn session_stats(&self, counts: MessageCounts) -> SessionStats {
        SessionStats {
            user_messages: counts.user,
            assistant_messages: counts.assistant,
            tool_calls: counts.tool_calls,
            total_messages: counts.total,
            tokens: self.tokens,
            cost_usd: self.cost_usd,
            context_usage: self.context.clone(),
        }
    }
}

/// The tokens and cost of `modelUsage`, summed over its models.
pub fn totals(models: &BTreeMap<String, ModelUsage>) -> (TokenUsage, Option<f64>) {
    let mut tokens = TokenUsage::default();
    let mut cost = 0.0;

    for model in models.values() {
        tokens.input += model.input_tokens;
        tokens.output += model.output_tokens;
        tokens.cache_read += model.cache_read_input_tokens;
        tokens.cache_write += model.cache_creation_input_tokens;

        if model.cost_usd.is_finite() && model.cost_usd > 0.0 {
            cost += model.cost_usd;
        }
    }

    tokens.total = tokens.input + tokens.output + tokens.cache_read + tokens.cache_write;

    (tokens, (cost > 0.0).then_some(cost))
}

/// The context window of `model` in `modelUsage` (else of the only model it lists).
fn window(models: &BTreeMap<String, ModelUsage>, model: Option<&str>) -> Option<u64> {
    let named = model.and_then(|model| models.get(model));
    let only = (models.len() == 1).then(|| models.values().next()).flatten();

    named.or(only).and_then(|usage| usage.context_window)
}

/// The context after a turn: its newest model call's tokens against `window`; `None` for a turn
/// without a model call.
pub fn result_context(result: &ResultFrame, window: Option<u64>) -> Option<ContextUsage> {
    usage(last_call_tokens(result)?, window)
}

/// The tokens of the turn's newest model call; `None` for a turn without one.
fn last_call_tokens(result: &ResultFrame) -> Option<u64> {
    let last = result.usage.as_ref()?.iterations.last()?;

    Some(last.total())
}

/// The context `get_context_usage` reports; `None` without a window.
pub fn context_usage(reply: &ContextUsageReply) -> Option<ContextUsage> {
    if reply.max_tokens == 0 {
        return None;
    }

    usage(reply.total_tokens, Some(reply.max_tokens))
}
