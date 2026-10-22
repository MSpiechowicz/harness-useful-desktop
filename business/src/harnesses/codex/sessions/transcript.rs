//! A saved thread (`thread/turns/list` and `thread/items/list`, oldest first) as the transcript the
//! Sessions view shows, read the way the live chat shows the same items:
//!
//! - A `userMessage` is a user message (its text and images); a steer is a second one in the turn.
//! - Agent messages and reasoning with text join one assistant message with tool calls after them,
//!   as cards ([`saved_card`]: commands, file changes, MCP calls, web searches) whose results
//!   follow the message. Text after a tool call starts the next assistant message. Reasoning
//!   without text (Codex's default) shows nothing.
//! - A compaction is a compaction marker; item types the app does not show are skipped.
//! - A failed turn ends with its error on its last reply, an interrupted one as `aborted`.
//! - Every message's `id` is its turn's: a fork from it keeps that turn (`thread/fork{lastTurnId}`).
//! - The touched files are the project files of the file changes that applied.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use crate::codex_rpc::mapper::saved_card;
use crate::codex_rpc::wire::items::{ThreadItem, Turn, TurnStatus, UserInput};
use crate::codex_rpc::wire::results::ItemEntry;
use crate::model::{ChatBlock, ChatMessage, MessageRole, TranscriptStats};
use crate::pi_family::sessions::StatsTally;

/// The image type shown for an image whose type the thread does not name.
const ANY_IMAGE: &str = "image/*";

/// A read thread.
#[derive(Debug)]
pub(super) struct Transcript {
    /// Chat messages in transcript order.
    pub messages: Vec<ChatMessage>,
    /// Totals of the whole transcript.
    pub stats: TranscriptStats,
    /// Project-relative files the file changes touched, sorted.
    pub touched: Vec<String>,
    /// The thread's turn ids, oldest first: the points a fork can keep.
    pub turns: Vec<String>,
}

/// What the thread ran on, for its replies.
#[derive(Debug, Clone, Copy)]
pub(super) struct Origin<'a> {
    pub model: Option<&'a str>,
    pub provider: Option<&'a str>,
}

/// The transcript of a thread with `turns` and `items` in `project` (see the module docs).
pub(super) fn transcript(turns: &[Turn], items: &[ItemEntry], origin: Origin<'_>, project: &Path) -> Transcript {
    let by_id: HashMap<&str, &Turn> = turns.iter().map(|turn| (turn.id.as_str(), turn)).collect();
    let mut order: Vec<String> = turns.iter().map(|turn| turn.id.clone()).collect();

    let mut builder = Builder {
        project,
        origin,
        messages: Vec::new(),
        tally: StatsTally::default(),
        touched: BTreeSet::new(),
        reply: None,
        results: Vec::new(),
        turn: None,
    };

    for entry in items {
        if builder.turn.as_ref().map(|(id, _)| id.as_str()) != Some(entry.turn_id.as_str()) {
            builder.end_turn();
            builder.turn = Some((entry.turn_id.clone(), by_id.get(entry.turn_id.as_str()).copied()));

            if !by_id.contains_key(entry.turn_id.as_str()) && !order.contains(&entry.turn_id) {
                order.push(entry.turn_id.clone());
            }
        }

        builder.add(&entry.item);
    }

    builder.end_turn();

    Transcript {
        messages: builder.messages,
        stats: builder.tally.finish(),
        touched: builder.touched.into_iter().collect(),
        turns: order,
    }
}

/// Collects the transcript of one thread.
struct Builder<'a> {
    project: &'a Path,
    origin: Origin<'a>,
    messages: Vec<ChatMessage>,
    tally: StatsTally,
    touched: BTreeSet<String>,
    /// The assistant message being collected and the results of its tool calls.
    reply: Option<ChatMessage>,
    results: Vec<ChatMessage>,
    /// The turn of the items being read, with what `thread/turns/list` said about it.
    turn: Option<(String, Option<&'a Turn>)>,
}

