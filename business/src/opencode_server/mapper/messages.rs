//! The messages and tool calls of a live OpenCode chat, from `message.updated`,
//! `message.part.updated` and `message.part.delta` events.
//!
//! - Messages of the chat's own session stream: `messageStart` once the first text, reasoning or
//!   tool part arrives, deltas with the part's content index, `messageEnd` with the blocks of
//!   every part seen ([`assistant_message`]) once `time.completed` is set. A user message ends as
//!   soon as an event about something else arrives (its parts come right after it).
//! - A part's full text reconciles what was streamed: only text past what was already sent is
//!   sent, so nothing repeats; a rewritten text waits for `messageEnd`.
//! - Tool calls: `toolStart` once the call runs (its input is known then), `toolUpdate` for new
//!   partial output, `toolEnd` once it completed or failed.
//! - A message or call that ended is never reported again; late updates are ignored. So are the
//!   messages a session already had when the chat moved to it ([`Mapper::seen`]): a fork's copies
//!   are announced by events that may arrive after the move.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use serde_json::Value;

use super::Mapper;
use super::message::{assistant_message, blank_message, part_type, text, tool_call_id, tool_outcome, user_message};
use super::touched::part_touched;
use crate::model::{AgentEvent, MessageRole};
use crate::pi_family::normalize::tool_result;
use crate::security::redact_text;

/// Result text of a call still running when its turn ended.
const UNFINISHED_TOOL: &str = "The tool call did not finish.";

/// Open messages and tool calls (see the module docs).
pub(super) struct Messages {
    project: PathBuf,
    directory: PathBuf,
    /// Open messages of the chat's session, in arrival order.
    open: Vec<Track>,
    /// Messages that ended.
    done: HashSet<String>,
    /// Tool calls by part id (OpenCode's ids sort by creation).
    tools: BTreeMap<String, ToolTrack>,
}

struct Track {
    id: String,
    role: MessageRole,
    info: Value,
    parts: Vec<Part>,
    started: bool,
    next_index: u32,
}

struct Part {
    id: String,
    value: Value,
    index: u32,
}

#[derive(Default)]
struct ToolTrack {
    /// The latest part, until the call ended.
    part: Value,
    started: bool,
    ended: bool,
    output: Option<String>,
}

impl Messages {
    pub(super) fn new(project: PathBuf, directory: PathBuf) -> Self {
        Self {
            project,
            directory,
            open: Vec::new(),
            done: HashSet::new(),
            tools: BTreeMap::new(),
        }
    }

    /// The open user message waiting for its parts, unless `envelope_message` is it.
    pub(super) fn pending_user(&self, envelope_message: Option<&str>) -> Option<String> {
        self.open
            .iter()
            .find(|track| track.role == MessageRole::User)
            .filter(|track| Some(track.id.as_str()) != envelope_message)
            .map(|track| track.id.clone())
    }

    /// A `message.updated` of the chat's session.
    pub(super) fn message_updated(&mut self, info: &Value, out: &mut Vec<AgentEvent>) {
        let Some(id) = text(info, "id") else {
            return;
        };

        if self.done.contains(&id) {
            return;
        }

        let role = match info.get("role").and_then(Value::as_str) {
            Some("user") => MessageRole::User,
            Some("assistant") => MessageRole::Assistant,
            _ => return,
        };

        match self.open.iter_mut().find(|track| track.id == id) {
            Some(track) => track.info = info.clone(),
            None => self.open.push(Track {
                id: id.clone(),
                role,
                info: info.clone(),
                parts: Vec::new(),
                started: false,
                next_index: 0,
            }),
        }

        if role == MessageRole::Assistant && info.pointer("/time/completed").is_some() {
            self.end(&id, None, out);
        }
    }

