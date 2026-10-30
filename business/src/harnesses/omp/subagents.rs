//! OMP's subagent stream (`subagent_progress`, `subagent_lifecycle`), enabled per session with
//! `set_subagent_subscription`. Mirrors `RpcSubagentFrame` and `AgentProgress` in
//! `pi-coding-agent/src/modes/rpc/rpc-types.ts` and `pi-tui/tools/task`.
//!
//! Only a fixed set of fields is copied, every text is redacted and clipped, and lists are capped,
//! so a chatty subagent cannot flood the webview.

use serde_json::{Value, json};

use crate::model::{MAX_SUBAGENT_TEXT_CHARS, Subagent, SubagentRoute, SubagentStatus, SubagentTool};
use crate::security::bounded_redacted;

/// Longest description, intent, tool argument preview or output line kept.
pub(super) const LINE_CHARS: usize = 240;
/// Recent tool calls kept per snapshot.
pub(super) const RECENT_TOOLS: usize = 5;
/// Recent output lines kept per snapshot.
const RECENT_OUTPUT: usize = 6;

/// The frame turning the subagent stream on (`progress` carries [`Subagent`] snapshots, not raw events).
pub fn subscription_frame() -> Value {
    json!({ "type": "set_subagent_subscription", "level": "progress" })
}

/// The snapshot carried by a `subagent_progress` or `subagent_lifecycle` frame; `None` for any
/// other frame or a payload without an id.
pub fn decode_subagent(frame: &Value) -> Option<Subagent> {
    let kind = frame.get("type").and_then(Value::as_str)?;
    let payload = frame.get("payload")?;

    match kind {
        "subagent_progress" => progress(payload),
        "subagent_lifecycle" => lifecycle(payload),
        _ => None,
    }
}

fn progress(payload: &Value) -> Option<Subagent> {
    let progress = payload.get("progress")?;
    let status = match progress.get("status").and_then(Value::as_str) {
        Some("pending") => SubagentStatus::Pending,
        Some("completed") => SubagentStatus::Completed,
        Some("failed") => SubagentStatus::Failed,
        Some("aborted") => SubagentStatus::Aborted,
        _ => SubagentStatus::Running,
    };

    let mut subagent = base(payload, progress.get("id").or_else(|| payload.get("id"))?, status)?;
    subagent.description = clipped(progress, "description", LINE_CHARS).or(subagent.description);
    subagent.task = clipped(payload, "assignment", MAX_SUBAGENT_TEXT_CHARS)
        .or_else(|| clipped(payload, "task", MAX_SUBAGENT_TEXT_CHARS))
        .or_else(|| clipped(progress, "task", MAX_SUBAGENT_TEXT_CHARS));
    subagent.intent =
        clipped(progress, "currentToolIntent", LINE_CHARS).or_else(|| clipped(progress, "lastIntent", LINE_CHARS));
    subagent.current_tool = clipped(progress, "currentTool", LINE_CHARS);
    subagent.current_args = clipped(progress, "currentToolArgs", LINE_CHARS);
    subagent.recent_tools = Some(recent_tools(progress));
    subagent.recent_output = Some(recent_output(progress));
    subagent.tool_count = number(progress, "toolCount").and_then(|count| u32::try_from(count).ok());
    subagent.tokens = number(progress, "tokens");
    subagent.context_tokens = number(progress, "contextTokens");
    subagent.context_window = number(progress, "contextWindow");
    subagent.cost_usd = progress
        .get("cost")
        .and_then(Value::as_f64)
        .filter(|cost| cost.is_finite() && *cost >= 0.0);
    subagent.duration_ms = number(progress, "durationMs");
    subagent.model = clipped(progress, "resolvedModelIdentity", LINE_CHARS)
        .or_else(|| clipped(progress, "resolvedModel", LINE_CHARS));

    Some(subagent)
}

fn lifecycle(payload: &Value) -> Option<Subagent> {
    let status = match payload.get("status").and_then(Value::as_str)? {
        "completed" => SubagentStatus::Completed,
        "failed" => SubagentStatus::Failed,
        "aborted" => SubagentStatus::Aborted,
        _ => SubagentStatus::Running,
    };

    base(payload, payload.get("id")?, status)
}

/// The fields both frames carry; progress-only fields start empty.
fn base(payload: &Value, id: &Value, status: SubagentStatus) -> Option<Subagent> {
    let id = id.as_str().filter(|id| !id.is_empty())?;

    let agent = clipped(payload, "agent", LINE_CHARS).unwrap_or_else(|| "task".to_owned());

    Some(Subagent {
        message_route: Some(SubagentRoute::Orchestrator),
        can_stop: Some(false),
        parent_tool_call_id: payload
            .get("parentToolCallId")
            .and_then(Value::as_str)
            .map(|id| bounded_redacted(id, LINE_CHARS)),
        index: number(payload, "index")
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(0),
        detached: payload.get("detached").and_then(Value::as_bool).unwrap_or(false),
        description: clipped(payload, "description", LINE_CHARS),
        ..Subagent::new(bounded_redacted(id, LINE_CHARS), agent, status)
    })
}

