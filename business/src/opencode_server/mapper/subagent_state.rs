//! The subagents of a chat: their runs ([`Run`]) and the tool calls of their parents that started
//! them. Shared by the OpenCode 1 and OpenCode 2 mappers.
//!
//! A subagent is a child session. What it was asked (description, brief, agent) is in the parent's
//! tool call, which names the child once the child runs (its metadata). Until then a child links
//! to the one running call of its parent that has no child yet.

use std::sync::Arc;

use serde_json::Value;

use super::subagent_run::Run;
use crate::model::{AgentEvent, MAX_STEP_DETAIL_CHARS, MAX_SUBAGENT_TEXT_CHARS, SubagentStatus, SubagentStep};
use crate::security::bounded_masked;

/// Parent calls kept (the oldest finished one goes first).
const MAX_CALLS: usize = 256;

/// Runs tracked; children past it are not shown.
const MAX_RUNS: usize = 128;

/// Longest id kept: a child session whose id is longer is not tracked.
const MAX_ID_CHARS: usize = 240;

/// What a parent's call asked of its subagent (texts masked, redacted and bounded).
#[derive(Debug, Clone, Default)]
pub(crate) struct CallInput {
    pub description: Option<String>,
    pub prompt: Option<String>,
    pub agent: Option<String>,
}

impl CallInput {
    /// Reads `{description, prompt, <agent_key>}` of a call's input, with `secret` masked first.
    pub(crate) fn read(input: &Value, agent_key: &str, secret: Option<&str>) -> Self {
        let field = |key: &str, max: usize| {
            input
                .get(key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| bounded_masked(text, secret, max))
        };

        Self {
            description: field("description", MAX_STEP_DETAIL_CHARS),
            prompt: field("prompt", MAX_SUBAGENT_TEXT_CHARS),
            agent: field(agent_key, MAX_STEP_DETAIL_CHARS),
        }
    }

    /// Keeps what `later` says over what this knows.
    fn merge(&mut self, later: Self) {
        self.description = later.description.or(self.description.take());
        self.prompt = later.prompt.or(self.prompt.take());
        self.agent = later.agent.or(self.agent.take());
    }
}

/// How a parent's call ended.
pub(crate) enum CallEnd {
    /// It returned this output.
    Done(Option<String>),
    /// It failed; `aborted` when the subagent was stopped.
    Failed { aborted: bool },
}

struct Call {
    id: String,
    /// The session whose model made the call.
    owner: String,
    input: CallInput,
    ended: bool,
    child: Option<String>,
}

/// The subagents of a chat (see the module docs).
#[derive(Default)]
pub(crate) struct Subagents {
    calls: Vec<Call>,
    runs: Vec<Run>,
    spawned: u32,
    /// A password the texts are masked of before they are cut.
    secret: Option<Arc<str>>,
}

impl Subagents {
    /// Masks `secret` in every text from now on.
    pub(crate) fn set_secret(&mut self, secret: Option<&str>) {
        self.secret = secret.filter(|secret| !secret.is_empty()).map(Arc::from);
    }

    /// The password the texts are masked of, if any.
    pub(crate) fn secret(&self) -> Option<Arc<str>> {
        self.secret.clone()
    }

    /// Notes the parent call `id` made by session `owner` (again as its input grows).
    pub(crate) fn call_started(&mut self, owner: &str, id: &str, input: CallInput, out: &mut Vec<AgentEvent>) {
        match self.calls.iter_mut().find(|call| call.id == id) {
            Some(call) => call.input.merge(input),
            None => {
                self.make_room();
                self.calls.push(Call {
                    id: id.to_owned(),
                    owner: owner.to_owned(),
                    input,
                    ended: false,
                    child: None,
                });
            }
        }

        self.refresh(id, out);
    }

    /// Call `id` runs `child`.
    pub(crate) fn call_linked(&mut self, id: &str, child: &str, out: &mut Vec<AgentEvent>) {
        let Some(call) = self.calls.iter_mut().find(|call| call.id == id) else {
            return;
        };

        if call.child.as_deref() != Some(child) {
            call.child = Some(child.to_owned());
            self.refresh(id, out);
        }
    }

