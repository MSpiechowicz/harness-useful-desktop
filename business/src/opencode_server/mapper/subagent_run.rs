//! One subagent run of a chat, as the subagent panel shows it. Shared by the OpenCode 1 and
//! OpenCode 2 mappers: they read their own events and feed a run through [`Run`]'s methods; the
//! run turns that into [`AgentEvent::Subagent`] snapshots and [`AgentEvent::SubagentStep`]s.
//!
//! - A snapshot is sent only when it differs from the last one, and carries the task once.
//! - A step is sent when it is new or changed state: a tool call arrives `Running`, then `Done` or
//!   `Failed` with the end of its output. A finished step never changes again.
//! - Every text is masked (the server password, if the chat has one) and redacted before it is
//!   cut ([`bounded_masked`], [`tail_masked`]), so no cut leaves a piece of a secret.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use serde_json::Value;

use super::subagent_state::CallInput;
use crate::model::{
    AgentEvent, MAX_STEP_DETAIL_CHARS, MAX_STEP_OUTPUT_BYTES, MAX_STEP_OUTPUT_LINES, MAX_STEP_TEXT_CHARS,
    MAX_SUBAGENT_TEXT_CHARS, Subagent, SubagentRoute, SubagentStatus, SubagentStep, SubagentStepKind,
    SubagentStepStatus,
};
use crate::security::{bounded_masked, tail_masked};

/// Shown as the agent of a subagent whose agent nobody named.
const UNNAMED_AGENT: &str = "subagent";

/// Shown as the name of a tool call nobody named.
const UNNAMED_TOOL: &str = "tool";

/// Steps kept (to tell a repeat from a change) per run; later ones are not reported.
const MAX_STEPS: usize = 2_000;

/// Longest id, agent, model or provider kept (they come from the server and the model).
const MAX_LABEL_CHARS: usize = 240;

/// Arguments that tell most about a tool call, most telling first.
const PRIMARY_ARGS: [&str; 9] = [
    "command",
    "filePath",
    "path",
    "pattern",
    "url",
    "query",
    "description",
    "prompt",
    "name",
];

const RESULT_OPEN: &str = "<task_result>";
const RESULT_CLOSE: &str = "</task_result>";

/// A tool call of a subagent that has not ended.
struct RunningTool {
    id: String,
    name: String,
    detail: String,
    at: Option<u64>,
}

/// What the run last sent, so a repeat is not sent again.
#[derive(Default)]
struct Sent {
    task: bool,
    snapshot: Option<Subagent>,
}

/// A subagent run (see the module docs).
pub(crate) struct Run {
    pub(crate) id: String,
    /// The tool call of the parent that started it.
    pub(crate) call: Option<String>,
    pub(crate) index: u32,
    pub(crate) label: Option<String>,
    /// The agent the parent's call asked for (model-written, so shown but never trusted).
    pub(crate) agent: Option<String>,
    /// The agent the server says the subagent's replies come from.
    reported_agent: Option<String>,
    /// The agent and model exactly as the server reported them, kept only when they fit and no
    /// masking or redaction touched them: a message is routed with these, never the shown copies.
    route_agent: Option<String>,
    route_model: Option<(String, String)>,
    pub(crate) prompt: Option<String>,
    secret: Option<Arc<str>>,
    model: Option<(String, String)>,
    status: SubagentStatus,
    report: Option<String>,
    last_text: Option<String>,
    /// The state and a hash of the text last reported per step (only to tell a repeat).
    steps: HashMap<String, (SubagentStepStatus, u64)>,
    running: Vec<RunningTool>,
    tool_count: u32,
    /// Tokens and cost per assistant message (a message is reported again as it grows).
    usage: HashMap<String, (u64, f64)>,
    context_tokens: Option<u64>,
    assistant: HashSet<String>,
    sent: Sent,
}

impl Run {
    pub(crate) fn new(id: &str, index: u32, secret: Option<Arc<str>>) -> Self {
        Self {
            id: id.to_owned(),
            call: None,
            index,
            label: None,
            agent: None,
            reported_agent: None,
            route_agent: None,
            route_model: None,
            prompt: None,
            secret,
            model: None,
            status: SubagentStatus::Running,
            report: None,
            last_text: None,
            steps: HashMap::new(),
            running: Vec::new(),
            tool_count: 0,
            usage: HashMap::new(),
            context_tokens: None,
            assistant: HashSet::new(),
            sent: Sent::default(),
        }
    }