    /// A `message.part.updated` of the chat's session.
    pub(super) fn part_updated(&mut self, part: &Value, out: &mut Vec<AgentEvent>) {
        if part_type(part) == "tool" {
            self.tool_updated(part, out);
        }

        let (Some(message_id), Some(part_id)) = (text(part, "messageID"), text(part, "id")) else {
            return;
        };

        let Some(track) = self.open.iter_mut().find(|track| track.id == message_id) else {
            return;
        };

        let kind = part_type(part);
        let shown = matches!(kind, "text" | "reasoning" | "tool");

        if track.role == MessageRole::Assistant && shown && !track.started {
            track.started = true;
            out.push(AgentEvent::MessageStart {
                message_id: track.id.clone(),
                role: MessageRole::Assistant,
            });
        }

        let position = track.parts.iter().position(|known| known.id == part_id);
        let index = match position {
            Some(position) => track.parts[position].index,
            None => {
                let index = track.next_index;
                if shown {
                    track.next_index += 1;
                }

                index
            }
        };

        let streamed = position
            .and_then(|position| track.parts[position].value.get("text").and_then(Value::as_str))
            .unwrap_or_default();
        let full = part.get("text").and_then(Value::as_str).unwrap_or_default();

        if track.role == MessageRole::Assistant
            && matches!(kind, "text" | "reasoning")
            && let Some(rest) = full.strip_prefix(streamed)
            && !rest.is_empty()
        {
            out.push(delta(&track.id, index, kind, rest.to_owned()));
        }

        let value = part.clone();
        match position {
            Some(position) => track.parts[position].value = value,
            None => track.parts.push(Part {
                id: part_id,
                value,
                index,
            }),
        }
    }

    /// A `message.part.delta` of the chat's session (only `text` deltas are shown).
    pub(super) fn part_delta(&mut self, properties: &Value, out: &mut Vec<AgentEvent>) {
        let field = properties.get("field").and_then(Value::as_str);
        let (Some(message_id), Some(part_id), Some(addition)) = (
            text(properties, "messageID"),
            text(properties, "partID"),
            text(properties, "delta"),
        ) else {
            return;
        };

        if field != Some("text") {
            return;
        }

        let Some(track) = self.open.iter_mut().find(|track| track.id == message_id) else {
            return;
        };

        let Some(part) = track.parts.iter_mut().find(|part| part.id == part_id) else {
            return;
        };

        let kind = part_type(&part.value).to_owned();
        if !matches!(kind.as_str(), "text" | "reasoning") {
            return;
        }

        let mut streamed = part
            .value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        streamed.push_str(&addition);
        part.value["text"] = Value::String(streamed);

        out.push(delta(&message_id, part.index, &kind, addition));
    }

    /// Ends the open user message `id` (its parts are in).
    pub(super) fn end_user(&mut self, id: &str, out: &mut Vec<AgentEvent>) {
        self.end(id, None, out);
    }

    /// Ends whatever is still open when a turn ends: calls fail with [`UNFINISHED_TOOL`], messages
    /// end with what arrived of them (replies of an `aborted` turn as aborted, OpenCode completes
    /// them only after the turn).
    pub(super) fn end_all(&mut self, aborted: bool, out: &mut Vec<AgentEvent>) {
        let unfinished: Vec<String> = self
            .tools
            .iter()
            .filter(|(_, tool)| tool.started && !tool.ended)
            .map(|(id, _)| id.clone())
            .collect();

        for part_id in unfinished {
            self.end_unfinished_tool(&part_id, out);
        }

        let open: Vec<String> = self.open.iter().map(|track| track.id.clone()).collect();

        for id in open {
            self.end(&id, aborted.then_some("aborted"), out);
        }
    }

