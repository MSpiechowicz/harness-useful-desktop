//! Subagents as the chat shows them, from the `system/task_*` frames of `Task` subagents
//! (`task_type: local_agent`, or a `subagent_type`, whatever the tool that spawned them is named).
//! Other tasks (a backgrounded shell, a monitor)
//! are not shown. The steps of a subagent come from its own frames (see [`super::subagent_steps`]).
//! Every text is redacted and bounded.

use std::collections::HashMap;

use super::subagent_steps::{SubagentSteps, Update};
use crate::claude_stream::wire::messages::{AssistantFrame, UserFrame};
use crate::claude_stream::wire::system::{TaskNotification, TaskProgress, TaskStarted, TaskUpdated, TaskUsage};
use crate::model::{AgentEvent, MAX_SUBAGENT_TEXT_CHARS, Subagent, SubagentRoute, SubagentStatus};
use crate::security::bounded_redacted;

/// The task type of a `Task` subagent.
const AGENT_TASK: &str = "local_agent";

/// Longest description, summary or tool name kept.
const LINE_CHARS: usize = 240;

/// Subagents remembered. Past it the oldest finished one is forgotten, and a new one is not
/// tracked when every one remembered still runs.
const MAX_AGENTS: usize = 256;

/// The subagents of the chat, by task id.
#[derive(Default)]
pub(super) struct Tasks {
    agents: HashMap<String, Known>,
    /// Subagents spawned per `Task` call, for their index.
    spawned: HashMap<String, u32>,
    /// The task of each spawning call (`parent_tool_use_id` of the subagent's frames).
    by_call: HashMap<String, String>,
    steps: SubagentSteps,
    /// Subagents started so far, to tell the oldest.
    started: u64,
}

/// What every snapshot of a subagent repeats.
struct Known {
    parent: Option<String>,
    agent: String,
    index: u32,
    /// The order the task started in.
    seq: u64,
    detached: bool,
    label: Option<String>,
    status: SubagentStatus,
    model: Option<String>,
    context_tokens: Option<u64>,
    /// The subagent's last prose, redacted and bounded: its report once it completes. Dropped once
    /// the subagent is over and the report was shown.
    last_text: Option<String>,
    /// A report went out: a later notification does not replace it with its one-line summary.
    reported: bool,
}

impl Tasks {
    pub(super) fn started(&mut self, task: &TaskStarted, out: &mut Vec<AgentEvent>) {
        let is_agent = task.task_type.as_deref() == Some(AGENT_TASK) || task.subagent_type.is_some();

        if !is_agent || task.task_id.is_empty() {
            return;
        }

        if !self.agents.contains_key(&task.task_id) && !self.make_room() {
            self.refuse_call(task.tool_use_id.as_deref());
            return;
        }

        let index = match &task.tool_use_id {
            Some(call) => {
                let count = self.spawned.entry(call.clone()).or_default();
                *count += 1;
                *count - 1
            }
            None => 0,
        };

        let known = Known {
            parent: task.tool_use_id.clone(),
            agent: bounded_redacted(task.subagent_type.as_deref().unwrap_or("task"), LINE_CHARS),
            index,
            seq: self.started,
            detached: task.is_backgrounded,
            label: text(task.description.as_deref(), LINE_CHARS),
            status: SubagentStatus::Running,
            model: None,
            context_tokens: None,
            last_text: None,
            reported: false,
        };

        self.started += 1;

        let mut subagent = snapshot(&task.task_id, &known);
        subagent.description = known.label.clone();
        subagent.task = text(task.prompt.as_deref(), MAX_SUBAGENT_TEXT_CHARS);
        out.push(event(subagent));
        self.agents.insert(task.task_id.clone(), known);

        if let Some(call) = &task.tool_use_id {
            self.steps.accept(call);
            self.by_call.insert(call.clone(), task.task_id.clone());

            for updates in self.steps.take(call) {
                self.apply(&task.task_id, updates, out);
            }
        }
    }

    /// The task of call `call` was refused: nothing may wait for it, unless another task of the
    /// same call is tracked (its frames then go there).
    fn refuse_call(&mut self, call: Option<&str>) {
        if let Some(call) = call.filter(|call| !self.by_call.contains_key(*call)) {
            self.steps.refuse(call);
        }
    }

