//! Subagents in the OpenCode 2 event stream: the child sessions a `subagent` call starts, shown in
//! the subagent panel (shared with OpenCode 1, see
//! [`crate::opencode_server::mapper`]'s `subagent_state`).
//!
//! - **The call:** a `subagent` tool call of the chat's session (or of a subagent) holds the brief
//!   (`input.description`, `prompt`, `agent`); its progress names the child session
//!   (`metadata.sessionID`) and its result is the subagent's answer. A child links to the one
//!   running call that has no child yet when its start is seen before the progress.
//! - **The child:** its tool calls and finished texts are steps, its steps tell the agent, the model,
//!   the tokens and the cost, and its executions tell whether it works.

use serde_json::Value;

use super::Mapper;
use super::family::SUBAGENT_TOOL;
use crate::model::{AgentEvent, SubagentStatus, SubagentStepStatus};
use crate::opencode_server::mapper::{CallEnd, CallInput, tool_step};
use crate::opencode_server_v2::events::{Event, EventKind, SessionStatus};
use crate::opencode_server_v2::tools::{content_text, failure_text};
use crate::opencode_server_v2::wire::{ModelRef, Tokens};

/// The error kind of an interruption.
const ABORTED: &str = "aborted";

/// Why a run was interrupted that does not end the subagent: a newer input took its place.
const SUPERSEDED: &str = "superseded";

impl Mapper {
    /// Whether session `session` is a subagent that works; `None` for any other session.
    pub fn subagent_active(&self, session: &str) -> Option<bool> {
        self.runs.is_active(session)
    }

    /// A subagent session started from session `parent`.
    pub(super) fn child_started(&mut self, session: &str, parent: &str, out: &mut Vec<AgentEvent>) {
        self.runs.child_known(session, Some(parent), out);
    }

    /// Whether call `id` of session `session` runs a subagent.
    fn is_subagent_call(&self, session: &str, id: &str) -> bool {
        if session == self.session {
            return self.tools.name(id) == Some(SUBAGENT_TOOL);
        }

        self.runs.tool_name(session, id) == SUBAGENT_TOOL
    }

    /// A tool event of session `session`: a `subagent` call starts, runs and ends a subagent.
    pub(super) fn subagent_call(&mut self, session: &str, kind: &EventKind, out: &mut Vec<AgentEvent>) {
        match kind {
            EventKind::ToolCalled(called) if self.is_subagent_call(session, &called.id) => {
                let secret = self.runs.secret();
                let input = CallInput::read(&called.input, "agent", secret.as_deref());
                self.runs.call_started(session, &called.id, input, out);
            }
            EventKind::ToolProgress(progress) if self.is_subagent_call(session, &progress.id) => {
                let child = progress.metadata.get("sessionID").and_then(Value::as_str);

                if let Some(child) = child.filter(|child| !child.is_empty()) {
                    self.runs.call_linked(&progress.id, child, out);
                }
            }
            EventKind::ToolSuccess(success) if self.is_subagent_call(session, &success.id) => {
                let output = Some(content_text(&success.content));
                self.runs.call_ended(&success.id, CallEnd::Done(output), out);
            }
            EventKind::ToolFailed(failed) if self.is_subagent_call(session, &failed.id) => {
                let aborted = failed.error.kind == ABORTED;
                self.runs.call_ended(&failed.id, CallEnd::Failed { aborted }, out);
            }
            _ => {}
        }
    }

    /// An event of subagent session `session`.
    pub(super) fn child_event(&mut self, session: &str, event: &Event, out: &mut Vec<AgentEvent>) {
        self.runs.child_known(session, None, out);
        self.subagent_call(session, &event.kind, out);

        let at = event.created;

        match &event.kind {
            EventKind::ToolInputStarted(started) => {
                let secret = self.runs.secret();
                let step = tool_step(
                    secret.as_deref(),
                    &started.id,
                    &started.name,
                    &Value::Null,
                    SubagentStepStatus::Running,
                    at,
                    None,
                );
                self.runs.step(session, step, out);
            }
            // The whole input once it finished streaming: it names what the call works on before (or without) `called`.
            EventKind::ToolInputEnded(ended) => {
                let Ok(input) = serde_json::from_str::<Value>(&ended.text) else {
                    return;
                };
                let secret = self.runs.secret();

                self.runs.update(session, out, |run, out| {
                    let name = run.tool_name(&ended.id);
                    let status = SubagentStepStatus::Running;
                    let step = tool_step(secret.as_deref(), &ended.id, &name, &input, status, at, None);
                    run.step(step, out);
                });
            }
            EventKind::ToolCalled(called) => {
                let secret = self.runs.secret();

                self.runs.update(session, out, |run, out| {
                    let name = run.tool_name(&called.id);
                    let status = SubagentStepStatus::Running;
                    let step = tool_step(secret.as_deref(), &called.id, &name, &called.input, status, at, None);
                    run.step(step, out);
                });
            }
            EventKind::ToolSuccess(success) => {
                let output = content_text(&success.content);
                self.runs.update(session, out, |run, out| {
                    run.tool_ended(&success.id, false, Some(&output), at, out);
                });
            }
            EventKind::ToolFailed(failed) => {
                let output = failure_text(&failed.error.message, &failed.content, None);
                self.runs.update(session, out, |run, out| {
                    run.tool_ended(&failed.id, true, Some(&output), at, out);
                });
            }
            EventKind::TextEnded(ended) => {
                let id = format!("{}:{}", ended.assistant_message_id, ended.ordinal);
                self.runs
                    .update(session, out, |run, out| run.text(&id, &ended.text, at, out));
            }
            EventKind::StepStarted(step) => {
                let agent = Some(step.agent.clone()).filter(|agent| !agent.is_empty());
                let model = step.model.as_ref().map(model_pair);

                self.runs.update(session, out, |run, _| {
                    run.note_reply(&step.assistant_message_id, agent, model)
                });
            }
            EventKind::StepEnded(step) => {
                let total = token_total(&step.tokens);
                let context = (step.tokens.output > 0).then_some(total);

                self.runs.update(session, out, |run, _| {
                    run.note_usage(&step.assistant_message_id, total, step.cost.max(0.0), context);
                });
            }
            EventKind::Status(changed) => match changed.status {
                SessionStatus::Busy => self.child_status(session, SubagentStatus::Running, out),
                SessionStatus::Idle => self.child_status(session, SubagentStatus::Completed, out),
                _ => {}
            },
            EventKind::Idle(_) | EventKind::ExecutionSucceeded(_) => {
                self.child_status(session, SubagentStatus::Completed, out);
            }
            EventKind::ExecutionStarted(_) => self.child_status(session, SubagentStatus::Running, out),
            EventKind::ExecutionFailed(_) => self.child_status(session, SubagentStatus::Failed, out),
            EventKind::ExecutionInterrupted(stopped) if stopped.reason != SUPERSEDED => {
                self.child_status(session, SubagentStatus::Aborted, out);
            }
            _ => {}
        }
    }

    fn child_status(&mut self, session: &str, status: SubagentStatus, out: &mut Vec<AgentEvent>) {
        self.runs.update(session, out, |run, out| run.set_status(status, out));
    }
}

/// The provider and id of `model`, as the snapshot names a model.
fn model_pair(model: &ModelRef) -> (String, String) {
    (model.provider_id.clone(), model.id.clone())
}

/// Input, output, reasoning and cache tokens.
fn token_total(tokens: &Tokens) -> u64 {
    tokens.input + tokens.output + tokens.reasoning + tokens.cache.read + tokens.cache.write
}
