//! The assistant's replies: one chat message per API message (`message.id`), streamed from the
//! `stream_event` frames and confirmed by the `assistant` frames.
//!
//! - `message_start` opens the message (`messageStart`); `content_block_start` adds a block (text,
//!   thinking, tool call or another kind) and the deltas stream into it with the block's position
//!   as content index. Thinking the API withheld streams nothing and ends as a redacted block.
//! - Claude Code also writes one `assistant` frame per finished block, with the block's full
//!   content. It confirms the streamed block of the same kind (only text past what streamed is
//!   sent, so nothing repeats; a rewritten text waits for `messageEnd`), starts the block's tool
//!   call, and names the message's last session entry (its `uuid`, the fork point). A message that
//!   never streamed (partial messages off) shows from its `assistant` frames alone.
//! - `message_delta` brings the stop reason and the final usage, `message_stop` ends the message
//!   (`messageEnd`, with every block). A turn's end closes what is still open (as `aborted` after
//!   an interrupt). A message that ended is never reported again.
//! - A reply's streamed text stops at [`MAX_STREAMED_BYTES`]: [`REPLY_CUT`] ends it, and what
//!   follows (deltas, confirmations) changes nothing.

use std::collections::HashSet;

use super::MAX_STREAMED_BYTES;
use super::tools::Tools;
use crate::claude_stream::wire::messages::{ApiUsage, AssistantFrame, ContentBlock, Delta};
use crate::model::{AgentEvent, ChatBlock, ChatMessage, MessageRole, TokenUsage};

/// The stop reason of a message an interrupt cut off.
const ABORTED: &str = "aborted";

/// What ends a reply whose streamed text reached [`MAX_STREAMED_BYTES`].
pub const REPLY_CUT: &str = "\n\n[The reply was cut here because it is too long to show.]";

/// Open replies (see the module docs).
#[derive(Default)]
pub(super) struct Replies {
    open: Vec<Reply>,
    done: HashSet<String>,
}

struct Reply {
    id: String,
    /// The session entry of its newest `assistant` frame.
    entry: Option<String>,
    blocks: Vec<Block>,
    model: Option<String>,
    stop_reason: Option<String>,
    usage: Option<ApiUsage>,
    aborted: bool,
    timestamp: Option<i64>,
    /// Bytes of text and thinking streamed so far.
    streamed: usize,
    /// Its text reached the bound: nothing more is added.
    cut: bool,
}

struct Block {
    /// The block's index in the API message, when it streamed.
    index: Option<u32>,
    kind: BlockKind,
    /// Its `assistant` frame arrived.
    confirmed: bool,
}

enum BlockKind {
    Text(String),
    Thinking(String),
    Tool(String),
    Other(String),
}

impl Replies {
    /// The newest open reply's id.
    pub(super) fn current(&self) -> Option<&str> {
        self.open.last().map(|reply| reply.id.as_str())
    }

    /// `message_start`: opens reply `id` of `model`.
    pub(super) fn started(&mut self, id: &str, model: Option<&str>, out: &mut Vec<AgentEvent>) {
        if let Some(reply) = self.reply(id, out) {
            reply.model = model.map(str::to_owned);
        }
    }

    /// `content_block_start` of reply `id`.
    pub(super) fn block_started(
        &mut self,
        id: &str,
        index: u32,
        block: &ContentBlock,
        tools: &mut Tools,
        out: &mut Vec<AgentEvent>,
    ) {
        let Some(reply) = self.reply(id, out) else {
            return;
        };

        let kind = match block {
            ContentBlock::Text { .. } => BlockKind::Text(String::new()),
            ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking => BlockKind::Thinking(String::new()),
            ContentBlock::ToolUse { id: call, name, .. } => {
                tools.named(call, name);
                BlockKind::Tool(call.clone())
            }
            other => BlockKind::Other(kind_name(other)),
        };

        reply.blocks.push(Block {
            index: Some(index),
            kind,
            confirmed: false,
        });

        let initial = match block {
            ContentBlock::Text { text } => Some((text.as_str(), true)),
            ContentBlock::Thinking { thinking } => Some((thinking.as_str(), false)),
            _ => None,
        };

        if let Some((text, is_text)) = initial {
            let position = reply.blocks.len() - 1;
            append(reply, position, text, is_text, out);
        }
    }

