//! The typed OpenCode 2 event stream (`GET /api/event`); unknown event types are kept as
//! [`EventKind::Unknown`] and can be ignored.
//!
//! Each SSE `data:` is an envelope `{id, created?, type, data, location?, metadata?, durable?}`
//! ([`parse`]). A run, as recorded against `opencode` 2.1.2: `session.inbox.enqueued` →
//! `session.execution.started` → `session.inbox.delivered` → `session.step.started` → text,
//! reasoning and tool events → `session.step.ended` → `session.usage.updated` →
//! `session.execution.succeeded | failed | interrupted`. Payloads read leniently: unknown fields
//! are ignored, missing optional ones are `None`, empty or 0.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

mod session;
mod turn;

pub use session::{
    AgentSelected, CompactionDelta, CompactionEnded, CompactionFailed, CompactionStarted, ExecutionFailed,
    ExecutionInterrupted, Forked, FormCreated, FormSettled, InboxEnqueued, InboxRef, ModelSelected, PermissionReplied,
    Renamed, SessionCreated, SessionRef, SessionStatus, ShellEnded, ShellInfo, ShellOutput, ShellStarted,
    StatusChanged, Usage,
};
pub use turn::{
    PartDelta, PartEnded, PartRef, RetryScheduled, StepEnded, StepFailed, StepRef, StepStarted, ToolCalled,
    ToolContent, ToolFailed, ToolInputDelta, ToolInputEnded, ToolInputStarted, ToolProgress, ToolSuccess,
};

use super::wire::envelope::Location;
use super::wire::prompt::PermissionRequest;
use super::wire::session::optional_count;

/// One event of the stream.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// `evt_…`.
    pub id: String,
    /// When it happened, in milliseconds since the epoch (absent on `server.connected`).
    pub created: Option<u64>,
    /// The directory it is about (absent on some session events).
    pub location: Option<Location>,
    /// The session it is about: `data.sessionID`, or `data.form.sessionID` for `form.created`.
    pub session_id: Option<String>,
    /// What happened.
    pub kind: EventKind,
}

/// Why an event's `data:` could not be read. Never carries the payload.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EventError {
    /// Not a JSON object with a `type`.
    #[error("the OpenCode event is not a JSON object with a type")]
    Envelope,

    /// A known event type without its required fields.
    #[error("the OpenCode `{kind}` event does not have the expected fields")]
    Data {
        /// The event type.
        kind: String,
    },
}

#[derive(Deserialize)]
struct RawEvent {
    #[serde(default)]
    id: String,
    #[serde(default, deserialize_with = "optional_count")]
    created: Option<u64>,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    data: Value,
    #[serde(default)]
    location: Option<Location>,
}

/// Reads one SSE `data:` payload.
pub fn parse(data: &str) -> Result<Event, EventError> {
    let raw: RawEvent = serde_json::from_str(data).map_err(|_| EventError::Envelope)?;

    let session_id = raw
        .data
        .get("sessionID")
        .or_else(|| raw.data.get("form").and_then(|form| form.get("sessionID")))
        .and_then(Value::as_str)
        .map(str::to_owned);

    let kind = EventKind::decode(&raw.kind, raw.data).map_err(|_| EventError::Data { kind: raw.kind })?;

    Ok(Event {
        id: raw.id,
        created: raw.created,
        location: raw.location,
        session_id,
        kind,
    })
}

