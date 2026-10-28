//! The steps of subagents, from the `assistant` and `user` frames they write (`parent_tool_use_id`
//! names the `Agent` or `Task` call that spawned them, which `system/task_started` ties to a task).
//!
//! - A `tool_use` block is a running tool step (its preview is the call's primary argument), the
//!   `tool_result` of that call ends it with the tail of its output, and a `text` block is a prose
//!   step. The model and the context size of each API call become snapshot fields.
//! - The frames come in any order: a frame whose call has no task yet waits (at most
//!   [`MAX_WAITING_FRAMES`] per call and [`MAX_WAITING_CALLS`] calls, then it is dropped) and is
//!   applied once the task starts. Frames that arrive after the parent's `result` are applied the
//!   same way: they never touch the chat's turn.
//! - Every text is redacted before it is cut. A subagent's `stream_event` frames are not read: the
//!   complete `assistant` frames carry the same blocks.

use std::collections::{HashMap, HashSet, VecDeque};
use std::str::FromStr;

use jiff::Timestamp;
use serde_json::Value;

use super::tools::{primary_arg, result_text};
use crate::claude_stream::wire::messages::{AssistantFrame, ContentBlock, UserFrame};
use crate::model::{
    MAX_STEP_DETAIL_CHARS, MAX_STEP_OUTPUT_BYTES, MAX_STEP_OUTPUT_LINES, MAX_STEP_TEXT_CHARS, MAX_SUBAGENT_TEXT_CHARS,
    SubagentStep, SubagentStepKind, SubagentStepStatus,
};
use crate::security::{bounded_redacted, redact_text, tail_redacted};

/// Frames kept per `Agent` call whose task has not started yet.
pub(super) const MAX_WAITING_FRAMES: usize = 32;

/// `Agent` calls with waiting frames; frames of any further call are dropped.
pub(super) const MAX_WAITING_CALLS: usize = 8;

/// `Agent` calls whose text numbering is remembered; the oldest without waiting frames is forgotten
/// past this, so a long chat does not keep one entry per subagent ever spawned.
const MAX_TRACKED_PARENTS: usize = 256;

/// Refused `Agent` calls remembered (the subagent was not tracked), so their frames are dropped
/// instead of waiting for a task that never comes; the oldest is forgotten past this.
const MAX_REFUSED_CALLS: usize = 256;

/// Tool calls remembered until their result arrives, so a result keeps the call's name.
const MAX_OPEN_CALLS: usize = 1024;

/// The model name of a synthetic message.
const SYNTHETIC_MODEL: &str = "<synthetic>";

/// What one frame of a subagent changes.
pub(super) enum Update {
    /// A tool step, running or done.
    Tool(SubagentStep),
    /// Prose: a text step, and the candidate for the subagent's report.
    Text {
        id: String,
        at: Option<u64>,
        text: String,
    },
    Model(String),
    /// Tokens the API call read and wrote.
    Context(u64),
}

/// The decoder of subagent frames and the frames waiting for their task.
#[derive(Default)]
pub(super) struct SubagentSteps {
    /// Open tool calls, by `tool_use` id.
    calls: HashMap<String, Call>,
    /// By the `parent_tool_use_id` of the frames.
    parents: HashMap<String, Parent>,
    /// The keys of `parents`, oldest first.
    order: VecDeque<String>,
    /// Calls whose task was refused, and the same calls oldest first.
    refused: HashSet<String>,
    refused_order: VecDeque<String>,
}

struct Call {
    name: String,
    detail: String,
    at: Option<u64>,
}

/// The state of the frames of one `Agent` call.
#[derive(Default)]
struct Parent {
    /// The API message of the last text step and how many text steps it has.
    message: String,
    texts: u32,
    /// Frames that arrived before their task started, oldest first.
    waiting: Vec<Vec<Update>>,
}

impl SubagentSteps {
    /// What an `assistant` frame of the subagent spawned by call `parent` says.
    pub(super) fn assistant(&mut self, parent: &str, frame: &AssistantFrame) -> Vec<Update> {
        if frame.is_api_error_message {
            return Vec::new();
        }

        let message = &frame.message;
        let at = frame.timestamp.as_deref().and_then(millis);
        let mut updates = Vec::new();

        if let Some(model) = message
            .model
            .as_deref()
            .map(str::trim)
            .filter(|model| !model.is_empty() && *model != SYNTHETIC_MODEL)
        {
            updates.push(Update::Model(bounded_redacted(model, MAX_STEP_DETAIL_CHARS)));
        }

        if let Some(tokens) = message.usage.map(|usage| usage.total()).filter(|tokens| *tokens > 0) {
            updates.push(Update::Context(tokens));
        }

        for block in &message.content {
            match block {
                ContentBlock::ToolUse { id, name, input } => updates.extend(self.called(id, name, input, at)),
                ContentBlock::Text { text } if !text.trim().is_empty() => {
                    let id = self.text_id(parent, &message.id);
                    updates.push(Update::Text {
                        id,
                        at,
                        text: bounded_redacted(text.trim(), MAX_SUBAGENT_TEXT_CHARS),
                    });
                }
                _ => {}
            }
        }

        updates
    }

