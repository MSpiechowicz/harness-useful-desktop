//! The steps of a subagent (its tool calls and prose), streamed while it runs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Longest `detail` of a tool step (a one-line preview of its arguments), in characters.
pub const MAX_STEP_DETAIL_CHARS: usize = 240;

/// Longest `detail` of a text step, in characters (4 KiB).
pub const MAX_STEP_TEXT_CHARS: usize = 4 * 1024;

/// Most lines of a tool step's `output`, counted from the end.
pub const MAX_STEP_OUTPUT_LINES: usize = 400;

/// Most bytes of a tool step's `output` (64 KiB).
pub const MAX_STEP_OUTPUT_BYTES: usize = 64 * 1024;

/// What a step is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SubagentStepKind {
    /// A tool call.
    Tool,
    /// A piece of the subagent's own prose.
    Text,
}

/// Where a step is in its run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SubagentStepStatus {
    Running,
    Done,
    Failed,
}

/// One step of a subagent. Every text is bounded and redacted. A later event with the same `id`
/// replaces the step, so a tool call arrives once as `Running` and once as `Done` or `Failed`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SubagentStep {
    /// Upsert key: the tool call id, or `<message id>:<n>` for prose.
    pub id: String,
    pub kind: SubagentStepKind,
    /// The tool name (`None` for prose).
    pub name: Option<String>,
    /// Tool: a one-line preview of the arguments, at most [`MAX_STEP_DETAIL_CHARS`] characters.
    /// Text: the prose, at most [`MAX_STEP_TEXT_CHARS`] characters.
    pub detail: String,
    pub status: SubagentStepStatus,
    /// When it started, in milliseconds since the Unix epoch, when the harness says.
    #[ts(type = "number | null")]
    pub at: Option<u64>,
    /// The end of the tool result, set on `Done` and `Failed` only: at most
    /// [`MAX_STEP_OUTPUT_LINES`] lines and [`MAX_STEP_OUTPUT_BYTES`] bytes.
    pub output: Option<String>,
    /// Lines the result had before the cut.
    pub output_lines: Option<u32>,
}