/// What an [`Event`] says, by its `type`.
#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    /// `server.connected`: the first event of every stream.
    ServerConnected,
    /// `agent.updated`: the agent list changed.
    AgentsUpdated,
    /// `config.updated`: the configuration changed (the agents are reloaded after it).
    ConfigUpdated,
    /// `command.updated`: the command list changed.
    CommandsUpdated,
    /// `model.updated`: the model list changed.
    ModelsUpdated,
    /// `provider.updated`: the provider list changed.
    ProvidersUpdated,
    /// `session.created`.
    SessionCreated(SessionCreated),
    /// `session.deleted`.
    SessionDeleted(SessionRef),
    /// `session.renamed` (also the automatic title).
    Renamed(Renamed),
    /// `session.forked`: `session_id` is the new session.
    Forked(Forked),
    /// `session.agent.selected`.
    AgentSelected(AgentSelected),
    /// `session.model.selected`.
    ModelSelected(ModelSelected),
    /// `session.status` (`idle | busy | retry`; not sent by 2.1.2 in the recordings).
    Status(StatusChanged),
    /// `session.idle` (deprecated).
    Idle(SessionRef),
    /// `session.instructions.updated`.
    InstructionsUpdated(SessionRef),
    /// `session.execution.started`: a run began.
    ExecutionStarted(SessionRef),
    /// `session.execution.succeeded`: the run ended.
    ExecutionSucceeded(SessionRef),
    /// `session.execution.failed`.
    ExecutionFailed(ExecutionFailed),
    /// `session.execution.interrupted` (`reason`: `user`, `shutdown`, `superseded`, `inactivity`).
    ExecutionInterrupted(ExecutionInterrupted),
    /// `session.inbox.enqueued`: a prompt, synthetic input or compaction was admitted.
    InboxEnqueued(InboxEnqueued),
    /// `session.inbox.delivered`: the item reached the run.
    InboxDelivered(InboxRef),
    /// `session.inbox.cancelled`.
    InboxCancelled(InboxRef),
    /// `session.step.started`: one model call.
    StepStarted(StepStarted),
    /// `session.step.streamed`: the model's response body ended.
    StepStreamed(StepRef),
    /// `session.step.ended`, with the step's cost, tokens and changed files.
    StepEnded(StepEnded),
    /// `session.step.failed`.
    StepFailed(StepFailed),
    /// `session.text.started`.
    TextStarted(PartRef),
    /// `session.text.delta`.
    TextDelta(PartDelta),
    /// `session.text.ended`, with the full text.
    TextEnded(PartEnded),
    /// `session.reasoning.started`.
    ReasoningStarted(PartRef),
    /// `session.reasoning.delta`.
    ReasoningDelta(PartDelta),
    /// `session.reasoning.ended`, with the full text.
    ReasoningEnded(PartEnded),
    /// `session.tool.input.started`: the tool's name.
    ToolInputStarted(ToolInputStarted),
    /// `session.tool.input.delta`.
    ToolInputDelta(ToolInputDelta),
    /// `session.tool.input.ended`: the raw input text.
    ToolInputEnded(ToolInputEnded),
    /// `session.tool.called`: the parsed input.
    ToolCalled(ToolCalled),
    /// `session.tool.progress`: live metadata of a running tool.
    ToolProgress(ToolProgress),
    /// `session.tool.success`.
    ToolSuccess(ToolSuccess),
    /// `session.tool.failed` (`aborted`, `permission.rejected`, …).
    ToolFailed(ToolFailed),
    /// `session.retry.scheduled`.
    RetryScheduled(RetryScheduled),
    /// `session.usage.updated`: the session's running totals.
    UsageUpdated(Usage),
    /// `session.compaction.started`.
    CompactionStarted(CompactionStarted),
    /// `session.compaction.delta`.
    CompactionDelta(CompactionDelta),
    /// `session.compaction.ended`, with the summary.
    CompactionEnded(CompactionEnded),
    /// `session.compaction.failed`.
    CompactionFailed(CompactionFailed),
    /// `session.shell.started`: a user `!command` runs.
    ShellStarted(ShellStarted),
    /// `session.shell.ended`, with its output.
    ShellEnded(ShellEnded),
    /// `permission.asked`.
    PermissionAsked(PermissionRequest),
    /// `permission.replied`.
    PermissionReplied(PermissionReplied),
    /// `form.created`: a tool asks questions.
    FormCreated(FormCreated),
    /// `form.replied`.
    FormReplied(FormSettled),
    /// `form.cancelled`.
    FormCancelled(FormSettled),
    /// Any other type (`project.updated`, `shell.created`, `vcs.branch.updated`, …).
    Unknown(String),
}