    /// Emits the end of message `id`; `stop_reason` fills in a reply's missing one.
    fn end(&mut self, id: &str, stop_reason: Option<&str>, out: &mut Vec<AgentEvent>) {
        let Some(position) = self.open.iter().position(|track| track.id == id) else {
            return;
        };

        let track = self.open.remove(position);
        let parts: Vec<Value> = track.parts.into_iter().map(|part| part.value).collect();
        self.done.insert(track.id.clone());

        let message = match track.role {
            MessageRole::User => user_message(&track.info, &parts),
            _ => assistant_message(&track.info, &parts)
                .map(|(message, _)| message)
                .or_else(|| {
                    track
                        .started
                        .then(|| blank_message(&track.info, MessageRole::Assistant))
                }),
        };

        let Some(mut message) = message else {
            return;
        };

        if message.role == MessageRole::Assistant && message.stop_reason.is_none() {
            message.stop_reason = stop_reason.map(str::to_owned);
        }

        if !track.started {
            out.push(AgentEvent::MessageStart {
                message_id: track.id.clone(),
                role: message.role,
            });
        }

        out.push(AgentEvent::MessageEnd {
            message_id: track.id,
            message,
        });
    }

    fn tool_updated(&mut self, part: &Value, out: &mut Vec<AgentEvent>) {
        let Some(part_id) = text(part, "id") else {
            return;
        };

        let state = part.get("state").unwrap_or(&Value::Null);
        let status = state.get("status").and_then(Value::as_str).unwrap_or_default();
        let tool = self.tools.entry(part_id).or_default();

        if tool.ended {
            return;
        }

        tool.part = part.clone();

        if status == "pending" {
            return;
        }

        let tool_call_id = tool_call_id(part);
        let name = text(part, "tool").unwrap_or_default();

        if !tool.started {
            tool.started = true;
            out.push(AgentEvent::ToolStart {
                tool_call_id: tool_call_id.clone(),
                name: name.clone(),
                args: state.get("input").cloned().unwrap_or(Value::Null),
                intent: text(state, "title"),
            });
        }

        if let Some((output, is_error)) = tool_outcome(state) {
            tool.ended = true;
            tool.part = Value::Null;
            out.push(AgentEvent::ToolEnd {
                touched_paths: part_touched(&self.project, &self.directory, part),
                tool_call_id,
                name,
                is_error,
                result: tool_result(Some(&Value::String(output))),
            });
            return;
        }

        let output = state.pointer("/metadata/output").and_then(Value::as_str);

        if let Some(output) = output.filter(|output| tool.output.as_deref() != Some(*output)) {
            tool.output = Some(output.to_owned());
            out.push(AgentEvent::ToolUpdate {
                tool_call_id,
                partial: tool_result(Some(&Value::String(redact_text(output)))),
            });
        }
    }

    fn end_unfinished_tool(&mut self, part_id: &str, out: &mut Vec<AgentEvent>) {
        let Some(tool) = self.tools.get_mut(part_id) else {
            return;
        };

        let part = std::mem::take(&mut tool.part);
        tool.ended = true;

        out.push(AgentEvent::ToolEnd {
            touched_paths: part_touched(&self.project, &self.directory, &part),
            tool_call_id: tool_call_id(&part),
            name: text(&part, "tool").unwrap_or_default(),
            is_error: true,
            result: tool_result(Some(&Value::String(UNFINISHED_TOOL.to_owned()))),
        });
    }
}

impl Mapper {
    /// The session already holds `records` (`{info, parts}`): later events about them are ignored.
    pub fn seen(&mut self, records: &[Value]) {
        let messages = &mut self.messages;

        for record in records {
            messages
                .done
                .extend(record.pointer("/info/id").and_then(Value::as_str).map(str::to_owned));

            let parts = record.get("parts").and_then(Value::as_array).into_iter().flatten();

            for part in parts.filter(|part| part_type(part) == "tool") {
                if let Some(id) = text(part, "id") {
                    messages.tools.entry(id).or_default().ended = true;
                }
            }
        }
    }
}

fn delta(message_id: &str, content_index: u32, kind: &str, delta: String) -> AgentEvent {
    let message_id = message_id.to_owned();

    if kind == "reasoning" {
        return AgentEvent::ThinkingDelta {
            message_id,
            content_index,
            delta,
        };
    }

    AgentEvent::TextDelta {
        message_id,
        content_index,
        delta,
    }
}
