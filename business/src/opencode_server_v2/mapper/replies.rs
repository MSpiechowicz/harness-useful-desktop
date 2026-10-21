//! The messages of a live OpenCode 2 chat: the assistant's replies, streamed from the step, text,
//! reasoning and tool events, and the user's prompts.
//!
//! - **Replies:** OpenCode 2 writes one assistant message per model step
//!   (`assistantMessageID`). `messageStart` comes with its first text, reasoning or tool call,
//!   deltas carry the part's content index (parts are numbered in arrival order, as on OpenCode
//!   1), and `messageEnd` comes with `session.step.ended` (its finish reason, tokens and cost) or
//!   `session.step.failed` (`aborted` or `error`, with the redacted error), with the blocks of
//!   every part: text, reasoning (empty when the provider withheld it) and tool calls.
//! - A part's full text (`*.ended`) reconciles what was streamed: only text past what was already
//!   sent is sent, so nothing repeats. A rewritten text waits for `messageEnd`.
//! - **Prompts:** a user inbox item shows as the user's message once the run takes it
//!   (`session.inbox.delivered`): its text, images (MIME type only) and attached files as `@name`.
//!   Synthetic items (a `!command`'s output) and compactions are OpenCode's own and not shown.
//! - Events of a step the chat never saw start still open its message, so a reply whose first
//!   events were missed shows from where the chat joined. A message that ended is never reported
//!   again; nor are those a session already had ([`Replies::mark_done`]).

use std::collections::HashSet;

use super::ContextNote;
use super::tools::Tools;
use crate::model::{AgentEvent, ChatBlock, ChatMessage, MessageRole, TokenUsage};
use crate::opencode_server_v2::events::{
    InboxEnqueued, PartDelta, PartEnded, PartRef, StepEnded, StepFailed, StepStarted,
};
use crate::opencode_server_v2::messages::{FileAttachment, SessionMessage, Times, UserMessage};
use crate::opencode_server_v2::wire::{ModelRef, Tokens};
use crate::security::redact_text;

/// The error kind of an interruption.
const ABORTED: &str = "aborted";

/// A streamed part's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PartKind {
    Text,
    Reasoning,
}

/// Open replies (see the module docs).
#[derive(Default)]
pub(super) struct Replies {
    /// Open replies, in arrival order.
    open: Vec<Reply>,
    /// Messages that ended.
    done: HashSet<String>,
}

struct Reply {
    id: String,
    /// `messageStart` was emitted.
    started: bool,
    parts: Vec<Part>,
    model: Option<ModelRef>,
    /// When the chat first heard of it, in milliseconds since the epoch.
    created: Option<u64>,
}

#[derive(PartialEq, Eq)]
enum PartKey {
    Streamed(PartKind, u64),
    Tool(String),
}

struct Part {
    key: PartKey,
    /// The streamed text so far, then the full text.
    text: String,
}

/// How a reply ends.
struct Ending {
    stop_reason: Option<String>,
    error: Option<String>,
    usage: Option<TokenUsage>,
    cost: Option<f64>,
}

impl Replies {
    /// Message `id` is already in the chat: later events about it are ignored.
    pub(super) fn mark_done(&mut self, id: &str) {
        self.done.insert(id.to_owned());
    }

    /// True when message `id` ended, is open, or the session already had it.
    pub(super) fn knows(&self, id: &str) -> bool {
        self.done.contains(id) || self.open.iter().any(|reply| reply.id == id)
    }

    /// `session.step.started`: a reply begins (nothing shows before its first part).
    pub(super) fn step_started(&mut self, step: &StepStarted, created: Option<u64>) {
        if let Some(reply) = self.reply(&step.assistant_message_id, created) {
            reply.model = step.model.clone();
        }
    }

    /// `session.text.started` or `session.reasoning.started`.
    pub(super) fn part_started(
        &mut self,
        kind: PartKind,
        part: &PartRef,
        created: Option<u64>,
        out: &mut Vec<AgentEvent>,
    ) {
        let key = PartKey::Streamed(kind, part.ordinal);
        self.part(&part.assistant_message_id, key, created, out);
    }

    /// A text or reasoning fragment.
    pub(super) fn part_delta(
        &mut self,
        kind: PartKind,
        delta: &PartDelta,
        created: Option<u64>,
        out: &mut Vec<AgentEvent>,
    ) {
        let message = &delta.assistant_message_id;
        let key = PartKey::Streamed(kind, delta.ordinal);

        let Some((index, part)) = self.part(message, key, created, out) else {
            return;
        };

        if delta.delta.is_empty() {
            return;
        }

        part.text.push_str(&delta.delta);
        out.push(delta_event(message, index, kind, delta.delta.clone()));
    }