    /// What a `user` frame of the subagent says: the results of its tool calls.
    pub(super) fn user(&mut self, frame: &UserFrame) -> Vec<Update> {
        let mut updates = Vec::new();

        for block in frame.message.content.blocks() {
            if let ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } = block
            {
                updates.push(Update::Tool(self.ended(tool_use_id, content, *is_error)));
            }
        }

        updates
    }

    /// Keeps `updates` for the task of call `parent`, unless the bounds are reached.
    pub(super) fn hold(&mut self, parent: &str, updates: Vec<Update>) {
        if updates.is_empty() || self.refused.contains(parent) {
            return;
        }

        let waiting_calls = self
            .parents
            .values()
            .filter(|parent| !parent.waiting.is_empty())
            .count();
        let entry = self.parent_mut(parent);

        if entry.waiting.is_empty() && waiting_calls >= MAX_WAITING_CALLS {
            return;
        }

        if entry.waiting.len() < MAX_WAITING_FRAMES {
            entry.waiting.push(updates);
        }
    }

    /// Call `parent` got no task (the chat tracks no more subagents): its waiting frames are freed
    /// and its later frames are dropped, so it cannot hold one of the waiting slots for good.
    pub(super) fn refuse(&mut self, parent: &str) {
        self.parents.remove(parent);
        self.order.retain(|key| key != parent);

        if self.refused.insert(parent.to_owned()) {
            self.refused_order.push_back(parent.to_owned());
        }

        while self.refused_order.len() > MAX_REFUSED_CALLS {
            if let Some(oldest) = self.refused_order.pop_front() {
                self.refused.remove(&oldest);
            }
        }
    }

    /// Call `parent` got a task after all: its frames wait for it again.
    pub(super) fn accept(&mut self, parent: &str) {
        if self.refused.remove(parent) {
            self.refused_order.retain(|key| key != parent);
        }
    }

    /// The frames that waited for the task of call `parent`, oldest first.
    pub(super) fn take(&mut self, parent: &str) -> Vec<Vec<Update>> {
        self.parents
            .get_mut(parent)
            .map(|parent| std::mem::take(&mut parent.waiting))
            .unwrap_or_default()
    }

    /// The state of call `key`, created on first use. Past [`MAX_TRACKED_PARENTS`] the oldest call
    /// without waiting frames is forgotten first (only its text numbering is lost).
    fn parent_mut(&mut self, key: &str) -> &mut Parent {
        if !self.parents.contains_key(key) {
            if self.parents.len() >= MAX_TRACKED_PARENTS {
                self.forget_oldest();
            }

            self.order.push_back(key.to_owned());
        }

        self.parents.entry(key.to_owned()).or_default()
    }

    fn forget_oldest(&mut self) {
        for _ in 0..self.order.len() {
            let Some(oldest) = self.order.pop_front() else {
                return;
            };

            if self
                .parents
                .get(&oldest)
                .is_some_and(|parent| !parent.waiting.is_empty())
            {
                self.order.push_back(oldest);
            } else {
                self.parents.remove(&oldest);
                return;
            }
        }
    }

    fn called(&mut self, id: &str, name: &str, input: &Value, at: Option<u64>) -> Option<Update> {
        if id.is_empty() {
            return None;
        }

        let call = Call {
            name: bounded_redacted(name, MAX_STEP_DETAIL_CHARS),
            detail: preview(input),
            at,
        };
        let step = tool_step(id, &call, SubagentStepStatus::Running);

        if self.calls.len() < MAX_OPEN_CALLS || self.calls.contains_key(id) {
            self.calls.insert(id.to_owned(), call);
        }

        Some(Update::Tool(step))
    }

    fn ended(&mut self, id: &str, content: &Value, is_error: bool) -> SubagentStep {
        let call = self.calls.remove(id).unwrap_or(Call {
            name: String::new(),
            detail: String::new(),
            at: None,
        });
        let status = if is_error {
            SubagentStepStatus::Failed
        } else {
            SubagentStepStatus::Done
        };
        let (output, lines) = tail_redacted(&result_text(content), MAX_STEP_OUTPUT_LINES, MAX_STEP_OUTPUT_BYTES);

        SubagentStep {
            output: Some(output),
            output_lines: Some(lines),
            ..tool_step(id, &call, status)
        }
    }

    /// The id of the next text step of API message `message` of call `parent`.
    fn text_id(&mut self, parent: &str, message: &str) -> String {
        let parent = self.parent_mut(parent);

        if parent.message != message {
            parent.message = message.to_owned();
            parent.texts = 0;
        }

        parent.texts += 1;

        format!("{message}:{}", parent.texts - 1)
    }
}

