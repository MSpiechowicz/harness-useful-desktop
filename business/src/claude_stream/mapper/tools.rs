//! Tool calls as tool cards, named by their Claude Code ids (`Bash`, `Read`, `Write`, `Edit`,
//! `Task`, `AskUserQuestion`, MCP tools as `mcp__server__tool`).
//!
//! - A `tool_use` block's input streams as JSON text (`input_json_delta`) and is not shown. The
//!   call starts (`toolStart`) with its complete input: the `assistant` frame of the block, which
//!   comes first, or else the streamed text once the block stops. A string `description` in the
//!   input (`Bash`, `Task`) is its intent.
//! - The `tool_result` block of a `user` frame ends it (`toolEnd`) with its redacted text and
//!   `is_error` (a denied call is an error with the denial message). A call that did not fail
//!   names the project files it may have changed ([`touched_paths`]).
//! - Streamed input stops growing at [`MAX_STREAMED_BYTES`]: a call that starts from it then shows
//!   [`TOOL_INPUT_CUT`] as its only argument.
//! - A call still running when its turn ends fails with [`UNFINISHED_TOOL`]. Results of calls the
//!   chat never saw start (a subagent's, an earlier process's) are not shown.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde_json::{Value, json};

use super::MAX_STREAMED_BYTES;
use crate::claude_stream::tools::touched_paths;
use crate::model::{AgentEvent, ChatBlock};
use crate::pi_family::normalize::tool_result;
use crate::security::redact_text;

/// Result text of a call still running when its turn ended.
pub const UNFINISHED_TOOL: &str = "The tool call did not finish.";

/// The `note` argument of a call whose streamed input grew past [`MAX_STREAMED_BYTES`].
pub const TOOL_INPUT_CUT: &str = "The tool input was too long to show.";

/// The calls of the running turn.
pub(super) struct Tools {
    project: PathBuf,
    calls: HashMap<String, Call>,
    /// Call ids in arrival order.
    order: Vec<String>,
    /// Calls of earlier turns.
    done: HashSet<String>,
}

struct Call {
    name: String,
    /// The input as streamed so far.
    raw: String,
    /// The streamed input grew past the bound (and was dropped).
    raw_cut: bool,
    /// The complete input, once known.
    input: Value,
    started: bool,
    ended: bool,
}

impl Tools {
    pub(super) fn new(project: PathBuf) -> Self {
        Self {
            project,
            calls: HashMap::new(),
            order: Vec::new(),
            done: HashSet::new(),
        }
    }

    /// A `tool_use` block named call `id`; false for a call the chat already has.
    pub(super) fn named(&mut self, id: &str, name: &str) -> bool {
        if id.is_empty() || self.done.contains(id) || self.calls.contains_key(id) {
            return false;
        }

        self.order.push(id.to_owned());
        self.calls.insert(
            id.to_owned(),
            Call {
                name: name.to_owned(),
                raw: String::new(),
                raw_cut: false,
                input: Value::Null,
                started: false,
                ended: false,
            },
        );

        true
    }

    pub(super) fn input_delta(&mut self, id: &str, json: &str) {
        let Some(call) = self.calls.get_mut(id).filter(|call| !call.raw_cut) else {
            return;
        };

        if call.raw.len() + json.len() > MAX_STREAMED_BYTES {
            call.raw_cut = true;
            call.raw = String::new();
            return;
        }

        call.raw.push_str(json);
    }

    /// The block of call `id` stopped: it starts with the streamed input, unless it already did.
    pub(super) fn input_ended(&mut self, id: &str, out: &mut Vec<AgentEvent>) {
        let Some(call) = self.calls.get_mut(id).filter(|call| !call.started) else {
            return;
        };

        if call.input.is_null() {
            call.input = streamed_input(call);
        }

        start(id, call, out);
    }

    /// The `assistant` frame of call `id`: it starts with its complete `input`.
    pub(super) fn called(&mut self, id: &str, name: &str, input: &Value, out: &mut Vec<AgentEvent>) {
        self.named(id, name);

        let Some(call) = self.calls.get_mut(id).filter(|call| !call.ended) else {
            return;
        };

        if input.is_object() {
            call.input = input.clone();
        }

        start(id, call, out);
    }

    /// A `tool_result` for call `id` with `content` (a string or blocks).
    pub(super) fn result(&mut self, id: &str, content: &Value, is_error: bool, out: &mut Vec<AgentEvent>) {
        self.finish(id, &result_text(content), is_error, out);
    }

    /// Ends every call still running (its turn ended) with [`UNFINISHED_TOOL`].
    pub(super) fn end_unfinished(&mut self, out: &mut Vec<AgentEvent>) {
        let unfinished: Vec<String> = self
            .order
            .iter()
            .filter(|id| self.calls.get(*id).is_some_and(|call| !call.ended))
            .cloned()
            .collect();

        for id in unfinished {
            self.finish(&id, UNFINISHED_TOOL, true, out);
        }
    }

    /// Forgets the turn's calls; later frames about them are ignored.
    pub(super) fn forget(&mut self) {
        self.done.extend(self.order.drain(..));
        self.calls.clear();
    }

    /// Call `id` as a block of its reply; input that is not an object becomes `{}`.
    pub(super) fn block(&self, id: &str) -> Option<ChatBlock> {
        let call = self.calls.get(id)?;

        Some(ChatBlock::ToolCall {
            id: id.to_owned(),
            name: call.name.clone(),
            args: object(&call.input),
            intent: intent(&call.input),
        })
    }

    fn finish(&mut self, id: &str, output: &str, is_error: bool, out: &mut Vec<AgentEvent>) {
        let Some(call) = self.calls.get_mut(id).filter(|call| !call.ended) else {
            return;
        };

        if call.input.is_null() {
            call.input = streamed_input(call);
        }

        start(id, call, out);
        call.ended = true;

        let touched = if is_error {
            Vec::new()
        } else {
            touched_paths(&self.project, &call.name, &call.input)
        };

        out.push(AgentEvent::ToolEnd {
            tool_call_id: id.to_owned(),
            name: call.name.clone(),
            is_error,
            result: tool_result(Some(&Value::String(redact_text(output)))),
            touched_paths: touched,
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
        args: object(&call.input),
        intent: intent(&call.input),
    });
}

/// The input `call` streamed; a note once it grew past the bound.
fn streamed_input(call: &Call) -> Value {
    if call.raw_cut {
        return json!({ "note": TOOL_INPUT_CUT });
    }

    parsed(&call.raw)
}

/// The streamed input as JSON; `null` while it is incomplete.
fn parsed(raw: &str) -> Value {
    serde_json::from_str::<Value>(raw)
        .ok()
        .filter(Value::is_object)
        .unwrap_or(Value::Null)
}

fn object(input: &Value) -> Value {
    if input.is_object() { input.clone() } else { json!({}) }
}

/// A call's `description` argument, which Claude Code's `Bash` and `Task` calls carry.
fn intent(input: &Value) -> Option<String> {
    input
        .get("description")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(redact_text)
}

/// The text of a `tool_result`'s content: a string, or its text blocks one per line.
fn result_text(content: &Value) -> String {
    let Value::Array(blocks) = content else {
        return content.as_str().unwrap_or_default().to_owned();
    };

    let mut text = String::new();

    for block in blocks {
        let Some(part) = block["text"].as_str().filter(|_| block["type"] == "text") else {
            continue;
        };

        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(part);
    }

    text
}