    /// The full text of a text or reasoning part: sends what was not streamed yet.
    pub(super) fn part_ended(
        &mut self,
        kind: PartKind,
        ended: &PartEnded,
        created: Option<u64>,
        out: &mut Vec<AgentEvent>,
    ) {
        let message = &ended.assistant_message_id;
        let key = PartKey::Streamed(kind, ended.ordinal);

        let Some((index, part)) = self.part(message, key, created, out) else {
            return;
        };

        if let Some(rest) = ended.text.strip_prefix(part.text.as_str())
            && !rest.is_empty()
        {
            out.push(delta_event(message, index, kind, rest.to_owned()));
        }

        part.text = ended.text.clone();
    }

    /// A tool call of reply `message` begins (its input streams).
    pub(super) fn tool_started(&mut self, message: &str, call: &str, created: Option<u64>, out: &mut Vec<AgentEvent>) {
        self.part(message, PartKey::Tool(call.to_owned()), created, out);
    }

    /// `session.step.ended`: the reply ends with its finish reason, tokens and cost. The context
    /// note of a step with output.
    pub(super) fn step_ended(
        &mut self,
        step: &StepEnded,
        tools: &Tools,
        out: &mut Vec<AgentEvent>,
    ) -> Option<ContextNote> {
        let model = self
            .open
            .iter()
            .find(|reply| reply.id == step.assistant_message_id)
            .and_then(|reply| reply.model.clone());

        let ending = Ending {
            stop_reason: Some(step.finish.clone()).filter(|finish| !finish.is_empty()),
            error: None,
            usage: Some(token_usage(&step.tokens)),
            cost: Some(step.cost),
        };
        self.end(&step.assistant_message_id, ending, tools, out);

        (step.tokens.output > 0).then(|| ContextNote::Tokens {
            tokens: token_usage(&step.tokens).total,
            model,
        })
    }

    /// `session.step.failed`: the reply ends as `aborted` or `error`. A step that showed nothing
    /// shows only when it failed for another reason than an interruption.
    pub(super) fn step_failed(&mut self, step: &StepFailed, tools: &Tools, out: &mut Vec<AgentEvent>) {
        let aborted = step.error.kind == ABORTED;
        let id = &step.assistant_message_id;
        let open = self.open.iter().any(|reply| &reply.id == id);

        if !open && (aborted || self.done.contains(id)) {
            return;
        }

        if !open {
            self.reply(id, None);
        }

        let ending = Ending {
            stop_reason: Some(if aborted { "aborted" } else { "error" }.to_owned()),
            error: Some(redact_text(&step.error.message)).filter(|message| !message.is_empty()),
            usage: None,
            cost: None,
        };
        self.end(id, ending, tools, out);
    }

    /// Ends whatever is still open when a turn ends (replies of an `aborted` turn as aborted).
    pub(super) fn end_all(&mut self, aborted: bool, tools: &Tools, out: &mut Vec<AgentEvent>) {
        let open: Vec<String> = self.open.iter().map(|reply| reply.id.clone()).collect();

        for id in open {
            let ending = Ending {
                stop_reason: aborted.then(|| ABORTED.to_owned()),
                error: None,
                usage: None,
                cost: None,
            };
            self.end(&id, ending, tools, out);
        }
    }

    /// A user inbox item the run took: the user's message.
    pub(super) fn user_delivered(&mut self, enqueued: &InboxEnqueued, created: Option<u64>, out: &mut Vec<AgentEvent>) {
        let id = &enqueued.inbox_id;

        if !self.done.insert(id.clone()) {
            return;
        }

        let payload = &enqueued.item.payload;
        let files = payload
            .files
            .iter()
            .filter_map(|file| serde_json::from_value::<FileAttachment>(file.clone()).ok())
            .collect();
        let user = UserMessage {
            id: id.clone(),
            time: Times {
                created: timestamp(created),
                completed: None,
            },
            text: payload.text.clone().unwrap_or_default(),
            files,
            agents: Vec::new(),
        };

        let Some(message) = SessionMessage::User(user).chat_messages().into_iter().next() else {
            return;
        };

        out.push(AgentEvent::MessageStart {
            message_id: id.clone(),
            role: MessageRole::User,
        });
        out.push(AgentEvent::MessageEnd {
            message_id: id.clone(),
            message,
        });
    }