    /// `content_block_delta` of reply `id`.
    pub(super) fn delta(&mut self, id: &str, index: u32, delta: &Delta, tools: &mut Tools, out: &mut Vec<AgentEvent>) {
        let Some(reply) = self.open.iter_mut().find(|reply| reply.id == id) else {
            return;
        };

        let Some(position) = reply.blocks.iter().position(|block| block.index == Some(index)) else {
            return;
        };

        match (delta, &reply.blocks[position].kind) {
            (Delta::TextDelta { text }, BlockKind::Text(_)) => append(reply, position, text, true, out),
            (Delta::ThinkingDelta { thinking }, BlockKind::Thinking(_)) => {
                append(reply, position, thinking, false, out);
            }
            (Delta::InputJsonDelta { partial_json }, BlockKind::Tool(call)) => tools.input_delta(call, partial_json),
            _ => {}
        }
    }

    /// `content_block_stop` of reply `id`: a tool call's input is complete.
    pub(super) fn block_stopped(&mut self, id: &str, index: u32, tools: &mut Tools, out: &mut Vec<AgentEvent>) {
        let call = self
            .open
            .iter()
            .find(|reply| reply.id == id)
            .and_then(|reply| reply.blocks.iter().find(|block| block.index == Some(index)))
            .and_then(|block| match &block.kind {
                BlockKind::Tool(call) => Some(call.clone()),
                _ => None,
            });

        if let Some(call) = call {
            tools.input_ended(&call, out);
        }
    }

    /// An `assistant` frame (see the module docs).
    pub(super) fn assistant(&mut self, frame: &AssistantFrame, tools: &mut Tools, out: &mut Vec<AgentEvent>) {
        let Some(reply) = self.reply(&frame.message.id, out) else {
            return;
        };

        if reply.model.is_none() {
            reply.model.clone_from(&frame.message.model);
        }

        if frame.uuid.is_some() {
            reply.entry.clone_from(&frame.uuid);
        }

        if reply.timestamp.is_none() {
            reply.timestamp = frame.timestamp.as_deref().and_then(millis);
        }

        reply.aborted |= frame.aborted;

        for block in &frame.message.content {
            confirm(reply, block, tools, out);
        }
    }

    /// `message_delta` of reply `id`.
    pub(super) fn finishing(&mut self, id: &str, stop_reason: Option<&str>, usage: Option<ApiUsage>) {
        if let Some(reply) = self.open.iter_mut().find(|reply| reply.id == id) {
            if let Some(reason) = stop_reason {
                reply.stop_reason = Some(reason.to_owned());
            }

            if usage.is_some() {
                reply.usage = usage;
            }
        }
    }

    /// `message_stop` of reply `id`.
    pub(super) fn stopped(&mut self, id: &str, tools: &Tools, out: &mut Vec<AgentEvent>) {
        self.end(id, false, tools, out);
    }

    /// Ends whatever is still open when a turn ends (as aborted after an interrupt).
    pub(super) fn end_all(&mut self, aborted: bool, tools: &Tools, out: &mut Vec<AgentEvent>) {
        let open: Vec<String> = self.open.iter().map(|reply| reply.id.clone()).collect();

        for id in open {
            self.end(&id, aborted, tools, out);
        }
    }

