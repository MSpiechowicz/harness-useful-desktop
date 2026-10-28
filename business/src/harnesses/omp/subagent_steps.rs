//! The steps of OMP subagents, derived from their `subagent_progress` snapshots.
//!
//! OMP has no per-step event for a subagent, only a snapshot with `toolCount` (finished tool
//! calls), `recentTools` (the last finished calls, oldest first), `currentTool` (the call in
//! flight) and `recentOutput`. When `toolCount` rises by `k`, the last `k` entries of `recentTools`
//! are new: each becomes a step with the id `<subagent id>#<ordinal>`, and the call in flight
//! becomes a running step with the next ordinal that the finished step later replaces. When the
//! subagent ends (a completed, failed or aborted frame) a call still in flight is closed: `Done`
//! if the subagent completed, `Failed` if it failed or was aborted, so no step stays running.

use serde_json::Value;

use super::subagents::{LINE_CHARS, RECENT_TOOLS, clipped, number};
use crate::model::{
    AgentEvent, MAX_STEP_DETAIL_CHARS, MAX_STEP_OUTPUT_BYTES, MAX_STEP_OUTPUT_LINES, Subagent, SubagentStatus,
    SubagentStep, SubagentStepKind, SubagentStepStatus,
};
use crate::security::tail_redacted;

/// Subagents followed at once; the oldest is forgotten past this (it only resends its `task`).
const MAX_TRACKED: usize = 64;

/// What the tracker knows about one subagent.
#[derive(Debug, Clone)]
struct Followed {
    id: String,
    tool_count: u32,
    /// The call in flight last sent: its name, its arguments and its ordinal.
    running: Option<(String, String, u32)>,
    output: String,
    last_done: Option<SubagentStep>,
}

/// Per-session step tracker, one per OMP session.
#[derive(Debug, Clone, Default)]
pub struct SubagentSteps {
    followed: Vec<Followed>,
}

impl SubagentSteps {
    /// The events for one subagent frame: its snapshot (the `task` only the first time the id is
    /// seen) followed by the steps the frame completed or started.
    pub fn observe(&mut self, frame: &Value, mut subagent: Subagent) -> Vec<AgentEvent> {
        let (followed, first) = self.follow(&subagent.id);

        if !first {
            subagent.task = None;
        }

        let steps = match frame.pointer("/payload/progress") {
            Some(progress) => followed.steps(progress, &subagent),
            None => followed.settle(subagent.status),
        };

        let id = subagent.id.clone();
        let mut events = vec![AgentEvent::Subagent {
            subagent: Box::new(subagent),
        }];

        events.extend(steps.into_iter().map(|step| AgentEvent::SubagentStep {
            subagent_id: id.clone(),
            step: Box::new(step),
        }));

        events
    }

    fn follow(&mut self, id: &str) -> (&mut Followed, bool) {
        let known = self.followed.iter().position(|followed| followed.id == id);

        let index = known.unwrap_or_else(|| {
            if self.followed.len() >= MAX_TRACKED {
                self.followed.remove(0);
            }

            self.followed.push(Followed {
                id: id.to_owned(),
                tool_count: 0,
                running: None,
                output: String::new(),
                last_done: None,
            });

            self.followed.len() - 1
        });

        (&mut self.followed[index], known.is_none())
    }
}

impl Followed {
    /// The steps `progress` adds to what was already sent: finished calls, then output on the
    /// latest finished one, then the running call.
    fn steps(&mut self, progress: &Value, subagent: &Subagent) -> Vec<SubagentStep> {
        let mut steps = self.finished(progress, subagent.tool_count);

        if let Some(output) = self.new_output(progress) {
            self.attach_output(&mut steps, output);
        }

        if is_final(subagent.status) {
            steps.extend(self.settle(subagent.status));
        } else if let Some(step) = self.running_step(subagent) {
            steps.push(step);
        }

        steps
    }

    /// Closes the call still in flight when the subagent ended: `Done` after a completion,
    /// `Failed` after a failure or an abort. A call the tool count already finished (its step was
    /// replaced) needs nothing.
    fn settle(&mut self, status: SubagentStatus) -> Vec<SubagentStep> {
        if !is_final(status) {
            return Vec::new();
        }

        let Some((name, detail, ordinal)) = self.running.take() else {
            return Vec::new();
        };

        if ordinal <= self.tool_count {
            return Vec::new();
        }

        vec![SubagentStep {
            id: step_id(&self.id, ordinal),
            kind: SubagentStepKind::Tool,
            name: Some(name),
            detail,
            status: if status == SubagentStatus::Completed {
                SubagentStepStatus::Done
            } else {
                SubagentStepStatus::Failed
            },
            at: None,
            output: None,
            output_lines: None,
        }]
    }

