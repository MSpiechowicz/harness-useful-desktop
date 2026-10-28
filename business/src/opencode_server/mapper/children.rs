//! Subagents in the OpenCode 1 event stream: the child sessions a `task` call starts, shown in the
//! subagent panel ([`super::subagent_state`]).
//!
//! - **The call:** a `task` tool part of the chat's session (or of a subagent) holds the brief
//!   (`input.description`, `prompt`, `subagent_type`) and, once the child runs, its session in
//!   `state.metadata.sessionId`. Until then the child links to the one running `task` call that
//!   has none. The call's output is the subagent's answer.
//! - **The child:** its tool calls and finished texts are steps, its replies tell the model, the
//!   tokens and the cost, and `session.status` tells whether it works. The user message in the
//!   child is the brief, which comes from the call instead.

use serde_json::Value;

use super::Mapper;
use super::message::{part_type, text, tool_call_id};
use super::subagent_run::tool_step;
use super::subagent_state::{CallEnd, CallInput};
use crate::model::{AgentEvent, SubagentStatus, SubagentStepStatus};
use crate::opencode_server::wire::{Envelope, ModelRef, SessionInfo};

/// The tool that starts a subagent.
const TASK_TOOL: &str = "task";

/// The error OpenCode ends an aborted session with.
const ABORTED_ERROR: &str = "MessageAbortedError";

/// The agent and model a subagent's session works with.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubagentSettings {
    pub agent: Option<String>,
    pub model: Option<ModelRef>,
}

impl Mapper {
    /// Whether session `session` is a subagent that works; `None` for any other session.
    pub fn subagent_active(&self, session: &str) -> Option<bool> {
        self.runs.is_active(session)
    }

    /// The agent and model subagent `session` works with, when known.
    pub fn subagent_settings(&self, session: &str) -> Option<SubagentSettings> {
        let (agent, model) = self.runs.run(session)?.settings();

        Some(SubagentSettings {
            agent: agent.map(str::to_owned),
            model: model.map(|(provider, id)| ModelRef {
                provider: provider.to_owned(),
                id: id.to_owned(),
            }),
        })
    }

    /// A subagent session was started from session `parent`.
    pub(super) fn child_started(&mut self, info: &SessionInfo, out: &mut Vec<AgentEvent>) {
        self.runs.child_known(&info.id, info.parent_id.as_deref(), out);
    }

    /// A tool part of session `session`: a `task` call starts, runs and ends a subagent.
    pub(super) fn task_part(&mut self, session: &str, part: &Value, out: &mut Vec<AgentEvent>) {
        if part_type(part) != "tool" || text(part, "tool").as_deref() != Some(TASK_TOOL) {
            return;
        }

        let id = tool_call_id(part);
        let state = part.get("state").unwrap_or(&Value::Null);
        let input = state.get("input").unwrap_or(&Value::Null);

        let secret = self.runs.secret();
        let call = CallInput::read(input, "subagent_type", secret.as_deref());

        self.runs.call_started(session, &id, call, out);

        let child = state.pointer("/metadata/sessionId").and_then(Value::as_str);

        if let Some(child) = child.filter(|child| self.children.contains_key(*child)) {
            self.runs.call_linked(&id, child, out);
        }

        if let Some(end) = call_end(state) {
            self.runs.call_ended(&id, end, out);
        }
    }

    /// An event of subagent session `session`.
    pub(super) fn child_event(&mut self, session: &str, envelope: &Envelope, out: &mut Vec<AgentEvent>) {
        let properties = &envelope.properties;

        match envelope.kind.as_str() {
            "message.updated" => self.child_message(session, properties.get("info").unwrap_or(&Value::Null), out),
            "message.part.updated" => self.child_part(session, properties.get("part").unwrap_or(&Value::Null), out),
            "session.status" => {
                let kind = properties.pointer("/status/type").and_then(Value::as_str);
                self.child_status(session, kind, out);
            }
            "session.idle" => self.child_status(session, Some("idle"), out),
            "session.error" => {
                let aborted = properties.pointer("/error/name").and_then(Value::as_str) == Some(ABORTED_ERROR);
                let status = if aborted {
                    SubagentStatus::Aborted
                } else {
                    SubagentStatus::Failed
                };

                self.runs.update(session, out, |run, out| run.set_status(status, out));
            }
            _ => {}
        }
    }