    /// The `assistant` frame of the subagent spawned by call `parent`.
    pub(super) fn child_assistant(&mut self, parent: &str, frame: &AssistantFrame, out: &mut Vec<AgentEvent>) {
        let updates = self.steps.assistant(parent, frame);
        self.route(parent, updates, out);
    }

    /// The `user` frame (tool results) of the subagent spawned by call `parent`.
    pub(super) fn child_user(&mut self, parent: &str, frame: &UserFrame, out: &mut Vec<AgentEvent>) {
        let updates = self.steps.user(frame);
        self.route(parent, updates, out);
    }

    /// Whether subagent `id` is still running: `None` for a subagent the chat never saw.
    pub(super) fn is_active(&self, id: &str) -> Option<bool> {
        self.agents.get(id).map(|known| is_active(known.status))
    }

    fn route(&mut self, parent: &str, updates: Vec<Update>, out: &mut Vec<AgentEvent>) {
        match self.by_call.get(parent).cloned() {
            Some(task_id) => self.apply(&task_id, updates, out),
            None => self.steps.hold(parent, updates),
        }
    }

    /// Shows the steps of `updates` and a snapshot when the model or the context size changed (or
    /// the final text of a completed subagent came late).
    fn apply(&mut self, task_id: &str, updates: Vec<Update>, out: &mut Vec<AgentEvent>) {
        let Some(known) = self.agents.get_mut(task_id) else {
            return;
        };

        let id = bounded_redacted(task_id, LINE_CHARS);
        let mut changed = false;
        let mut texted = false;

        for update in updates {
            if let Some(step) = update.step() {
                out.push(AgentEvent::SubagentStep {
                    subagent_id: id.clone(),
                    step: Box::new(step),
                });
            }

            match update {
                Update::Text { text, .. } => {
                    known.last_text = Some(text);
                    texted = true;
                }
                Update::Model(model) if known.model.as_ref() != Some(&model) => {
                    known.model = Some(model);
                    changed = true;
                }
                Update::Context(tokens) if known.context_tokens != Some(tokens) => {
                    known.context_tokens = Some(tokens);
                    changed = true;
                }
                Update::Tool(_) | Update::Model(_) | Update::Context(_) => {}
            }
        }

        let late_report = texted && known.status == SubagentStatus::Completed;

        if changed || late_report {
            let mut subagent = snapshot(task_id, known);
            subagent.report = late_report.then(|| known.last_text.clone()).flatten();
            known.reported |= subagent.report.is_some();
            out.push(event(subagent));
        }

        // The report went out, or never will (the subagent failed or was stopped): the text is
        // not kept. A completed subagent keeps it until its notification brings the report.
        if late_report || matches!(known.status, SubagentStatus::Failed | SubagentStatus::Aborted) {
            known.last_text = None;
        }
    }

    /// Makes room for one more subagent by forgetting the oldest finished one; false when every
    /// subagent remembered still runs.
    fn make_room(&mut self) -> bool {
        if self.agents.len() < MAX_AGENTS {
            return true;
        }

        let oldest = self
            .agents
            .iter()
            .filter(|(_, known)| !is_active(known.status))
            .min_by_key(|(_, known)| known.seq)
            .map(|(id, _)| id.clone());

        let Some(oldest) = oldest.and_then(|id| self.agents.remove(&id).map(|known| (id, known))) else {
            return false;
        };

        self.forget_call(&oldest.0, oldest.1.parent.as_deref());

        true
    }

    /// Forgets what the spawning call `parent` of the forgotten subagent `id` left, unless another
    /// subagent remembered came from it.
    fn forget_call(&mut self, id: &str, parent: Option<&str>) {
        let Some(parent) = parent else {
            return;
        };

        if self.by_call.get(parent).is_some_and(|task| task == id) {
            self.by_call.remove(parent);
        }

        if !self
            .agents
            .values()
            .any(|known| known.parent.as_deref() == Some(parent))
        {
            self.spawned.remove(parent);
        }
    }