    /// Call `id` ended.
    pub(crate) fn call_ended(&mut self, id: &str, end: CallEnd, out: &mut Vec<AgentEvent>) {
        let Some(call) = self.calls.iter_mut().find(|call| call.id == id) else {
            return;
        };

        call.ended = true;
        let Some(child) = call.child.clone() else {
            return;
        };

        self.update(&child, out, |run, out| match &end {
            CallEnd::Done(output) => {
                if let Some(output) = output {
                    run.set_report(output);
                }
                run.set_status(SubagentStatus::Completed, out);
            }
            CallEnd::Failed { aborted: true } => run.set_status(SubagentStatus::Aborted, out),
            CallEnd::Failed { aborted: false } => run.set_status(SubagentStatus::Failed, out),
        });
    }

    /// Makes sure `child` (started from session `parent`, when known) has a run, and reports it.
    pub(crate) fn child_known(&mut self, child: &str, parent: Option<&str>, out: &mut Vec<AgentEvent>) {
        if self.runs.iter().any(|run| run.id == child) {
            return;
        }

        if self.runs.len() >= MAX_RUNS || child.chars().count() > MAX_ID_CHARS {
            return;
        }

        let mut run = Run::new(child, self.spawned, self.secret.clone());
        self.spawned += 1;

        if let Some(call) = self.call_of(child, parent) {
            run.apply_call(&call.id, call.input.clone());
        }

        self.runs.push(run);
        self.update(child, out, |_, _| {});
    }

    /// The call that started `child`: the one that named it, else the only call of `parent` that
    /// runs and has no child.
    fn call_of(&mut self, child: &str, parent: Option<&str>) -> Option<&Call> {
        if let Some(position) = self.calls.iter().position(|call| call.child.as_deref() == Some(child)) {
            return self.calls.get(position);
        }

        let mut open = self
            .calls
            .iter_mut()
            .filter(|call| !call.ended && call.child.is_none() && Some(call.owner.as_str()) == parent);
        let only = open.next().filter(|_| open.next().is_none())?;

        only.child = Some(child.to_owned());
        Some(only)
    }

    /// Passes what call `id` says to its run.
    fn refresh(&mut self, id: &str, out: &mut Vec<AgentEvent>) {
        let Some(call) = self.calls.iter().find(|call| call.id == id) else {
            return;
        };

        let Some(child) = call.child.clone() else {
            return;
        };

        let (id, input) = (call.id.clone(), call.input.clone());

        self.update(&child, out, |run, _| run.apply_call(&id, input));
    }

    /// Changes `child`'s run with `change`, then reports the run and what changed. A run is
    /// reported before its first step.
    pub(crate) fn update(
        &mut self,
        child: &str,
        out: &mut Vec<AgentEvent>,
        change: impl FnOnce(&mut Run, &mut Vec<AgentEvent>),
    ) {
        let Some(run) = self.runs.iter_mut().find(|run| run.id == child) else {
            return;
        };

        run.publish(out);
        change(run, out);
        run.publish(out);
    }

    /// Whether `child` is a subagent that works; `None` when it is not a known subagent.
    pub(crate) fn is_active(&self, child: &str) -> Option<bool> {
        self.runs.iter().find(|run| run.id == child).map(Run::is_active)
    }

    /// The run of subagent `child`.
    pub(crate) fn run(&self, child: &str) -> Option<&Run> {
        self.runs.iter().find(|run| run.id == child)
    }

    /// The name of tool call `id` of subagent `child`; empty when unknown.
    pub(crate) fn tool_name(&self, child: &str, id: &str) -> String {
        self.runs
            .iter()
            .find(|run| run.id == child)
            .map(|run| run.tool_name(id))
            .unwrap_or_default()
    }

    /// Reports `step` of `child`.
    pub(crate) fn step(&mut self, child: &str, step: SubagentStep, out: &mut Vec<AgentEvent>) {
        self.update(child, out, |run, out| run.step(step, out));
    }

    /// Drops the oldest finished call (else the oldest) once [`MAX_CALLS`] are kept.
    fn make_room(&mut self) {
        if self.calls.len() < MAX_CALLS {
            return;
        }

        let oldest = self.calls.iter().position(|call| call.ended).unwrap_or(0);
        self.calls.remove(oldest);
    }
}
