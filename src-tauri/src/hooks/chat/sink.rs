//! The sink around a chat's event channel: it hands the events the hooks react to to the chat's
//! hook task, and tells the limits check when the agent starts a subagent.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

use harness_business::agent::EventSink;
use harness_business::model::{AgentEnvelope, AgentEvent, Subagent, SubagentStatus};
use serde_json::Value;
use tokio::sync::mpsc;

use super::counts::Counts;

/// Forwards a chat's events to its sink (the webview, or a routine run's relay) first, then hands
/// the ones the hooks react to (tool ends, prompt results, settles, the exit) to the chat's hook
/// task without waiting. Counts the settles before the sink sees them.
pub struct HookSink {
    inner: Arc<dyn EventSink>,
    events: mpsc::UnboundedSender<AgentEvent>,
    counts: Arc<Counts>,
    /// Tells the limits check that the agent started a subagent.
    started: mpsc::UnboundedSender<()>,
    /// The subagents seen, so each starts one check.
    subagents: Mutex<HashSet<String>>,
    /// The provider of the chat's current model, as the harness last reported it.
    provider: ModelProvider,
}

/// The provider of a chat's current model (`anthropic`, `opencode-go`, ...), shared between its sink
/// and its hooks: the limits a chat sees are its plan's ([`harness_business::ecosystem::limit_providers`]).
pub type ModelProvider = Arc<Mutex<Option<String>>>;

/// The events a [`HookSink`] passed on, and its counts, for [`track`].
pub struct HookEvents {
    pub(super) received: mpsc::UnboundedReceiver<AgentEvent>,
    pub(super) counts: Arc<Counts>,
    pub(super) started: mpsc::UnboundedReceiver<()>,
    pub(super) provider: ModelProvider,
}

/// The most subagent ids a sink remembers (a chat that started more starts the count again).
const MAX_TRACKED_SUBAGENTS: usize = 512;

impl HookSink {
    /// A sink around `inner`, and the events it passes on.
    pub fn new(inner: Arc<dyn EventSink>) -> (Self, HookEvents) {
        let (events, received) = mpsc::unbounded_channel();
        let (started, spawned) = mpsc::unbounded_channel();
        let counts = Arc::new(Counts::default());
        let provider = ModelProvider::default();

        let sink = Self {
            inner,
            events,
            counts: counts.clone(),
            started,
            subagents: Mutex::default(),
            provider: provider.clone(),
        };

        let events = HookEvents {
            received,
            counts,
            started: spawned,
            provider,
        };

        (sink, events)
    }

    /// True the first time `subagent` is seen starting (pending or running).
    fn is_new_start(&self, subagent: &Subagent) -> bool {
        if !matches!(subagent.status, SubagentStatus::Pending | SubagentStatus::Running) {
            return false;
        }

        let mut seen = self.subagents.lock().unwrap_or_else(PoisonError::into_inner);

        if seen.len() >= MAX_TRACKED_SUBAGENTS {
            seen.clear();
        }

        seen.insert(subagent.id.clone())
    }
}

impl EventSink for HookSink {
    fn emit(&self, envelope: AgentEnvelope) {
        let relevant = hook_relevant(&envelope.event);

        // Counted before anyone can react to it, so a message sent after it starts the next run.
        let settled = matches!(envelope.event, AgentEvent::Settled { .. });
        self.counts.arrived(settled, relevant.is_some());

        // The current model's provider decides whose plan limits the chat sees.
        let model = match &envelope.event {
            AgentEvent::State { state } => Some(state.model.as_ref()),
            AgentEvent::ModelChanged { model } => Some(model.as_ref()),
            _ => None,
        };

        if let Some(model) = model {
            *self.provider.lock().unwrap_or_else(PoisonError::into_inner) = model.map(|model| model.provider.clone());
        }

        if let AgentEvent::Subagent { subagent } = &envelope.event
            && self.is_new_start(subagent)
        {
            // Nobody listening means the chat is not tracked: nothing to check.
            self.started.send(()).ok();
        }

        self.inner.emit(envelope);

        // A closed receiver means the chat is not tracked: nothing will handle it.
        if let Some(event) = relevant
            && self.events.send(event).is_err()
        {
            self.counts.handled();
        }
    }
}

/// The part of `event` the engine reads, when it reads it (a tool's result is left out).
fn hook_relevant(event: &AgentEvent) -> Option<AgentEvent> {
    match event {
        AgentEvent::ToolEnd {
            tool_call_id,
            name,
            is_error,
            touched_paths,
            ..
        } => Some(AgentEvent::ToolEnd {
            tool_call_id: tool_call_id.clone(),
            name: name.clone(),
            is_error: *is_error,
            result: Value::Null,
            touched_paths: touched_paths.clone(),
        }),
        AgentEvent::PromptResult { .. } | AgentEvent::Settled { .. } | AgentEvent::Exited { .. } => Some(event.clone()),
        _ => None,
    }
}
