//! The tool calls of a live OpenCode 2 chat as tool cards, from the `session.tool.*` events,
//! named by their OpenCode 2 ids (`shell`, `edit`, `write`, `patch`, `subagent`, …).
//!
//! - `session.tool.input.started` names the call and opens its place in the reply; its input
//!   streams as text (`input.delta`, `input.ended`) and is not shown.
//! - `toolStart` once the call runs (`session.tool.called`, with the parsed input), `toolUpdate`
//!   for new partial output (`metadata.output` of `session.tool.progress`), `toolEnd` once it
//!   succeeded (its redacted content) or failed (what it wrote, then its redacted error, `aborted`
//!   ones included), with the project files the call may have changed
//!   ([`call_touched`](crate::opencode_server_v2::tools::call_touched)). Content shows as after
//!   resume ([`content_text`], [`failure_text`]). A call that ends without having been called
//!   (interrupted while its input streamed) starts and ends at once.
//! - **Shell output:** a `shell` call reports only its shell (`metadata.shellID`, ignored from any
//!   other tool). The engine reads that shell's output while the call runs
//!   ([`Mapper::take_shells`]) and hands it in ([`Mapper::shell_output`]), so it streams as
//!   `toolUpdate` as OpenCode 1 streamed `bash`.
//! - A call still running when its turn ends fails with [`UNFINISHED_TOOL`]. A call the chat
//!   never saw named (its first events came before the chat followed the stream) is not shown,
//!   nor is one a session already had.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde_json::{Value, json};

use super::Mapper;
use crate::claude_stream::mapper::UNFINISHED_TOOL;
use crate::model::{AgentEvent, ChatBlock};
use crate::opencode_server_v2::events::{
    ToolCalled, ToolFailed, ToolInputDelta, ToolInputEnded, ToolInputStarted, ToolProgress, ToolSuccess,
};
use crate::opencode_server_v2::tools::{call_touched, content_text, failure_text};
use crate::pi_family::normalize::tool_result;
use crate::security::redact_text;

/// The tool whose calls run in a shell the engine reads (`metadata.shellID`).
const SHELL_TOOL: &str = "shell";

/// The calls of the running turn (see the module docs).
pub(super) struct Tools {
    project: PathBuf,
    directory: PathBuf,
    calls: HashMap<String, Call>,
    /// Call ids in arrival order.
    order: Vec<String>,
    /// Calls of earlier turns, and of messages the session already had.
    done: HashSet<String>,
    /// Shells of running calls the engine does not read yet.
    shells: Vec<ShellCall>,
}

/// The shell a running `shell` call runs its command in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCall {
    /// The tool call.
    pub call: String,
    /// The shell (`sh_…`).
    pub shell: String,
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
    /// The call's shell is known.
    shell: bool,
}

impl Tools {
    pub(super) fn new(project: PathBuf, directory: PathBuf) -> Self {
        Self {
            project,
            directory,
            calls: HashMap::new(),
            order: Vec::new(),
            done: HashSet::new(),
            shells: Vec::new(),
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
                shell: false,
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

    /// `session.tool.progress`: new partial output, when the tool reports one, or the shell of a
    /// `shell` call, for the engine to read (only a `shell` call names a shell the chat reads).
    pub(super) fn progress(&mut self, progress: &ToolProgress, out: &mut Vec<AgentEvent>) {
        let shell = progress.metadata.get("shellID").and_then(Value::as_str);

        if let Some(shell) = shell
            && let Some(call) = self
                .running(&progress.id)
                .filter(|call| call.name == SHELL_TOOL && !call.shell)
        {
            call.shell = true;
            self.shells.push(ShellCall {
                call: progress.id.clone(),
                shell: shell.to_owned(),
            });
        }

        if let Some(output) = progress.metadata.get("output").and_then(Value::as_str) {
            self.partial(&progress.id, output, out);
        }
    }

    pub(super) fn success(&mut self, success: &ToolSuccess, out: &mut Vec<AgentEvent>) {
        self.finish(&success.id, &content_text(&success.content), false, out);
    }

    /// `session.tool.failed`: what the call wrote (else what streamed while it ran), then the
    /// error.
    pub(super) fn failed(&mut self, failed: &ToolFailed, out: &mut Vec<AgentEvent>) {
        let streamed = self.calls.get(&failed.id).and_then(|call| call.output.as_deref());
        let output = failure_text(&failed.error.message, &failed.content, streamed);

        self.finish(&failed.id, &output, true, out);
    }

    /// Sends `output` as call `id`'s partial output when it is new.
    fn partial(&mut self, id: &str, output: &str, out: &mut Vec<AgentEvent>) {
        let Some(call) = self.running(id) else {
            return;
        };

        if call.output.as_deref() == Some(output) {
            return;
        }

        call.output = Some(output.to_owned());
        out.push(AgentEvent::ToolUpdate {
            tool_call_id: id.to_owned(),
            partial: tool_result(Some(&Value::String(redact_text(output)))),
        });
    }

    /// Call `id` while it runs (started, not ended).
    fn running(&mut self, id: &str) -> Option<&mut Call> {
        self.calls.get_mut(id).filter(|call| call.started && !call.ended)
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
        self.shells.clear();
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

    /// The tool id of call `id`.
    pub(super) fn name(&self, id: &str) -> Option<&str> {
        self.calls.get(id).map(|call| call.name.as_str())
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

impl Mapper {
    /// The shells of running `shell` calls the engine should read now, each handed out once.
    pub fn take_shells(&mut self) -> Vec<ShellCall> {
        std::mem::take(&mut self.tools.shells)
    }

    /// True while call `call` runs: its shell's output is still wanted.
    pub fn shell_wanted(&self, call: &str) -> bool {
        self.tools
            .calls
            .get(call)
            .is_some_and(|call| call.started && !call.ended)
    }

    /// The output of call `call`'s shell so far: a `toolUpdate` when it is new and the call
    /// still runs.
    pub fn shell_output(&mut self, call: &str, output: &str) -> Vec<AgentEvent> {
        let mut out = Vec::new();

        if !output.is_empty() {
            self.tools.partial(call, output, &mut out);
        }

        out
    }
}
