//! Statistics of a Codex chat for `getSessionStats` and the context meter, from
//! `thread/tokenUsage/updated` ([`ThreadTokenUsage`]). Pure: no I/O.
//!
//! - **Tokens** are the thread's totals as Codex counts them (a resumed thread's history
//!   included): input without the cached part, the cached part as cache reads, cache writes,
//!   output (reasoning included). Codex reports no cost.
//! - **Context**: the newest model call's tokens of the window Codex names for the model
//!   (`modelContextWindow`); no window, no meter. After a compaction the next update shows what
//!   is left.
//! - **Counts** are the chat's messages, which the engine counts ([`MessageCounts`]).

pub use crate::claude_stream::stats::MessageCounts;
use crate::codex_rpc::wire::notifications::{ThreadTokenUsage, TokenBreakdown};
use crate::model::{ContextUsage, SessionStats, TokenUsage};
use crate::opencode_server::stats::usage;

/// What a thread used so far (see the module docs).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CodexStats {
    tokens: TokenUsage,
    context: Option<ContextUsage>,
}

impl CodexStats {
    /// A `thread/tokenUsage/updated`.
    pub fn on_token_usage(&mut self, usage_update: &ThreadTokenUsage) {
        self.tokens = token_usage(&usage_update.total);
        self.context = usage(usage_update.last.total_tokens, usage_update.model_context_window);
    }

    pub fn tokens(&self) -> TokenUsage {
        self.tokens
    }

    /// The context meter, when known.
    pub fn context(&self) -> Option<&ContextUsage> {
        self.context.as_ref()
    }

    /// The statistics with the chat's message `counts`.
    pub fn session_stats(&self, counts: MessageCounts) -> SessionStats {
        SessionStats {
            tokens: self.tokens,
            context_usage: self.context.clone(),
            cost_usd: None,
            total_messages: counts.total,
            user_messages: counts.user,
            assistant_messages: counts.assistant,
            tool_calls: counts.tool_calls,
        }
    }
}

/// Codex's token counts as the chat's.
pub fn token_usage(counts: &TokenBreakdown) -> TokenUsage {
    let cache_read = counts.cached_input_tokens.min(counts.input_tokens);

    TokenUsage {
        input: counts.input_tokens - cache_read,
        output: counts.output_tokens,
        cache_read,
        cache_write: counts.cache_write_input_tokens,
        total: counts.total_tokens.max(counts.input_tokens + counts.output_tokens),
    }
}