    fn child_status(&mut self, session: &str, kind: Option<&str>, out: &mut Vec<AgentEvent>) {
        let status = match kind {
            Some("busy") => SubagentStatus::Running,
            Some("idle") => SubagentStatus::Completed,
            _ => return,
        };

        self.runs.update(session, out, |run, out| run.set_status(status, out));
    }

    /// A `message.updated` of a subagent: its replies tell the model and the usage.
    fn child_message(&mut self, session: &str, info: &Value, out: &mut Vec<AgentEvent>) {
        let Some(id) = text(info, "id").filter(|_| text(info, "role").as_deref() == Some("assistant")) else {
            return;
        };

        let agent = text(info, "agent");
        let model = text(info, "providerID").zip(text(info, "modelID"));
        let tokens = info.get("tokens").unwrap_or(&Value::Null);
        let output = count(tokens, "/output");
        let total = ["/input", "/output", "/reasoning", "/cache/read", "/cache/write"]
            .into_iter()
            .map(|pointer| count(tokens, pointer))
            .sum();
        let cost = info.get("cost").and_then(Value::as_f64).unwrap_or(0.0).max(0.0);

        self.runs.update(session, out, |run, _| {
            run.note_reply(&id, agent, model);
            run.note_usage(&id, total, cost, (output > 0).then_some(total));
        });
    }

    /// A `message.part.updated` of a subagent: a tool call or a finished text of its replies.
    fn child_part(&mut self, session: &str, part: &Value, out: &mut Vec<AgentEvent>) {
        match part_type(part) {
            "tool" => {
                self.task_part(session, part, out);
                self.child_tool(session, part, out);
            }
            "text" => self.child_text(session, part, out),
            _ => {}
        }
    }

    fn child_tool(&mut self, session: &str, part: &Value, out: &mut Vec<AgentEvent>) {
        let state = part.get("state").unwrap_or(&Value::Null);
        let input = state.get("input").unwrap_or(&Value::Null);
        let (status, output) = match state.get("status").and_then(Value::as_str) {
            Some("completed") => (SubagentStepStatus::Done, state.get("output")),
            Some("error") => (SubagentStepStatus::Failed, state.get("error")),
            _ => (SubagentStepStatus::Running, None),
        };

        let secret = self.runs.secret();
        let step = tool_step(
            secret.as_deref(),
            &tool_call_id(part),
            &text(part, "tool").unwrap_or_default(),
            input,
            status,
            state.pointer("/time/start").and_then(Value::as_u64),
            output.and_then(Value::as_str),
        );

        self.runs.step(session, step, out);
    }

    fn child_text(&mut self, session: &str, part: &Value, out: &mut Vec<AgentEvent>) {
        let hidden = ["synthetic", "ignored"]
            .iter()
            .any(|key| part.get(key).and_then(Value::as_bool) == Some(true));
        let ended = part.pointer("/time/end").is_some();

        let (Some(message), Some(id), Some(content)) = (text(part, "messageID"), text(part, "id"), text(part, "text"))
        else {
            return;
        };

        if hidden || !ended {
            return;
        }

        let at = part.pointer("/time/start").and_then(Value::as_u64);

        self.runs.update(session, out, |run, out| {
            if run.is_reply(&message) {
                run.text(&id, &content, at, out);
            }
        });
    }
}

/// How a `task` call ended, once it has.
fn call_end(state: &Value) -> Option<CallEnd> {
    match state.get("status").and_then(Value::as_str) {
        Some("completed") => Some(CallEnd::Done(text(state, "output"))),
        Some("error") => {
            let error = text(state, "error").unwrap_or_default();

            Some(CallEnd::Failed {
                aborted: error.to_lowercase().contains("abort"),
            })
        }
        _ => None,
    }
}

/// The non-negative whole number at `pointer` in `value`; 0 when absent.
fn count(value: &Value, pointer: &str) -> u64 {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .filter(|number| *number > 0.0)
        .map_or(0, |number| number as u64)
}
