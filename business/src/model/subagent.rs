//! Subagents a session's `task` tool spawned, as the chat shows them.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where a subagent is in its run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SubagentStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Aborted,
}

/// One recent tool call of a subagent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubagentTool {
    /// Tool name.
    pub tool: String,
    /// Short preview of its arguments (bounded, secrets redacted).
    pub args: String,
    pub is_error: bool,
}

/// A snapshot of one subagent. Every text is bounded and redacted; a field is `None` when the frame
/// that produced the snapshot did not carry it (lifecycle frames carry no progress), so the webview
/// keeps what it already knows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Subagent {
    /// Unique per session, e.g. `FixCanonicalUpdater`.
    pub id: String,
    /// The `task` tool call that spawned it, when known.
    pub parent_tool_call_id: Option<String>,
    /// Agent type, e.g. `explore` or `reviewer`.
    pub agent: String,
    pub status: SubagentStatus,
    /// Spawn order within its `task` call.
    pub index: u32,
    /// Runs in the background while the parent keeps working.
    pub detached: bool,
    pub description: Option<String>,
    /// The task it was given.
    pub task: Option<String>,
    /// What it says it is doing now.
    pub intent: Option<String>,
    pub current_tool: Option<String>,
    pub current_args: Option<String>,
    /// The last few tool calls, oldest first.
    pub recent_tools: Option<Vec<SubagentTool>>,
    /// The last few output lines.
    pub recent_output: Option<Vec<String>>,
    pub tool_count: Option<u32>,
    #[ts(type = "number | null")]
    pub tokens: Option<u64>,
    #[ts(type = "number | null")]
    pub context_tokens: Option<u64>,
    #[ts(type = "number | null")]
    pub context_window: Option<u64>,
    pub cost_usd: Option<f64>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    /// `provider/id` of the model it runs on.
    pub model: Option<String>,
}