    /// Steps for the calls finished since the last frame.
    fn finished(&mut self, progress: &Value, count: Option<u32>) -> Vec<SubagentStep> {
        let Some(count) = count.filter(|count| *count > self.tool_count) else {
            return Vec::new();
        };

        let rose = usize::try_from(count - self.tool_count).unwrap_or(usize::MAX);
        self.tool_count = count;

        let tools = progress
            .get("recentTools")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let take = rose.min(RECENT_TOOLS).min(tools.len());
        let tools = &tools[tools.len() - take..];

        let steps: Vec<SubagentStep> = tools
            .iter()
            .enumerate()
            .filter_map(|(offset, tool)| {
                // The last entry is the call that made the count what it is.
                let ordinal = count.saturating_sub(u32::try_from(take - 1 - offset).ok()?);
                finished_step(&self.id, ordinal, tool)
            })
            .collect();

        if let Some(last) = steps.last() {
            self.last_done = Some(last.clone());
        }

        steps
    }

    /// The tail of `recentOutput` when it differs from the one sent last.
    fn new_output(&mut self, progress: &Value) -> Option<(String, u32)> {
        let lines: Vec<&str> = progress
            .get("recentOutput")
            .and_then(Value::as_array)?
            .iter()
            .filter_map(Value::as_str)
            .collect();

        let (tail, total) = tail_redacted(&lines.join("\n"), MAX_STEP_OUTPUT_LINES, MAX_STEP_OUTPUT_BYTES);

        if tail.is_empty() || tail == self.output {
            return None;
        }

        self.output.clone_from(&tail);

        Some((tail, total))
    }

    /// Puts `output` on the newest finished step: one in `steps`, else the last one sent before.
    fn attach_output(&mut self, steps: &mut Vec<SubagentStep>, (output, lines): (String, u32)) {
        if steps.is_empty() {
            let Some(step) = self.last_done.clone() else {
                return;
            };

            steps.push(step);
        }

        if let Some(step) = steps.last_mut() {
            step.output = Some(output);
            step.output_lines = Some(lines);
            self.last_done = Some(step.clone());
        }
    }

    /// The call in flight, unless it is the one sent last.
    fn running_step(&mut self, subagent: &Subagent) -> Option<SubagentStep> {
        let Some(name) = subagent.current_tool.clone() else {
            self.running = None;
            return None;
        };

        let detail = subagent.current_args.clone().unwrap_or_default();
        let ordinal = self.tool_count.saturating_add(1);
        let key = (name.clone(), detail.clone(), ordinal);

        // The ordinal tells a second identical call from a repeated frame of the first.
        if self.running.as_ref() == Some(&key) {
            return None;
        }

        self.running = Some(key);

        Some(SubagentStep {
            id: step_id(&self.id, ordinal),
            kind: SubagentStepKind::Tool,
            name: Some(name),
            detail,
            status: SubagentStepStatus::Running,
            at: None,
            output: None,
            output_lines: None,
        })
    }
}

fn finished_step(subagent_id: &str, ordinal: u32, tool: &Value) -> Option<SubagentStep> {
    let failed = tool.get("isError").and_then(Value::as_bool).unwrap_or(false);

    Some(SubagentStep {
        id: step_id(subagent_id, ordinal),
        kind: SubagentStepKind::Tool,
        name: Some(clipped(tool, "tool", LINE_CHARS)?),
        detail: clipped(tool, "args", MAX_STEP_DETAIL_CHARS).unwrap_or_default(),
        status: if failed {
            SubagentStepStatus::Failed
        } else {
            SubagentStepStatus::Done
        },
        at: number(tool, "endMs"),
        output: None,
        output_lines: None,
    })
}

fn is_final(status: SubagentStatus) -> bool {
    matches!(
        status,
        SubagentStatus::Completed | SubagentStatus::Failed | SubagentStatus::Aborted
    )
}

fn step_id(subagent_id: &str, ordinal: u32) -> String {
    format!("{subagent_id}#{ordinal}")
}

#[cfg(test)]
mod tests;