    /// The open reply `id`, opened (with its `messageStart`) when new; `None` once it ended.
    fn reply(&mut self, id: &str, out: &mut Vec<AgentEvent>) -> Option<&mut Reply> {
        if id.is_empty() || self.done.contains(id) {
            return None;
        }

        let position = match self.open.iter().position(|reply| reply.id == id) {
            Some(position) => position,
            None => {
                out.push(AgentEvent::MessageStart {
                    message_id: id.to_owned(),
                    role: MessageRole::Assistant,
                });
                self.open.push(Reply {
                    id: id.to_owned(),
                    entry: None,
                    blocks: Vec::new(),
                    model: None,
                    stop_reason: None,
                    usage: None,
                    aborted: false,
                    timestamp: None,
                    streamed: 0,
                    cut: false,
                });
                self.open.len() - 1
            }
        };

        Some(&mut self.open[position])
    }

    fn end(&mut self, id: &str, aborted: bool, tools: &Tools, out: &mut Vec<AgentEvent>) {
        let Some(position) = self.open.iter().position(|reply| reply.id == id) else {
            return;
        };

        let reply = self.open.remove(position);
        self.done.insert(reply.id.clone());

        let blocks = reply
            .blocks
            .iter()
            .filter_map(|block| match &block.kind {
                BlockKind::Text(text) if text.is_empty() => None,
                BlockKind::Text(text) => Some(ChatBlock::Text { text: text.clone() }),
                BlockKind::Thinking(text) => Some(ChatBlock::Thinking {
                    text: text.clone(),
                    redacted: text.is_empty(),
                }),
                BlockKind::Tool(call) => tools.block(call),
                BlockKind::Other(kind) => Some(ChatBlock::Other { kind: kind.clone() }),
            })
            .collect();

        let stop_reason = if aborted || reply.aborted {
            Some(ABORTED.to_owned())
        } else {
            reply.stop_reason
        };

        let message = ChatMessage {
            id: Some(reply.entry.unwrap_or_else(|| reply.id.clone())),
            role: MessageRole::Assistant,
            blocks,
            model: reply.model,
            provider: None,
            stop_reason,
            error_message: None,
            usage: reply.usage.as_ref().map(token_usage),
            cost_usd: None,
            timestamp: reply.timestamp,
            tool_call_id: None,
            tool_name: None,
            is_error: None,
        };

        out.push(AgentEvent::MessageEnd {
            message_id: reply.id,
            message,
        });
    }
}

/// Confirms `block` of an `assistant` frame against what streamed into `reply`.
fn confirm(reply: &mut Reply, block: &ContentBlock, tools: &mut Tools, out: &mut Vec<AgentEvent>) {
    match block {
        ContentBlock::Text { text } => confirm_text(reply, text, true, out),
        ContentBlock::Thinking { thinking } => confirm_text(reply, thinking, false, out),
        ContentBlock::RedactedThinking => confirm_text(reply, "", false, out),
        ContentBlock::ToolUse { id, name, input } => {
            let streamed = reply
                .blocks
                .iter()
                .any(|known| matches!(&known.kind, BlockKind::Tool(call) if call == id));

            if !streamed {
                reply.blocks.push(unstreamed(BlockKind::Tool(id.clone())));
            }

            tools.called(id, name, input, out);
        }
        ContentBlock::Image { .. } | ContentBlock::Other { .. } => confirm_other(reply, &kind_name(block)),
        ContentBlock::ToolResult { .. } => {}
    }
}

/// Confirms a block of another kind: the first streamed one of `kind` waiting, else a new one.
fn confirm_other(reply: &mut Reply, kind: &str) {
    let waiting = reply
        .blocks
        .iter_mut()
        .find(|known| !known.confirmed && matches!(&known.kind, BlockKind::Other(other) if other == kind));

    match waiting {
        Some(block) => block.confirmed = true,
        None => reply.blocks.push(unstreamed(BlockKind::Other(kind.to_owned()))),
    }
}