impl EventKind {
    fn decode(kind: &str, data: Value) -> serde_json::Result<Self> {
        use EventKind as E;

        let event = match kind {
            "server.connected" => E::ServerConnected,
            "agent.updated" => E::AgentsUpdated,
            "config.updated" => E::ConfigUpdated,
            "command.updated" => E::CommandsUpdated,
            "model.updated" => E::ModelsUpdated,
            "provider.updated" => E::ProvidersUpdated,
            "session.created" => E::SessionCreated(from(data)?),
            "session.deleted" => E::SessionDeleted(from(data)?),
            "session.renamed" => E::Renamed(from(data)?),
            "session.forked" => E::Forked(from(data)?),
            "session.agent.selected" => E::AgentSelected(from(data)?),
            "session.model.selected" => E::ModelSelected(from(data)?),
            "session.status" => E::Status(from(data)?),
            "session.idle" => E::Idle(from(data)?),
            "session.instructions.updated" => E::InstructionsUpdated(from(data)?),
            "session.execution.started" => E::ExecutionStarted(from(data)?),
            "session.execution.succeeded" => E::ExecutionSucceeded(from(data)?),
            "session.execution.failed" => E::ExecutionFailed(from(data)?),
            "session.execution.interrupted" => E::ExecutionInterrupted(from(data)?),
            "session.inbox.enqueued" => E::InboxEnqueued(from(data)?),
            "session.inbox.delivered" => E::InboxDelivered(from(data)?),
            "session.inbox.cancelled" => E::InboxCancelled(from(data)?),
            "session.step.started" => E::StepStarted(from(data)?),
            "session.step.streamed" => E::StepStreamed(from(data)?),
            "session.step.ended" => E::StepEnded(from(data)?),
            "session.step.failed" => E::StepFailed(from(data)?),
            "session.text.started" => E::TextStarted(from(data)?),
            "session.text.delta" => E::TextDelta(from(data)?),
            "session.text.ended" => E::TextEnded(from(data)?),
            "session.reasoning.started" => E::ReasoningStarted(from(data)?),
            "session.reasoning.delta" => E::ReasoningDelta(from(data)?),
            "session.reasoning.ended" => E::ReasoningEnded(from(data)?),
            "session.tool.input.started" => E::ToolInputStarted(from(data)?),
            "session.tool.input.delta" => E::ToolInputDelta(from(data)?),
            "session.tool.input.ended" => E::ToolInputEnded(from(data)?),
            "session.tool.called" => E::ToolCalled(from(data)?),
            "session.tool.progress" => E::ToolProgress(from(data)?),
            "session.tool.success" => E::ToolSuccess(from(data)?),
            "session.tool.failed" => E::ToolFailed(from(data)?),
            "session.retry.scheduled" => E::RetryScheduled(from(data)?),
            "session.usage.updated" => E::UsageUpdated(from(data)?),
            "session.compaction.started" => E::CompactionStarted(from(data)?),
            "session.compaction.delta" => E::CompactionDelta(from(data)?),
            "session.compaction.ended" => E::CompactionEnded(from(data)?),
            "session.compaction.failed" => E::CompactionFailed(from(data)?),
            "session.shell.started" => E::ShellStarted(from(data)?),
            "session.shell.ended" => E::ShellEnded(from(data)?),
            "permission.asked" => E::PermissionAsked(from(data)?),
            "permission.replied" => E::PermissionReplied(from(data)?),
            "form.created" => E::FormCreated(from(data)?),
            "form.replied" => E::FormReplied(from(data)?),
            "form.cancelled" => E::FormCancelled(from(data)?),
            other => E::Unknown(other.to_owned()),
        };

        Ok(event)
    }
}

fn from<T: DeserializeOwned>(data: Value) -> serde_json::Result<T> {
    serde_json::from_value(data)
}