    /// Takes what the parent's call `id` says about the subagent.
    pub(crate) fn apply_call(&mut self, id: &str, input: CallInput) {
        self.call = Some(self.bound(id));
        self.label = input.description.or(self.label.take());
        self.prompt = input.prompt.or(self.prompt.take());
        self.agent = input.agent.or(self.agent.take());
    }

    /// True while the subagent works (or waits to).
    pub(crate) fn is_active(&self) -> bool {
        matches!(self.status, SubagentStatus::Pending | SubagentStatus::Running)
    }

    /// `text` masked, redacted and cut to the longest label.
    fn bound(&self, text: &str) -> String {
        bounded_masked(text, self.secret.as_deref(), MAX_LABEL_CHARS)
    }

    /// The agent the server reported for the subagent's session and its model, when known and
    /// usable as they are (not cut, masked or redacted). The agent the parent's call asked for
    /// does not count: a message must not run as an agent a model chose.
    pub(crate) fn settings(&self) -> (Option<&str>, Option<(&str, &str)>) {
        let model = self
            .route_model
            .as_ref()
            .map(|(provider, id)| (provider.as_str(), id.as_str()));

        (self.route_agent.as_deref(), model)
    }

    /// Whether message `id` is one of the subagent's own replies (not the brief it was given).
    pub(crate) fn is_reply(&self, id: &str) -> bool {
        self.assistant.contains(id)
    }

    /// Notes that message `id` is a reply of the subagent, written by `agent` with `model`.
    pub(crate) fn note_reply(&mut self, id: &str, agent: Option<String>, model: Option<(String, String)>) {
        if self.assistant.len() < MAX_STEPS {
            self.assistant.insert(id.to_owned());
        }

        // A reported value that the display copy would change cannot be sent back as it is, and
        // must not be kept as it is: the routing then lacks it (and the agent with a bad model).
        let unchanged = |text: &str| self.bound(text) == text;
        let route_model = model
            .as_ref()
            .filter(|(provider, id)| unchanged(provider) && unchanged(id));
        let model_unusable = model.is_some() && route_model.is_none();
        let route_model = route_model.cloned();
        let route_agent = agent.as_ref().filter(|agent| unchanged(agent)).cloned();
        let agent_unusable = agent.is_some() && route_agent.is_none();

        let shown_model = model.map(|(provider, id)| (self.bound(&provider), self.bound(&id)));

        self.reported_agent = agent.map(|agent| self.bound(&agent)).or(self.reported_agent.take());
        self.model = shown_model.or(self.model.take());

        if model_unusable || agent_unusable {
            self.route_agent = None;
            self.route_model = None;
        } else {
            self.route_agent = route_agent.or(self.route_agent.take());
            self.route_model = route_model.or(self.route_model.take());
        }
    }

    /// The tokens and cost of reply `message`; `context` is the context size when it has output.
    pub(crate) fn note_usage(&mut self, message: &str, tokens: u64, cost: f64, context: Option<u64>) {
        if self.usage.len() < MAX_STEPS || self.usage.contains_key(message) {
            self.usage.insert(message.to_owned(), (tokens, cost));
        }

        self.context_tokens = context.or(self.context_tokens);
    }

    /// Moves the run to `status`. A run that ended stays ended unless it works again.
    pub(crate) fn set_status(&mut self, status: SubagentStatus, out: &mut Vec<AgentEvent>) {
        let ends = !matches!(status, SubagentStatus::Pending | SubagentStatus::Running);

        if ends && self.is_active() {
            self.status = status;
            self.fail_running(out);
        } else if !ends {
            self.status = status;
        }
    }

    /// The parent's call returned `output` (the subagent's answer).
    pub(crate) fn set_report(&mut self, output: &str) {
        let report = task_report(output);

        if !report.is_empty() {
            self.report = Some(bounded_masked(report, self.secret.as_deref(), MAX_SUBAGENT_TEXT_CHARS));
        }
    }