fn recent_tools(progress: &Value) -> Vec<SubagentTool> {
    let tools = progress
        .get("recentTools")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let skip = tools.len().saturating_sub(RECENT_TOOLS);

    tools
        .iter()
        .skip(skip)
        .filter_map(|tool| {
            Some(SubagentTool {
                tool: clipped(tool, "tool", LINE_CHARS)?,
                args: clipped(tool, "args", LINE_CHARS).unwrap_or_default(),
                is_error: tool.get("isError").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

fn recent_output(progress: &Value) -> Vec<String> {
    let lines = progress
        .get("recentOutput")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    // OMP sends the newest line first: keep the newest, oldest first
    let mut kept: Vec<&Value> = lines.iter().take(RECENT_OUTPUT).collect();
    kept.reverse();

    kept.into_iter()
        .filter_map(Value::as_str)
        .map(|line| bounded_redacted(line, LINE_CHARS))
        .collect()
}

pub(super) fn clipped(value: &Value, key: &str, max: usize) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| bounded_redacted(text, max))
}

pub(super) fn number(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|number| {
        number.as_u64().or_else(|| {
            number
                .as_f64()
                .filter(|float| float.is_finite() && *float >= 0.0)
                .map(|float| float as u64)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_frames_become_bounded_snapshots() {
        let output: Vec<String> = (0..20).map(|line| format!("line {line}")).collect();
        // OMP sends the newest line first
        let sent: Vec<&String> = output.iter().rev().collect();
        let tools: Vec<Value> = (0..9)
            .map(|n| json!({ "tool": "read", "args": format!("f{n}.rs"), "endMs": n }))
            .collect();
        let frame = json!({
            "type": "subagent_progress",
            "payload": {
                "index": 1, "agent": "explore", "agentSource": "bundled", "task": "Map the parser",
                "parentToolCallId": "call_1",
                "progress": {
                    "index": 1, "id": "ParserScout", "agent": "explore", "status": "running",
                    "task": "Map the parser", "currentTool": "grep", "currentToolArgs": "fn parse",
                    "currentToolIntent": "Finding parse callers", "recentTools": tools, "recentOutput": sent,
                    "toolCount": 9, "tokens": 12_000, "cost": 0.012, "durationMs": 4200.5,
                    "resolvedModelIdentity": "anthropic/claude-sonnet-4-5", "requests": 3
                }
            }
        });

        let subagent = decode_subagent(&frame).unwrap();

        assert_eq!(subagent.id, "ParserScout");
        assert_eq!(subagent.parent_tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(subagent.status, SubagentStatus::Running);
        assert_eq!(subagent.intent.as_deref(), Some("Finding parse callers"));
        assert_eq!(subagent.recent_tools.as_ref().unwrap().len(), RECENT_TOOLS);
        assert_eq!(subagent.recent_tools.as_ref().unwrap().last().unwrap().args, "f8.rs");
        assert_eq!(subagent.recent_output.as_ref().unwrap(), &output[14..]);
        assert_eq!(subagent.duration_ms, Some(4200));
        assert_eq!(subagent.model.as_deref(), Some("anthropic/claude-sonnet-4-5"));
    }

    #[test]
    fn lifecycle_frames_carry_status_only() {
        let frame = json!({
            "type": "subagent_lifecycle",
            "payload": { "id": "ParserScout", "agent": "explore", "agentSource": "bundled", "status": "completed", "index": 0, "detached": true }
        });

        let subagent = decode_subagent(&frame).unwrap();

        assert_eq!(subagent.status, SubagentStatus::Completed);
        assert!(subagent.detached);
        assert_eq!(subagent.tokens, None);
        assert_eq!(subagent.recent_output, None);
    }

    #[test]
    fn texts_are_clipped_and_bad_frames_ignored() {
        let long = "x".repeat(MAX_SUBAGENT_TEXT_CHARS + 2000);
        let frame = json!({
            "type": "subagent_progress",
            "payload": { "agent": "task", "task": long, "progress": { "id": "Long", "status": "pending" } }
        });

        let subagent = decode_subagent(&frame).unwrap();
        assert_eq!(subagent.task.unwrap().chars().count(), MAX_SUBAGENT_TEXT_CHARS);

        assert!(decode_subagent(&json!({ "type": "subagent_progress", "payload": { "progress": {} } })).is_none());
        assert!(decode_subagent(&json!({ "type": "agent_start" })).is_none());
    }
}