impl Builder<'_> {
    fn add(&mut self, item: &ThreadItem) {
        match item {
            ThreadItem::UserMessage(message) => {
                self.flush();

                let blocks = message.content.iter().filter_map(input_block).collect();
                let prompt = self.message(MessageRole::User, blocks, self.started_at());

                self.tally.note_message();
                self.tally.note_time(prompt.timestamp);
                self.messages.push(prompt);
            }
            ThreadItem::AgentMessage(message) if !message.text.trim().is_empty() => {
                let text = ChatBlock::Text {
                    text: message.text.clone(),
                };
                let reply = self.reply_for_text();
                reply.blocks.push(text);
                reply.stop_reason = message.phase.map(|phase| phase.wire().to_owned());
            }
            ThreadItem::Reasoning(reasoning) => {
                let parts = if reasoning.summary.is_empty() {
                    &reasoning.content
                } else {
                    &reasoning.summary
                };
                let text = parts.join("\n\n");

                if !text.trim().is_empty() {
                    let thinking = ChatBlock::Thinking { text, redacted: false };
                    self.reply_for_text().blocks.push(thinking);
                }
            }
            ThreadItem::ContextCompaction { .. } => {
                self.flush();

                let marker = self.message(MessageRole::CompactionSummary, Vec::new(), self.started_at());
                self.messages.push(marker);
            }
            _ => self.add_tool(item),
        }
    }

    /// A tool item as a card of the open reply, its result queued after it.
    fn add_tool(&mut self, item: &ThreadItem) {
        let Some(card) = saved_card(item, self.project) else {
            return;
        };

        let id = item.id().to_owned();
        self.touched.extend(card.touched);

        let mut result = self.message(
            MessageRole::ToolResult,
            vec![ChatBlock::Text { text: card.result }],
            self.ended_at(),
        );
        result.tool_call_id = Some(id.clone());
        result.tool_name = Some(card.name.clone());
        result.is_error = Some(card.is_error);
        self.results.push(result);

        let call = ChatBlock::ToolCall {
            id,
            name: card.name,
            args: card.args,
            intent: None,
        };
        self.reply().blocks.push(call);
    }

    /// The open reply, a new one when text follows its tool calls.
    fn reply_for_text(&mut self) -> &mut ChatMessage {
        if !self.results.is_empty() {
            self.flush();
        }

        self.reply()
    }

    fn reply(&mut self) -> &mut ChatMessage {
        let reply = self.reply.take().unwrap_or_else(|| self.new_reply());

        self.reply.insert(reply)
    }

    /// An empty assistant message of the turn being read.
    fn new_reply(&self) -> ChatMessage {
        let mut reply = self.message(MessageRole::Assistant, Vec::new(), self.ended_at());
        reply.model = self.origin.model.map(str::to_owned);
        reply.provider = self.origin.provider.map(str::to_owned);

        reply
    }

    /// Ends the turn being read: its failure or interruption on its last reply.
    fn end_turn(&mut self) {
        let turn: Option<&Turn> = self.turn.as_ref().and_then(|(_, turn)| *turn);
        let status = turn.map(|turn| (turn.status, turn.error.as_ref().map(|error| error.message.trim())));

        match status {
            Some((TurnStatus::Failed, error)) => {
                let error = error.filter(|message| !message.is_empty()).map(str::to_owned);
                let reply = self.reply();
                reply.stop_reason = Some("error".to_owned());
                reply.error_message = error;
            }
            Some((TurnStatus::Interrupted, _)) => {
                if let Some(reply) = self.reply.as_mut() {
                    reply.stop_reason = Some("aborted".to_owned());
                }
            }
            _ => {}
        }

        self.flush();
        self.turn = None;
    }

    /// Adds the open reply and its tool results to the transcript.
    fn flush(&mut self) {
        let Some(reply) = self.reply.take() else {
            self.messages.append(&mut self.results);
            return;
        };

        let calls = reply
            .blocks
            .iter()
            .filter(|block| matches!(block, ChatBlock::ToolCall { .. }))
            .count();

        self.tally.note_message();
        self.tally.note_time(reply.timestamp);
        self.tally.note_reply(calls, reply.model.as_deref(), None, None);

        self.messages.push(reply);
        self.messages.append(&mut self.results);
    }

    /// A message of the turn being read.
    fn message(&self, role: MessageRole, blocks: Vec<ChatBlock>, timestamp: Option<i64>) -> ChatMessage {
        ChatMessage {
            role,
            id: self.turn.as_ref().map(|(id, _)| id.clone()),
            timestamp,
            blocks,
            tool_call_id: None,
            tool_name: None,
            is_error: None,
            model: None,
            provider: None,
            usage: None,
            cost_usd: None,
            stop_reason: None,
            error_message: None,
        }
    }

    /// When the turn being read started (Unix milliseconds).
    fn started_at(&self) -> Option<i64> {
        self.turn_time(|turn| turn.started_at)
    }

    /// When the turn being read ended, else when it started.
    fn ended_at(&self) -> Option<i64> {
        self.turn_time(|turn| turn.completed_at.or(turn.started_at))
    }

    fn turn_time(&self, seconds: impl Fn(&Turn) -> Option<u64>) -> Option<i64> {
        let (_, turn) = self.turn.as_ref()?;
        let seconds = seconds((*turn)?)?;

        i64::try_from(seconds).ok()?.checked_mul(1000)
    }
}

/// A part of what the user sent as a content block: text and images; mentions and skills are
/// already named in the text.
fn input_block(input: &UserInput) -> Option<ChatBlock> {
    match input {
        UserInput::Text { text, .. } if !text.is_empty() => Some(ChatBlock::Text { text: text.clone() }),
        UserInput::Image { url } => Some(ChatBlock::Image {
            mime_type: data_url_type(url).unwrap_or(ANY_IMAGE).to_owned(),
        }),
        UserInput::LocalImage { .. } => Some(ChatBlock::Image {
            mime_type: ANY_IMAGE.to_owned(),
        }),
        _ => None,
    }
}

/// The media type of a `data:<type>;base64,…` URL.
fn data_url_type(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("data:")?;
    let end = rest.find([';', ','])?;

    rest[..end].starts_with("image/").then(|| &rest[..end])
}