    /// Notes the end of a piece of the subagent's own prose, and reports it as a step.
    pub(crate) fn text(&mut self, id: &str, text: &str, at: Option<u64>, out: &mut Vec<AgentEvent>) {
        if text.trim().is_empty() {
            return;
        }

        let secret = self.secret.clone();

        self.last_text = Some(bounded_masked(text, secret.as_deref(), MAX_SUBAGENT_TEXT_CHARS));
        self.step(text_step(secret.as_deref(), id, text, at), out);
    }

    /// Reports `step` unless it repeats the last one of its id.
    pub(crate) fn step(&mut self, mut step: SubagentStep, out: &mut Vec<AgentEvent>) {
        // A later event of a call without its arguments keeps the preview an earlier one gave.
        let earlier = self
            .running
            .iter()
            .find(|tool| tool.id == step.id && !tool.detail.is_empty());

        if step.kind == SubagentStepKind::Tool
            && step.detail.is_empty()
            && let Some(tool) = earlier
        {
            step.detail = tool.detail.clone();
        }

        let known = self.steps.get(&step.id);
        let fingerprint = fingerprint(&step.detail);

        match known {
            Some((SubagentStepStatus::Done | SubagentStepStatus::Failed, _)) => return,
            Some((status, seen)) if *status == step.status && *seen == fingerprint => return,
            None if self.steps.len() >= MAX_STEPS => return,
            None if step.kind == SubagentStepKind::Tool => self.tool_count += 1,
            _ => {}
        }

        self.steps.insert(step.id.clone(), (step.status, fingerprint));
        self.running.retain(|tool| tool.id != step.id);

        if step.status == SubagentStepStatus::Running {
            self.running.push(RunningTool {
                id: step.id.clone(),
                name: step.name.clone().unwrap_or_default(),
                detail: step.detail.clone(),
                at: step.at,
            });
        }

        out.push(AgentEvent::SubagentStep {
            subagent_id: self.id.clone(),
            step: Box::new(step),
        });
    }

    /// The name of the running tool call `id`.
    pub(crate) fn tool_name(&self, id: &str) -> String {
        let known = self.running.iter().find(|tool| tool.id == id);

        known.map_or_else(|| UNNAMED_TOOL.to_owned(), |tool| tool.name.clone())
    }

    /// Call `id` ended with `output`: its step keeps the name, preview and start it had running.
    pub(crate) fn tool_ended(
        &mut self,
        id: &str,
        failed: bool,
        output: Option<&str>,
        at: Option<u64>,
        out: &mut Vec<AgentEvent>,
    ) {
        let id = self.bound(id);
        let known = self.running.iter().find(|tool| tool.id == id);
        let (name, detail, started) = match known {
            Some(tool) => (tool.name.clone(), tool.detail.clone(), tool.at),
            None => (UNNAMED_TOOL.to_owned(), String::new(), None),
        };
        let status = if failed {
            SubagentStepStatus::Failed
        } else {
            SubagentStepStatus::Done
        };

        let secret = self.secret.clone();
        let mut step = tool_step(
            secret.as_deref(),
            &id,
            &name,
            &Value::Null,
            status,
            started.or(at),
            output,
        );
        step.detail = detail;

        self.step(step, out);
    }

    /// Ends the tool calls that never reported an end: the subagent is over.
    fn fail_running(&mut self, out: &mut Vec<AgentEvent>) {
        let ids: Vec<String> = self.running.iter().map(|tool| tool.id.clone()).collect();

        for id in ids {
            self.tool_ended(&id, true, None, None, out);
        }
    }

    /// Sends the snapshot when it differs from the last one.
    pub(crate) fn publish(&mut self, out: &mut Vec<AgentEvent>) {
        let snapshot = self.snapshot();

        if self.sent.snapshot.as_ref() == Some(&snapshot) {
            return;
        }

        let mut shown = snapshot.clone();

        if self.sent.task {
            shown.task = None;
        }
        self.sent.task |= snapshot.task.is_some();
        self.sent.snapshot = Some(snapshot);

        out.push(AgentEvent::Subagent {
            subagent: Box::new(shown),
        });
    }

