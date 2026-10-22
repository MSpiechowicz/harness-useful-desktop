//! Agent messages and reasoning as assistant messages, one per item.
//!
//! - An agent message opens (`messageStart`) at its `item/started` or first delta, streams its
//!   deltas as text, and ends (`messageEnd`) at its `item/completed`, whose full text confirms what
//!   streamed (only the missing rest is sent; a rewritten text waits for `messageEnd`).
//! - Reasoning opens at its first summary or text delta and streams as thinking, a new summary
//!   part after a blank line. At its `item/completed` the summary (else the raw text) confirms it;
//!   reasoning that never had text shows nothing.
//! - A message's `id` is its turn's (a fork there keeps the turn), its stop reason the message's
//!   phase, or `aborted` when an interrupt ended it. A turn's end ends what is still open; an item
//!   that ended is never reported again.
//! - Streamed text stops at [`MAX_STREAMED_BYTES`]: [`REPLY_CUT`] ends it and nothing more is added.

use std::collections::HashSet;

use super::MAX_STREAMED_BYTES;
use crate::claude_stream::mapper::REPLY_CUT;
use crate::codex_rpc::wire::items::{AgentMessageItem, MessagePhase, ReasoningItem};
use crate::model::{AgentEvent, ChatBlock, ChatMessage, MessageRole};

/// The stop reason of a message an interrupt cut off.
const ABORTED: &str = "aborted";

/// What separates two summary parts of reasoning.
const PART_BREAK: &str = "\n\n";

/// Open replies (see the module docs).
#[derive(Default)]
pub(super) struct Replies {
    open: Vec<Reply>,
    /// Items of the running turn that ended.
    done: HashSet<String>,
}

struct Reply {
    id: String,
    turn_id: String,
    thinking: bool,
    text: String,
    /// The summary part streaming now.
    part: u32,
    /// A new summary part began: the next delta starts after a break.
    part_break: bool,
    /// The text reached the bound: nothing more is added.
    cut: bool,
}

impl Replies {
    /// `item/started` of an agent message.
    pub(super) fn message_started(&mut self, item: &AgentMessageItem, turn_id: &str, out: &mut Vec<AgentEvent>) {
        if let Some(reply) = self.reply(&item.id, turn_id, false, out) {
            append(reply, &item.text, out);
        }
    }

    /// `item/agentMessage/delta`.
    pub(super) fn text_delta(&mut self, item_id: &str, turn_id: &str, delta: &str, out: &mut Vec<AgentEvent>) {
        if let Some(reply) = self.reply(item_id, turn_id, false, out) {
            append(reply, delta, out);
        }
    }

    /// `item/completed` of an agent message.
    pub(super) fn message_completed(
        &mut self,
        item: &AgentMessageItem,
        turn_id: &str,
        at: Option<i64>,
        out: &mut Vec<AgentEvent>,
    ) {
        let Some(reply) = self.reply(&item.id, turn_id, false, out) else {
            return;
        };

        confirm(reply, &item.text, out);
        let stop_reason = item.phase.map(MessagePhase::wire);
        self.end(&item.id, stop_reason, at, out);
    }

    /// `item/reasoning/summaryPartAdded`, or the part of a summary delta.
    pub(super) fn thinking_part(&mut self, item_id: &str, part: u32) {
        let Some(reply) = self.open.iter_mut().find(|reply| reply.id == item_id) else {
            return;
        };

        if part > reply.part {
            reply.part = part;
            reply.part_break = true;
        }
    }

    /// A summary or raw text delta of reasoning.
    pub(super) fn thinking_delta(&mut self, item_id: &str, turn_id: &str, delta: &str, out: &mut Vec<AgentEvent>) {
        let Some(reply) = self.reply(item_id, turn_id, true, out) else {
            return;
        };

        if std::mem::take(&mut reply.part_break) && !reply.text.is_empty() && !delta.is_empty() {
            append(reply, PART_BREAK, out);
        }

        append(reply, delta, out);
    }

    /// `item/completed` of reasoning.
    pub(super) fn reasoning_completed(
        &mut self,
        item: &ReasoningItem,
        turn_id: &str,
        at: Option<i64>,
        out: &mut Vec<AgentEvent>,
    ) {
        let text = if item.summary.is_empty() {
            item.content.join("\n")
        } else {
            item.summary.join(PART_BREAK)
        };

        let streamed = self.open.iter().any(|reply| reply.id == item.id);

        if !streamed && text.trim().is_empty() {
            return;
        }

        if let Some(reply) = self.reply(&item.id, turn_id, true, out) {
            confirm(reply, &text, out);
            self.end(&item.id, None, at, out);
        }
    }

