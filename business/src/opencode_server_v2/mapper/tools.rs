//! The tool calls of a live OpenCode 2 chat as tool cards, from the `session.tool.*` events,
//! named by their OpenCode 2 ids (`shell`, `edit`, `write`, `patch`, `subagent`, …).
//!
//! - `session.tool.input.started` names the call and opens its place in the reply; its input
//!   streams as text (`input.delta`, `input.ended`) and is not shown.
//! - `toolStart` once the call runs (`session.tool.called`, with the parsed input), `toolUpdate`
//!   for new partial output (`metadata.output` of `session.tool.progress`), `toolEnd` once it
//!   succeeded (its redacted text content) or failed (its redacted error, `aborted` ones
//!   included), with the project files the call may have changed
//!   ([`call_touched`](crate::opencode_server_v2::tools::call_touched)). A call that ends without
//!   having been called (interrupted while its input streamed) starts and ends at once.
//! - A call still running when its turn ends fails with [`UNFINISHED_TOOL`]. A call the chat
//!   never saw named (its first events came before the chat followed the stream) is not shown,
//!   nor is one a session already had.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::model::{AgentEvent, ChatBlock};
use crate::opencode_server_v2::events::{
    ToolCalled, ToolContent, ToolFailed, ToolInputDelta, ToolInputEnded, ToolInputStarted, ToolProgress, ToolSuccess,
};
use crate::opencode_server_v2::tools::call_touched;
use crate::pi_family::normalize::tool_result;
use crate::security::redact_text;

/// Result text of a call still running when its turn ended.
const UNFINISHED_TOOL: &str = "The tool call did not finish.";

/// The calls of the running turn (see the module docs).
pub(super) struct Tools {
    project: PathBuf,
    directory: PathBuf,
    calls: HashMap<String, Call>,
    /// Call ids in arrival order.
    order: Vec<String>,
    /// Calls of earlier turns, and of messages the session already had.
    done: HashSet<String>,
}

struct Call {
    /// The tool id.
    name: String,
    /// The input text as streamed.
    raw: String,
    /// The parsed input, once known.
    input: Value,
    /// `toolStart` was emitted.
    started: bool,
    /// `toolEnd` was emitted.
    ended: bool,
    /// The last partial output sent.
    output: Option<String>,
}

impl Tools {
    pub(super) fn new(project: PathBuf, directory: PathBuf) -> Self {
        Self {
            project,
            directory,
            calls: HashMap::new(),
            order: Vec::new(),
            done: HashSet::new(),
        }
    }

    /// Call `id` is already in the chat: later events about it are ignored.
    pub(super) fn mark_done(&mut self, id: &str) {
        self.done.insert(id.to_owned());
    }

    /// `session.tool.input.started`; true for a call new to the chat.
    pub(super) fn input_started(&mut self, started: &ToolInputStarted) -> bool {
        if self.done.contains(&started.id) || self.calls.contains_key(&started.id) {
            return false;
        }

        self.order.push(started.id.clone());
        self.calls.insert(
            started.id.clone(),
            Call {
                name: started.name.clone(),
                raw: String::new(),
                input: Value::Null,
                started: false,
                ended: false,
                output: None,
            },
        );

        true
    }

    pub(super) fn input_delta(&mut self, delta: &ToolInputDelta) {
        if let Some(call) = self.calls.get_mut(&delta.id) {
            call.raw.push_str(&delta.delta);
        }
    }

    pub(super) fn input_ended(&mut self, ended: &ToolInputEnded) {
        if let Some(call) = self.calls.get_mut(&ended.id) {
            call.raw.clone_from(&ended.text);
        }
    }

    /// `session.tool.called`: the call runs with its parsed input.
    pub(super) fn called(&mut self, called: &ToolCalled, out: &mut Vec<AgentEvent>) {
        let Some(call) = self.calls.get_mut(&called.id).filter(|call| !call.ended) else {
            return;
        };

        call.input = called.input.clone();
        start(&called.id, call, out);
    }