    fn snapshot(&self) -> Subagent {
        let agent =
            (self.reported_agent.clone().or_else(|| self.agent.clone())).unwrap_or_else(|| UNNAMED_AGENT.to_owned());
        let mut subagent = Subagent::new(self.id.clone(), agent, self.status);

        subagent.parent_tool_call_id = self.call.clone();
        subagent.index = self.index;
        subagent.description = self.label.clone();
        subagent.label = self.label.clone();
        subagent.task = self.prompt.clone();
        subagent.model = self.model.as_ref().map(|(provider, id)| format!("{provider}/{id}"));
        subagent.report = self.report.clone().or_else(|| self.ended_text());
        subagent.message_route = Some(SubagentRoute::Direct);
        subagent.can_stop = Some(self.is_active());

        if let Some(tool) = self.running.last() {
            subagent.current_tool = Some(tool.name.clone());
            subagent.current_args = Some(tool.detail.clone());
        }

        if self.tool_count > 0 {
            subagent.tool_count = Some(self.tool_count);
        }

        if !self.usage.is_empty() {
            subagent.tokens = Some(self.usage.values().map(|(tokens, _)| tokens).sum());
            subagent.cost_usd = Some(self.usage.values().map(|(_, cost)| cost).sum());
        }
        subagent.context_tokens = self.context_tokens;

        subagent
    }

    /// The subagent's last words, as its report when it ended without a better one.
    fn ended_text(&self) -> Option<String> {
        let finished = self.status == SubagentStatus::Completed;

        self.last_text.clone().filter(|_| finished)
    }
}

/// The answer a subagent gave, from the output of the call that ran it: OpenCode wraps it in
/// `<task_result>`, after a line naming the session.
pub(crate) fn task_report(output: &str) -> &str {
    let Some((_, rest)) = output.split_once(RESULT_OPEN) else {
        return output.trim();
    };

    rest.rsplit_once(RESULT_CLOSE).map_or(rest, |(body, _)| body).trim()
}

/// A tool call as a step. Its output counts for a call that ended.
pub(crate) fn tool_step(
    secret: Option<&str>,
    id: &str,
    name: &str,
    input: &Value,
    status: SubagentStepStatus,
    at: Option<u64>,
    output: Option<&str>,
) -> SubagentStep {
    let ended = status != SubagentStepStatus::Running;
    let tail = output
        .filter(|text| ended && !text.trim().is_empty())
        .map(|text| tail_masked(text, secret, MAX_STEP_OUTPUT_LINES, MAX_STEP_OUTPUT_BYTES));

    SubagentStep {
        id: bounded_masked(id, secret, MAX_LABEL_CHARS),
        kind: SubagentStepKind::Tool,
        name: Some(bounded_masked(name, secret, MAX_STEP_DETAIL_CHARS)),
        detail: arg_preview(input, secret),
        status,
        at,
        output_lines: tail.as_ref().map(|(_, lines)| *lines),
        output: tail.map(|(text, _)| text),
    }
}

/// A piece of a subagent's own prose as a step.
pub(crate) fn text_step(secret: Option<&str>, id: &str, text: &str, at: Option<u64>) -> SubagentStep {
    SubagentStep {
        id: bounded_masked(id, secret, MAX_LABEL_CHARS),
        kind: SubagentStepKind::Text,
        name: None,
        detail: bounded_masked(text.trim(), secret, MAX_STEP_TEXT_CHARS),
        status: SubagentStepStatus::Done,
        at,
        output: None,
        output_lines: None,
    }
}

/// A one-line preview of a call's arguments: the most telling one, else all of them.
fn arg_preview(input: &Value, secret: Option<&str>) -> String {
    let primary = PRIMARY_ARGS
        .iter()
        .filter_map(|key| input.get(key).and_then(Value::as_str))
        .find(|text| !text.trim().is_empty());

    let text = match primary {
        Some(text) => text.to_owned(),
        None if input.as_object().is_some_and(|arguments| !arguments.is_empty()) => input.to_string(),
        None => String::new(),
    };
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");

    bounded_masked(&one_line, secret, MAX_STEP_DETAIL_CHARS)
}

/// A hash of a step's text, to tell a repeat without keeping the text.
fn fingerprint(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);

    hasher.finish()
}
