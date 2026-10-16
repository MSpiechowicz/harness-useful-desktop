//! Running [`TranscriptStats`] totals and transcript paging, shared by the PI-family session files
//! and the OpenCode session export.

use std::ops::Range;

use crate::error::{BusinessError, Result};
use crate::model::{MAX_SESSION_PAGE, ModelTurns, TranscriptStats};

/// The message range of one transcript page out of `total` messages: `cursor` is `None` for the
/// first (oldest) page or the index of its first message (a previous page's `nextCursor`), `limit`
/// is clamped to `1..=`[`MAX_SESSION_PAGE`]. An unknown cursor fails with `InvalidInput`.
pub(crate) fn page_range(cursor: Option<&str>, limit: u32, total: usize) -> Result<Range<usize>> {
    let start = match cursor {
        None => 0,
        Some(raw) => raw
            .parse::<usize>()
            .ok()
            .filter(|start| *start <= total)
            .ok_or_else(|| BusinessError::InvalidInput("unknown transcript cursor".to_owned()))?,
    };

    let limit = limit.clamp(1, MAX_SESSION_PAGE) as usize;
    let end = start.saturating_add(limit).min(total);

    Ok(start..end)
}

/// Running totals of a transcript, fed message by message.
#[derive(Default)]
pub(crate) struct StatsTally {
    messages: u32,
    tool_calls: u32,
    context_tokens: Option<u64>,
    cost_usd: Option<f64>,
    models: Vec<ModelTurns>,
    started_at: Option<i64>,
    ended_at: Option<i64>,
}

impl StatsTally {
    /// Moves the transcript's time span to include `time` (Unix milliseconds), if known.
    pub(crate) fn note_time(&mut self, time: Option<i64>) {
        self.started_at = self.started_at.or(time);
        self.ended_at = time.or(self.ended_at);
    }

    /// Counts one user or assistant message.
    pub(crate) fn note_message(&mut self) {
        self.messages = self.messages.saturating_add(1);
    }

    /// Adds one assistant reply: its tool calls, model, cost and the context size it reports.
    pub(crate) fn note_reply(
        &mut self,
        tool_calls: usize,
        model: Option<&str>,
        cost_usd: Option<f64>,
        context_tokens: Option<u64>,
    ) {
        self.tool_calls = self
            .tool_calls
            .saturating_add(u32::try_from(tool_calls).unwrap_or(u32::MAX));

        if let Some(model) = model {
            match self.models.iter_mut().find(|turns| turns.model == model) {
                Some(turns) => turns.turns += 1,
                None => self.models.push(ModelTurns {
                    model: model.to_owned(),
                    turns: 1,
                }),
            }
        }

        if let Some(cost) = cost_usd {
            self.cost_usd = Some(self.cost_usd.unwrap_or(0.0) + cost);
        }

        // Aborted and failed replies report no usage; the context is the last one that does.
        if let Some(tokens) = context_tokens.filter(|tokens| *tokens > 0) {
            self.context_tokens = Some(tokens);
        }
    }

    /// The totals, models ordered by most turns first.
    pub(crate) fn finish(mut self) -> TranscriptStats {
        self.models.sort_by_key(|turns| std::cmp::Reverse(turns.turns));

        TranscriptStats {
            messages: self.messages,
            tool_calls: self.tool_calls,
            context_tokens: self.context_tokens,
            cost_usd: self.cost_usd,
            models: self.models,
            started_at: self.started_at,
            ended_at: self.ended_at,
        }
    }
}