    pub(super) fn progress(&mut self, task: &TaskProgress, out: &mut Vec<AgentEvent>) {
        let Some(known) = self.agents.get_mut(&task.task_id) else {
            return;
        };

        // A progress frame that arrives after the end is stale: it must not revive the subagent.
        if is_final(known.status) {
            return;
        }

        known.status = SubagentStatus::Running;

        let mut subagent = snapshot(&task.task_id, known);
        subagent.description = text(task.description.as_deref(), LINE_CHARS);
        subagent.intent = text(task.summary.as_deref(), LINE_CHARS);
        subagent.current_tool = text(task.last_tool_name.as_deref(), LINE_CHARS);
        add_usage(&mut subagent, task.usage);
        out.push(event(subagent));
    }

    pub(super) fn updated(&mut self, task: &TaskUpdated, out: &mut Vec<AgentEvent>) {
        let Some(known) = self.agents.get_mut(&task.task_id) else {
            return;
        };

        if let Some(detached) = task.patch.is_backgrounded {
            known.detached = detached;
        }

        // A patch without a (known) status, such as a changed description, keeps the status.
        known.status = match task.patch.status.as_deref() {
            Some("pending" | "paused") => SubagentStatus::Pending,
            Some("running") => SubagentStatus::Running,
            Some("completed") => SubagentStatus::Completed,
            Some("failed") => SubagentStatus::Failed,
            Some("killed") => SubagentStatus::Aborted,
            _ => known.status,
        };

        let mut subagent = snapshot(&task.task_id, known);
        subagent.description = text(task.patch.description.as_deref(), LINE_CHARS);
        subagent.intent = text(task.patch.error.as_deref(), LINE_CHARS);
        out.push(event(subagent));
    }

    pub(super) fn ended(&mut self, task: &TaskNotification, out: &mut Vec<AgentEvent>) {
        let Some(known) = self.agents.get_mut(&task.task_id) else {
            return;
        };

        known.status = match task.status.as_deref() {
            Some("failed") => SubagentStatus::Failed,
            Some("stopped") => SubagentStatus::Aborted,
            _ => SubagentStatus::Completed,
        };

        let mut subagent = snapshot(&task.task_id, known);
        subagent.intent = text(task.summary.as_deref(), LINE_CHARS);

        if known.status == SubagentStatus::Completed {
            // The summary stands in only while no report went out: a real one is never replaced.
            let summary = (!known.reported)
                .then(|| text(task.summary.as_deref(), MAX_SUBAGENT_TEXT_CHARS))
                .flatten();
            subagent.report = known.last_text.take().or(summary);
            known.reported |= subagent.report.is_some();
        }

        known.last_text = None;
        add_usage(&mut subagent, task.usage);
        out.push(event(subagent));
    }
}

/// What every snapshot says: the subagent's status and, once known, its model and context size.
/// The panel messages it through the orchestrator, and stops it while it runs.
fn snapshot(id: &str, known: &Known) -> Subagent {
    Subagent {
        parent_tool_call_id: known.parent.clone(),
        index: known.index,
        detached: known.detached,
        label: known.label.clone(),
        model: known.model.clone(),
        context_tokens: known.context_tokens,
        message_route: Some(SubagentRoute::Orchestrator),
        can_stop: Some(is_active(known.status)),
        ..Subagent::new(bounded_redacted(id, LINE_CHARS), known.agent.clone(), known.status)
    }
}

fn is_final(status: SubagentStatus) -> bool {
    matches!(
        status,
        SubagentStatus::Completed | SubagentStatus::Failed | SubagentStatus::Aborted
    )
}

fn is_active(status: SubagentStatus) -> bool {
    matches!(status, SubagentStatus::Pending | SubagentStatus::Running)
}

fn add_usage(subagent: &mut Subagent, usage: Option<TaskUsage>) {
    let Some(usage) = usage else {
        return;
    };

    subagent.tool_count = Some(u32::try_from(usage.tool_uses).unwrap_or(u32::MAX));
    subagent.tokens = Some(usage.total_tokens);
    subagent.duration_ms = Some(usage.duration_ms);
}

fn event(subagent: Subagent) -> AgentEvent {
    AgentEvent::Subagent {
        subagent: Box::new(subagent),
    }
}

fn text(value: Option<&str>, max: usize) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| bounded_redacted(value, max))
}