    /// `session.tool.progress`: new partial output, when the tool reports one.
    pub(super) fn progress(&mut self, progress: &ToolProgress, out: &mut Vec<AgentEvent>) {
        let Some(call) = self
            .calls
            .get_mut(&progress.id)
            .filter(|call| call.started && !call.ended)
        else {
            return;
        };

        let output = progress.metadata.get("output").and_then(Value::as_str);

        if let Some(output) = output.filter(|output| call.output.as_deref() != Some(*output)) {
            call.output = Some(output.to_owned());
            out.push(AgentEvent::ToolUpdate {
                tool_call_id: progress.id.clone(),
                partial: tool_result(Some(&Value::String(redact_text(output)))),
            });
        }
    }

    pub(super) fn success(&mut self, success: &ToolSuccess, out: &mut Vec<AgentEvent>) {
        self.finish(&success.id, &content_text(&success.content), false, out);
    }

    pub(super) fn failed(&mut self, failed: &ToolFailed, out: &mut Vec<AgentEvent>) {
        let output = if failed.error.message.is_empty() {
            content_text(&failed.content)
        } else {
            failed.error.message.clone()
        };

        self.finish(&failed.id, &output, true, out);
    }

    /// Ends every call still running (its turn ended) with [`UNFINISHED_TOOL`].
    pub(super) fn end_unfinished(&mut self, out: &mut Vec<AgentEvent>) {
        let unfinished: Vec<String> = self
            .order
            .iter()
            .filter(|id| self.calls.get(*id).is_some_and(|call| call.started && !call.ended))
            .cloned()
            .collect();

        for id in unfinished {
            self.finish(&id, UNFINISHED_TOOL, true, out);
        }
    }

    /// Forgets the turn's calls once their replies ended; later events about them are ignored.
    pub(super) fn forget(&mut self) {
        self.done.extend(self.order.drain(..));
        self.calls.clear();
    }

    /// Call `id` as a block of its reply; arguments that are not an object become `{}`.
    pub(super) fn block(&self, id: &str) -> Option<ChatBlock> {
        let call = self.calls.get(id)?;
        let args = if call.input.is_object() {
            call.input.clone()
        } else {
            json!({})
        };

        Some(ChatBlock::ToolCall {
            id: id.to_owned(),
            name: call.name.clone(),
            args,
            intent: None,
        })
    }

    /// The parsed input of call `id`, once known.
    pub(super) fn input(&self, id: &str) -> Option<&Value> {
        self.calls
            .get(id)
            .map(|call| &call.input)
            .filter(|input| !input.is_null())
    }

    /// Emits the end of call `id` (and its start when it never ran).
    fn finish(&mut self, id: &str, output: &str, is_error: bool, out: &mut Vec<AgentEvent>) {
        let Some(call) = self.calls.get_mut(id).filter(|call| !call.ended) else {
            return;
        };

        if call.input.is_null() {
            call.input = serde_json::from_str::<Value>(&call.raw)
                .ok()
                .filter(Value::is_object)
                .unwrap_or(Value::Null);
        }

        start(id, call, out);
        call.ended = true;

        out.push(AgentEvent::ToolEnd {
            tool_call_id: id.to_owned(),
            name: call.name.clone(),
            is_error,
            result: tool_result(Some(&Value::String(redact_text(output)))),
            touched_paths: call_touched(&self.project, &self.directory, &call.name, &call.input),
        });
    }
}

/// Emits `toolStart` for call `id` once.
fn start(id: &str, call: &mut Call, out: &mut Vec<AgentEvent>) {
    if call.started {
        return;
    }

    call.started = true;
    out.push(AgentEvent::ToolStart {
        tool_call_id: id.to_owned(),
        name: call.name.clone(),
        args: call.input.clone(),
        intent: None,
    });
}

/// The text items of a tool's output, one per line. Files (images a tool read, as `data:` URLs)
/// are not shown.
fn content_text(content: &[ToolContent]) -> String {
    let lines: Vec<&str> = content
        .iter()
        .filter_map(|item| match item {
            ToolContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();

    lines.join("\n")
}