    /// Ends whatever is still open when a turn ends (as aborted after an interrupt) and forgets
    /// the turn's items.
    pub(super) fn end_all(&mut self, aborted: bool, out: &mut Vec<AgentEvent>) {
        let open: Vec<String> = self.open.iter().map(|reply| reply.id.clone()).collect();
        let stop_reason = aborted.then_some(ABORTED);

        for id in open {
            self.end(&id, stop_reason, None, out);
        }

        self.done.clear();
    }

    /// The open reply `id`, opened (with its `messageStart`) when new; `None` once it ended.
    fn reply(&mut self, id: &str, turn_id: &str, thinking: bool, out: &mut Vec<AgentEvent>) -> Option<&mut Reply> {
        if id.is_empty() || self.done.contains(id) {
            return None;
        }

        if !self.open.iter().any(|reply| reply.id == id) {
            out.push(AgentEvent::MessageStart {
                message_id: id.to_owned(),
                role: MessageRole::Assistant,
            });
            self.open.push(Reply::new(id, turn_id, thinking));
        }

        self.open.iter_mut().find(|reply| reply.id == id)
    }

    fn end(&mut self, id: &str, stop_reason: Option<&str>, at: Option<i64>, out: &mut Vec<AgentEvent>) {
        let Some(position) = self.open.iter().position(|reply| reply.id == id) else {
            return;
        };

        let Reply {
            id,
            turn_id,
            thinking,
            text,
            ..
        } = self.open.remove(position);
        self.done.insert(id.clone());

        let blocks = if thinking {
            vec![ChatBlock::Thinking { text, redacted: false }]
        } else if text.is_empty() {
            Vec::new()
        } else {
            vec![ChatBlock::Text { text }]
        };

        let message = ChatMessage {
            id: Some(turn_id).filter(|turn| !turn.is_empty()),
            role: MessageRole::Assistant,
            blocks,
            timestamp: at,
            stop_reason: stop_reason.map(str::to_owned),
            model: None,
            provider: None,
            error_message: None,
            usage: None,
            cost_usd: None,
            tool_call_id: None,
            tool_name: None,
            is_error: None,
        };

        out.push(AgentEvent::MessageEnd {
            message_id: id,
            message,
        });
    }
}

impl Reply {
    fn new(id: &str, turn_id: &str, thinking: bool) -> Self {
        Self {
            id: id.to_owned(),
            turn_id: turn_id.to_owned(),
            thinking,
            text: String::new(),
            part: 0,
            part_break: false,
            cut: false,
        }
    }
}

/// Confirms `reply` with its full `text`: what did not stream yet streams, a rewritten text
/// replaces what streamed (shown at `messageEnd`).
fn confirm(reply: &mut Reply, text: &str, out: &mut Vec<AgentEvent>) {
    if reply.cut {
        return;
    }

    match text.strip_prefix(reply.text.as_str()) {
        Some(rest) => append(reply, rest, out),
        None => {
            reply.text.clear();
            reply.text.push_str(within(text, MAX_STREAMED_BYTES));
        }
    }
}

/// Streams `text` into `reply`; at the bound, the part that fits and [`REPLY_CUT`].
fn append(reply: &mut Reply, text: &str, out: &mut Vec<AgentEvent>) {
    if text.is_empty() || reply.cut {
        return;
    }

    let room = MAX_STREAMED_BYTES.saturating_sub(reply.text.len());
    let mut delta = within(text, room).to_owned();

    if delta.len() < text.len() {
        reply.cut = true;
        delta.push_str(REPLY_CUT);
    }

    reply.text.push_str(&delta);

    let message_id = reply.id.clone();
    out.push(if reply.thinking {
        AgentEvent::ThinkingDelta {
            message_id,
            content_index: 0,
            delta,
        }
    } else {
        AgentEvent::TextDelta {
            message_id,
            content_index: 0,
            delta,
        }
    });
}

/// The longest start of `text` of at most `limit` bytes, on a character boundary.
fn within(text: &str, limit: usize) -> &str {
    let end = (0..=limit.min(text.len()))
        .rev()
        .find(|end| text.is_char_boundary(*end))
        .unwrap_or_default();

    &text[..end]
}
