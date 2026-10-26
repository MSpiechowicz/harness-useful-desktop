//! Subagents as the chat shows them, from the `system/task_*` frames of `Task` subagents
//! (`task_type: local_agent`, or a `subagent_type`). Other tasks (a backgrounded shell, a monitor)
//! and the subagents' own messages are not shown. Every text is redacted and bounded.

use std::collections::HashMap;

use crate::claude_stream::wire::system::{TaskNotification, TaskProgress, TaskStarted, TaskUpdated, TaskUsage};
use crate::model::{AgentEvent, Subagent, SubagentStatus};
use crate::security::redact_text;

/// The task type of a `Task` subagent.
const AGENT_TASK: &str = "local_agent";

/// Longest prompt kept.
const PROMPT_CHARS: usize = 600;

/// Longest description, summary or tool name kept.
const LINE_CHARS: usize = 240;

/// The subagents of the chat, by task id.
#[derive(Default)]
pub(super) struct Tasks {
    agents: HashMap<String, Known>,
    /// Subagents spawned per `Task` call, for their index.
    spawned: HashMap<String, u32>,
}

/// What every snapshot of a subagent repeats.
#[derive(Clone)]
struct Known {
    parent: Option<String>,
    agent: String,
    index: u32,
    detached: bool,
}

impl Tasks {
    pub(super) fn started(&mut self, task: &TaskStarted, out: &mut Vec<AgentEvent>) {
        let is_agent = task.task_type.as_deref() == Some(AGENT_TASK) || task.subagent_type.is_some();

        if !is_agent || task.task_id.is_empty() {
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
            agent: bounded(task.subagent_type.as_deref().unwrap_or("task"), LINE_CHARS),
            index,
            detached: task.is_backgrounded,
        };
        self.agents.insert(task.task_id.clone(), known.clone());

        let mut subagent = snapshot(&task.task_id, &known, SubagentStatus::Running);
        subagent.description = text(task.description.as_deref(), LINE_CHARS);
        subagent.task = text(task.prompt.as_deref(), PROMPT_CHARS);
        out.push(event(subagent));
    }

    pub(super) fn progress(&mut self, task: &TaskProgress, out: &mut Vec<AgentEvent>) {
        let Some(known) = self.agents.get(&task.task_id) else {
            return;
        };

        let mut subagent = snapshot(&task.task_id, known, SubagentStatus::Running);
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

        let status = match task.patch.status.as_deref() {
            Some("pending" | "paused") => SubagentStatus::Pending,
            Some("completed") => SubagentStatus::Completed,
            Some("failed") => SubagentStatus::Failed,
            Some("killed") => SubagentStatus::Aborted,
            _ => SubagentStatus::Running,
        };

        let mut subagent = snapshot(&task.task_id, known, status);
        subagent.description = text(task.patch.description.as_deref(), LINE_CHARS);
        subagent.intent = text(task.patch.error.as_deref(), LINE_CHARS);
        out.push(event(subagent));
    }

    pub(super) fn ended(&mut self, task: &TaskNotification, out: &mut Vec<AgentEvent>) {
        let Some(known) = self.agents.get(&task.task_id) else {
            return;
        };

        let status = match task.status.as_deref() {
            Some("failed") => SubagentStatus::Failed,
            Some("stopped") => SubagentStatus::Aborted,
            _ => SubagentStatus::Completed,
        };

        let mut subagent = snapshot(&task.task_id, known, status);
        subagent.intent = text(task.summary.as_deref(), LINE_CHARS);
        add_usage(&mut subagent, task.usage);
        out.push(event(subagent));
    }
}

fn snapshot(id: &str, known: &Known, status: SubagentStatus) -> Subagent {
    Subagent {
        id: bounded(id, LINE_CHARS),
        parent_tool_call_id: known.parent.clone(),
        agent: known.agent.clone(),
        status,
        index: known.index,
        detached: known.detached,
        description: None,
        task: None,
        intent: None,
        current_tool: None,
        current_args: None,
        recent_tools: None,
        recent_output: None,
        tool_count: None,
        tokens: None,
        context_tokens: None,
        context_window: None,
        cost_usd: None,
        duration_ms: None,
        model: None,
    }
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
        .map(|value| bounded(value, max))
}

/// `value` redacted and cut to `max` characters, the cut marked with an ellipsis.
fn bounded(value: &str, max: usize) -> String {
    let redacted = redact_text(value);

    match redacted.char_indices().nth(max.saturating_sub(1)) {
        Some((cut, _)) if redacted.chars().count() > max => format!("{}…", &redacted[..cut]),
        _ => redacted,
    }
}
