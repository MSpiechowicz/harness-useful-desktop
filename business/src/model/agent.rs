//! Launching and identifying agent chat sessions (`agent_start`, `agent_list`, `agent_bash`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::HarnessId;

/// An opaque reference to a harness session (OMP/PI session uuid).
///
/// Only the business layer resolves it to a session file; the webview never sees paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionRef {
    /// The harness that owns the session.
    pub harness: HarnessId,
    /// The harness session id.
    pub id: String,
}

/// OMP tool approval mode (`tools.approvalMode`, CLI `--approval-mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ApprovalMode {
    /// Ask before every tool call (CLI `always-ask`).
    AlwaysAsk,
    /// Ask before tools that write or execute (CLI `write`).
    Write,
    /// Never ask (CLI `yolo`; the OMP default).
    Yolo,
}

/// Arguments of `agent_start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentStartRequest {
    /// Which harness to run.
    pub harness: HarnessId,
    /// Absolute project path; must be the active project or a recent one.
    pub project: String,
    /// Resume this session instead of starting a new one.
    pub resume: Option<SessionRef>,
    /// Per-session approval mode; honoured only by harnesses with `capabilities.approvalModes`.
    pub approval_override: Option<ApprovalMode>,
    /// Trust the project's local resources for this launch only (PI project trust); honoured only
    /// by harnesses with `capabilities.projectTrust`. Never implied.
    pub trust_once: bool,
    /// A throwaway helper agent (naming a chat): no tools, no saved session, no thinking. Cannot
    /// resume a session.
    pub utility: bool,
}

/// A running agent session as returned by `agent_start` and `agent_list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentSessionInfo {
    /// Desktop session key (uuid v4); addresses the session in every later `agent_*` command.
    pub key: String,
    /// The harness running the session.
    pub harness: HarnessId,
    /// Absolute project path the agent runs in.
    pub project: String,
    /// Launch time (RFC 3339).
    pub started_at: String,
    /// The per-session approval override passed at launch.
    pub approval_override: Option<ApprovalMode>,
    /// The approval mode from the harness configuration (OMP `tools.approvalMode`, default
    /// `yolo`); `None` for harnesses without approval modes.
    pub configured_approval_mode: Option<ApprovalMode>,
    /// True when the session was launched with the one-time project trust.
    pub trusted_once: bool,
    /// RPC protocol version reported by the agent, once known.
    pub protocol_version: Option<u32>,
}

/// Result of `agent_bash` (a `!command` run through the agent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BashOutput {
    /// Combined output (redacted, possibly truncated).
    pub output: String,
    /// Exit code; `None` when the command was killed or cancelled.
    pub exit_code: Option<i32>,
    /// True when the command was cancelled.
    pub cancelled: bool,
    /// True when `output` was cut to the size limit.
    pub truncated: bool,
}