    /// The open reply `id`, opened when new; `None` once it ended.
    fn reply(&mut self, id: &str, created: Option<u64>) -> Option<&mut Reply> {
        if self.done.contains(id) {
            return None;
        }

        let position = match self.open.iter().position(|reply| reply.id == id) {
            Some(position) => position,
            None => {
                self.open.push(Reply {
                    id: id.to_owned(),
                    started: false,
                    parts: Vec::new(),
                    model: None,
                    created,
                });
                self.open.len() - 1
            }
        };

        Some(&mut self.open[position])
    }

    /// Part `key` of reply `message` and its content index, added (with the reply's
    /// `messageStart`) when new.
    fn part(
        &mut self,
        message: &str,
        key: PartKey,
        created: Option<u64>,
        out: &mut Vec<AgentEvent>,
    ) -> Option<(u32, &mut Part)> {
        let reply = self.reply(message, created)?;

        if !reply.started {
            reply.started = true;
            out.push(AgentEvent::MessageStart {
                message_id: reply.id.clone(),
                role: MessageRole::Assistant,
            });
        }

        let position = match reply.parts.iter().position(|part| part.key == key) {
            Some(position) => position,
            None => {
                reply.parts.push(Part {
                    key,
                    text: String::new(),
                });
                reply.parts.len() - 1
            }
        };

        let index = u32::try_from(position).unwrap_or(u32::MAX);

        Some((index, &mut reply.parts[position]))
    }

    /// Emits the end of reply `id`.
    fn end(&mut self, id: &str, ending: Ending, tools: &Tools, out: &mut Vec<AgentEvent>) {
        let Some(position) = self.open.iter().position(|reply| reply.id == id) else {
            return;
        };

        let reply = self.open.remove(position);
        self.done.insert(reply.id.clone());

        let blocks: Vec<ChatBlock> = reply
            .parts
            .iter()
            .filter_map(|part| match &part.key {
                PartKey::Streamed(PartKind::Text, _) if part.text.is_empty() => None,
                PartKey::Streamed(PartKind::Text, _) => Some(ChatBlock::Text {
                    text: part.text.clone(),
                }),
                PartKey::Streamed(PartKind::Reasoning, _) => Some(ChatBlock::Thinking {
                    text: part.text.clone(),
                    redacted: part.text.is_empty(),
                }),
                PartKey::Tool(call) => tools.block(call),
            })
            .collect();

        if blocks.is_empty() && ending.error.is_none() && !reply.started {
            return;
        }

        let model = reply.model.as_ref();
        let message = ChatMessage {
            id: Some(reply.id.clone()),
            role: MessageRole::Assistant,
            blocks,
            model: model.map(|model| model.id.clone()).filter(|id| !id.is_empty()),
            provider: model.map(|model| model.provider_id.clone()).filter(|id| !id.is_empty()),
            stop_reason: ending.stop_reason,
            error_message: ending.error,
            usage: ending.usage,
            cost_usd: ending.cost,
            timestamp: timestamp(reply.created),
            tool_call_id: None,
            tool_name: None,
            is_error: None,
        };

        if !reply.started {
            out.push(AgentEvent::MessageStart {
                message_id: reply.id.clone(),
                role: MessageRole::Assistant,
            });
        }

        out.push(AgentEvent::MessageEnd {
            message_id: reply.id,
            message,
        });
    }
}

/// A step's tokens as the chat's usage; the total includes reasoning tokens.
fn token_usage(tokens: &Tokens) -> TokenUsage {
    TokenUsage {
        input: tokens.input,
        output: tokens.output,
        cache_read: tokens.cache.read,
        cache_write: tokens.cache.write,
        total: tokens.input + tokens.output + tokens.reasoning + tokens.cache.read + tokens.cache.write,
    }
}

fn timestamp(created: Option<u64>) -> Option<i64> {
    created.and_then(|created| i64::try_from(created).ok())
}

fn delta_event(message_id: &str, content_index: u32, kind: PartKind, delta: String) -> AgentEvent {
    let message_id = message_id.to_owned();

    match kind {
        PartKind::Reasoning => AgentEvent::ThinkingDelta {
            message_id,
            content_index,
            delta,
        },
        PartKind::Text => AgentEvent::TextDelta {
            message_id,
            content_index,
            delta,
        },
    }
}