/// Confirms a text (or, not `is_text`, thinking) block with its full `text`: the first streamed
/// one of its kind waiting gets what did not stream yet, else it is a new block.
fn confirm_text(reply: &mut Reply, text: &str, is_text: bool, out: &mut Vec<AgentEvent>) {
    if reply.cut {
        return;
    }

    let waiting = reply.blocks.iter().position(|known| {
        !known.confirmed
            && match known.kind {
                BlockKind::Text(_) => is_text,
                BlockKind::Thinking(_) => !is_text,
                _ => false,
            }
    });

    let position = match waiting {
        Some(position) => position,
        None => {
            let kind = if is_text {
                BlockKind::Text(String::new())
            } else {
                BlockKind::Thinking(String::new())
            };
            reply.blocks.push(unstreamed(kind));
            reply.blocks.len() - 1
        }
    };

    reply.blocks[position].confirmed = true;

    let streamed = match &reply.blocks[position].kind {
        BlockKind::Text(streamed) | BlockKind::Thinking(streamed) => streamed.clone(),
        _ => String::new(),
    };

    match text.strip_prefix(streamed.as_str()) {
        Some(rest) => append(reply, position, rest, is_text, out),
        None => {
            reply.streamed = reply.streamed - streamed.len() + text.len();
            set_text(&mut reply.blocks[position].kind, text.to_owned());
        }
    }
}

fn unstreamed(kind: BlockKind) -> Block {
    Block {
        index: None,
        kind,
        confirmed: true,
    }
}

/// Appends `text` to the text or thinking block at `position` and streams it; at the bound, the
/// part that fits and [`REPLY_CUT`].
fn append(reply: &mut Reply, position: usize, text: &str, is_text: bool, out: &mut Vec<AgentEvent>) {
    if text.is_empty() || reply.cut {
        return;
    }

    let room = MAX_STREAMED_BYTES.saturating_sub(reply.streamed);
    let mut delta = fitting(text, room).to_owned();
    reply.streamed += delta.len();

    if delta.len() < text.len() {
        reply.cut = true;
        delta.push_str(REPLY_CUT);
    }

    if let BlockKind::Text(streamed) | BlockKind::Thinking(streamed) = &mut reply.blocks[position].kind {
        streamed.push_str(&delta);
    }

    let message_id = reply.id.clone();
    let content_index = u32::try_from(position).unwrap_or(u32::MAX);

    out.push(if is_text {
        AgentEvent::TextDelta {
            message_id,
            content_index,
            delta,
        }
    } else {
        AgentEvent::ThinkingDelta {
            message_id,
            content_index,
            delta,
        }
    });
}

/// The longest start of `text` of at most `room` bytes that ends on a character boundary.
fn fitting(text: &str, room: usize) -> &str {
    if text.len() <= room {
        return text;
    }

    let mut end = room;
    while !text.is_char_boundary(end) {
        end -= 1;
    }

    &text[..end]
}

fn set_text(kind: &mut BlockKind, text: String) {
    if let BlockKind::Text(streamed) | BlockKind::Thinking(streamed) = kind {
        *streamed = text;
    }
}

fn kind_name(block: &ContentBlock) -> String {
    match block {
        ContentBlock::Other { kind } => kind.clone(),
        ContentBlock::Image { .. } => "image".to_owned(),
        _ => "other".to_owned(),
    }
}

/// An API call's usage as the chat's; cache writes count as cache writes.
pub(super) fn token_usage(usage: &ApiUsage) -> TokenUsage {
    TokenUsage {
        input: usage.input_tokens,
        output: usage.output_tokens,
        cache_read: usage.cache_read_input_tokens,
        cache_write: usage.cache_creation_input_tokens,
        total: usage.total(),
    }
}

/// `timestamp` (RFC 3339) in milliseconds since the epoch.
fn millis(timestamp: &str) -> Option<i64> {
    timestamp
        .parse::<jiff::Timestamp>()
        .ok()
        .map(|time| time.as_millisecond())
}

/// The text of an `assistant` frame's text blocks (a synthetic error message's text).
pub(super) fn frame_text(frame: &AssistantFrame) -> String {
    frame
        .message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
