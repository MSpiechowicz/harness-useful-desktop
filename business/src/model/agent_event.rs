//! Normalized events streamed from a running agent to the webview (the `agent_start` channel).

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use ts_rs::TS;

use super::{
    ChatMessage, CommandSpec, MessageRole, ModelOption, NoticeLevel, PromptStatus, SessionState, Subagent, UiRequest,
};

/// One event of a session's stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentEnvelope {
    /// The `AgentSessionInfo.key` of the session.
    pub session_key: String,
    /// Sequence number, strictly increasing per session.
    #[ts(type = "number")]
    pub seq: u64,
    /// The event.
    pub event: AgentEvent,
}

/// What happened in an agent session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum AgentEvent {
    /// The agent process is up and accepts actions.
    Ready {
        /// RPC protocol version, when the harness reports one.
        protocol_version: Option<u32>,
    },
    /// Fresh session state.
    State {
        /// The state.
        state: SessionState,
    },
    /// The agent started working on a prompt.
    AgentStart,
    /// The agent stopped working (more events of the same prompt may follow).
    AgentEnd,
    /// The agent is idle: the prompt and everything queued behind it are done.
    Settled {
        /// True when the run ended by an abort.
        aborted: Option<bool>,
    },
    /// A model turn started.
    TurnStart,
    /// A model turn ended.
    TurnEnd,
    /// A message started streaming.
    MessageStart {
        /// Id used by the deltas and `messageEnd` of this message.
        message_id: String,
        /// Who produces the message.
        role: MessageRole,
    },
    /// Text appended to a text block.
    TextDelta {
        /// The streaming message.
        message_id: String,
        /// Index of the block in `ChatMessage.blocks`.
        content_index: u32,
        /// Appended text.
        delta: String,
    },
    /// Text appended to a thinking block.
    ThinkingDelta {
        /// The streaming message.
        message_id: String,
        /// Index of the block in `ChatMessage.blocks`.
        content_index: u32,
        /// Appended text.
        delta: String,
    },
    /// A message is complete; replaces everything streamed for it.
    MessageEnd {
        /// The message id from `messageStart`.
        message_id: String,
        /// The final message.
        message: ChatMessage,
    },
    /// A tool call started.
    ToolStart {
        /// Tool call id (matches `ChatBlock::ToolCall.id`).
        tool_call_id: String,
        /// Tool name.
        name: String,
        /// Tool arguments.
        args: JsonValue,
        /// Short human-readable intent, when the harness provides one.
        intent: Option<String>,
    },
    /// Partial output of a running tool.
    ToolUpdate {
        /// Tool call id.
        tool_call_id: String,
        /// Partial result so far.
        partial: JsonValue,
    },
    /// A tool call finished.
    ToolEnd {
        /// Tool call id.
        tool_call_id: String,
        /// Tool name.
        name: String,
        /// True when the tool failed or was denied.
        is_error: bool,
        /// Tool result (`content` blocks; harness `details` are dropped).
        result: JsonValue,
        /// Project-relative paths (forward slashes) a file-editing tool may have changed; only
        /// paths inside the project are kept.
        touched_paths: Vec<String>,
    },
    /// A subagent started, made progress or finished (OMP); the webview upserts it by `id`.
    Subagent {
        /// Its latest snapshot (boxed: it is by far the largest event).
        subagent: Box<Subagent>,
    },
    /// The slash-command catalog changed (already merged with the desktop commands).
    CommandsUpdated {
        /// The merged catalog.
        commands: Vec<CommandSpec>,
    },
    /// Output of a slash command the harness ran itself.
    CommandOutput {
        /// The output text.
        text: String,
    },
    /// A prompt finished.
    PromptResult {
        /// Id of the request that sent the prompt, when known.
        request_id: Option<String>,
        /// How it finished.
        status: PromptStatus,
        /// Error text when `status` is `error`.
        error: Option<String>,
        /// True when sending the prompt again may succeed (rate limit, network).
        retryable: bool,
    },
    /// The steering and follow-up queues changed.
    QueueUpdate {
        /// Pending steering messages.
        steering: Vec<String>,
        /// Pending follow-up messages.
        follow_up: Vec<String>,
        /// How many leading `steering` entries already went into the streaming response; they cannot be withdrawn.
        live_steered: u32,
    },
    /// Context compaction progress.
    Compaction {
        /// `start` or `end`.
        phase: String,
        /// Why compaction ran (e.g. `"threshold"`, `"manual"`).
        reason: Option<String>,
        /// Error text when compaction failed.
        error: Option<String>,
    },
    /// Automatic retry progress after a failed model call.
    Retry {
        /// `start` or `end`.
        phase: String,
        /// The attempt number (1-based).
        attempt: Option<u32>,
        /// How many attempts will be made.
        max_attempts: Option<u32>,
        /// Why the call is retried, or how the retry ended.
        message: Option<String>,
    },
    /// The active model changed.
    ModelChanged {
        /// The new model.
        model: Option<ModelOption>,
    },
    /// The thinking level changed.
    ThinkingChanged {
        /// The new level.
        level: String,
    },
    /// The agent asks the user something; answer with `agent_ui_respond`.
    UiRequest {
        /// The dialog.
        request: UiRequest,
    },
    /// The agent withdrew a pending request (timeout or abort); close its dialog.
    UiCancel {
        /// The `UiRequest.id`.
        request_id: String,
    },
    /// A message for the user (also extension `notify`).
    Notice {
        /// Severity.
        level: NoticeLevel,
        /// Text; never contains secrets.
        message: String,
    },
    /// An extension status-bar entry; `None` text removes it.
    Status {
        /// Entry key.
        key: String,
        /// Entry text.
        text: Option<String>,
    },
    /// An extension widget (lines of text) shown near the composer.
    Widget {
        /// Widget key.
        key: String,
        /// Lines to show; empty removes the widget.
        lines: Vec<String>,
        /// Where to show it (e.g. `"aboveEditor"`, `"belowEditor"`).
        placement: String,
    },
    /// Set the session tab title.
    Title {
        /// The title.
        title: String,
    },
    /// Prefill the composer with text.
    EditorText {
        /// The text.
        text: String,
    },
    /// The agent wants the user to visit a URL; the UI shows it with a copy button and never
    /// opens it automatically.
    OpenUrl {
        /// The URL.
        url: String,
        /// What to do there.
        instructions: Option<String>,
    },
    /// The agent sent a frame that could not be parsed or normalized.
    FrameError {
        /// The frame's `type`, when readable.
        original_type: Option<String>,
        /// What went wrong (no frame content).
        error: String,
    },
    /// A redacted line of the agent's stderr (rate-limited, at most 2 KiB).
    Stderr {
        /// The line.
        line: String,
    },
    /// The agent process exited; the session is over.
    Exited {
        /// Exit code, when the process exited normally.
        code: Option<i32>,
        /// Terminating signal (Unix), when killed.
        signal: Option<i32>,
    },
    /// An event type this app does not know; ignored by the UI.
    Unknown {
        /// The original event type.
        event_type: String,
    },
}