impl Update {
    /// The step this update shows, if it is one.
    pub(super) fn step(&self) -> Option<SubagentStep> {
        match self {
            Self::Tool(step) => Some(step.clone()),
            Self::Text { id, at, text } => Some(SubagentStep {
                id: id.clone(),
                kind: SubagentStepKind::Text,
                name: None,
                detail: bounded_redacted(text, MAX_STEP_TEXT_CHARS),
                status: SubagentStepStatus::Done,
                at: *at,
                output: None,
                output_lines: None,
            }),
            Self::Model(_) | Self::Context(_) => None,
        }
    }
}

fn tool_step(id: &str, call: &Call, status: SubagentStepStatus) -> SubagentStep {
    SubagentStep {
        id: id.to_owned(),
        kind: SubagentStepKind::Tool,
        name: Some(call.name.clone()).filter(|name| !name.is_empty()),
        detail: call.detail.clone(),
        status,
        at: call.at,
        output: None,
        output_lines: None,
    }
}

/// The primary argument of a call on one line, redacted first, then cut.
fn preview(input: &Value) -> String {
    let redacted = primary_arg(input).map(|arg| redact_text(&arg)).unwrap_or_default();
    let line = redacted.lines().map(str::trim).find(|line| !line.is_empty());

    bounded_redacted(line.unwrap_or_default(), MAX_STEP_DETAIL_CHARS)
}

/// An RFC 3339 time in milliseconds since the Unix epoch.
fn millis(time: &str) -> Option<u64> {
    let time = Timestamp::from_str(time).ok()?;

    u64::try_from(time.as_millisecond()).ok()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn text_frame(message: &str) -> AssistantFrame {
        serde_json::from_value(
            json!({ "type": "assistant", "message": { "id": message, "content": [{ "type": "text", "text": "hi" }] } }),
        )
        .unwrap()
    }

    fn tool_frame(call: &str) -> AssistantFrame {
        serde_json::from_value(json!({ "type": "assistant", "message": { "id": "m", "content": [{ "type": "tool_use", "id": call, "name": "Bash", "input": { "command": "ls" } }] } }))
            .unwrap()
    }

    #[test]
    fn many_finished_parents_do_not_grow_the_map_and_waiting_ones_survive() {
        let mut steps = SubagentSteps::default();

        let updates = steps.assistant("toolu_waiting", &tool_frame("c0"));
        steps.hold("toolu_waiting", updates);

        for n in 0..MAX_TRACKED_PARENTS * 3 {
            steps.assistant(&format!("toolu_{n}"), &text_frame(&format!("msg_{n}")));
        }

        assert!(steps.parents.len() <= MAX_TRACKED_PARENTS, "{}", steps.parents.len());
        assert_eq!(steps.parents.len(), steps.order.len());
        assert_eq!(
            steps.take("toolu_waiting").len(),
            1,
            "a waiting call is never forgotten"
        );
    }

    #[test]
    fn a_refused_call_frees_its_slot_and_its_frames_are_dropped() {
        let mut steps = SubagentSteps::default();

        for n in 0..MAX_WAITING_CALLS {
            let call = format!("toolu_{n}");
            let updates = steps.assistant(&call, &tool_frame(&format!("c{n}")));
            steps.hold(&call, updates);
        }

        steps.refuse("toolu_0");
        let updates = steps.assistant("toolu_0", &tool_frame("again"));
        steps.hold("toolu_0", updates);
        assert!(steps.take("toolu_0").is_empty(), "nothing waits for a refused call");

        let updates = steps.assistant("toolu_new", &tool_frame("c_new"));
        steps.hold("toolu_new", updates);
        assert_eq!(steps.take("toolu_new").len(), 1, "the freed slot is usable");

        steps.accept("toolu_0");
        let updates = steps.assistant("toolu_0", &tool_frame("later"));
        steps.hold("toolu_0", updates);
        assert_eq!(steps.take("toolu_0").len(), 1, "an accepted call waits again");
    }

    #[test]
    fn text_numbering_continues_within_a_message_and_restarts_for_a_new_one() {
        let mut steps = SubagentSteps::default();
        let ids: Vec<String> = [("a", "m1"), ("a", "m1"), ("a", "m2")]
            .iter()
            .flat_map(|(parent, message)| steps.assistant(parent, &text_frame(message)))
            .filter_map(|update| match update {
                Update::Text { id, .. } => Some(id),
                _ => None,
            })
            .collect();

        assert_eq!(ids, ["m1:0", "m1:1", "m2:0"]);
    }
}
