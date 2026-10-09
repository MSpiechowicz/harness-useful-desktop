//! Delta batching: consecutive `textDelta` / `thinkingDelta` events of the same message block that
//! arrive within a short window are merged into one event, so the webview renders at most about
//! once per frame. Any other event flushes the held delta first, so order is preserved.

use std::time::Duration;

use tokio::time::Instant;

use crate::model::AgentEvent;

/// The production batching window (about one 60 Hz frame).
pub const COALESCE_WINDOW: Duration = Duration::from_millis(16);

/// Merges delta events; see the module docs. A zero window disables merging.
#[derive(Debug, Clone)]
pub struct DeltaCoalescer {
    window: Duration,
    held: Option<(AgentEvent, Instant)>,
}

impl DeltaCoalescer {
    /// A coalescer with the given window.
    pub fn new(window: Duration) -> Self {
        Self { window, held: None }
    }

    /// Accepts `event` at `now` and returns the events to deliver now, in order.
    pub fn push(&mut self, event: AgentEvent, now: Instant) -> Vec<AgentEvent> {
        if self.window.is_zero() {
            return vec![event];
        }

        if let Some((held, deadline)) = &mut self.held
            && now < *deadline
            && append(held, &event)
        {
            return Vec::new();
        }

        let mut ready = self.flush();

        if is_delta(&event) {
            self.held = Some((event, now + self.window));
        } else {
            ready.push(event);
        }

        ready
    }

    /// Releases the held delta, if any.
    pub fn flush(&mut self) -> Vec<AgentEvent> {
        self.held.take().map(|(event, _)| event).into_iter().collect()
    }

    /// When the held delta must be delivered at the latest.
    pub fn deadline(&self) -> Option<Instant> {
        self.held.as_ref().map(|(_, deadline)| *deadline)
    }
}

fn is_delta(event: &AgentEvent) -> bool {
    matches!(event, AgentEvent::TextDelta { .. } | AgentEvent::ThinkingDelta { .. })
}

/// Appends `next` to `held` when both are deltas of the same kind, message and block.
fn append(held: &mut AgentEvent, next: &AgentEvent) -> bool {
    match (held, next) {
        (
            AgentEvent::TextDelta {
                message_id,
                content_index,
                delta,
            },
            AgentEvent::TextDelta {
                message_id: next_message,
                content_index: next_index,
                delta: next_delta,
            },
        )
        | (
            AgentEvent::ThinkingDelta {
                message_id,
                content_index,
                delta,
            },
            AgentEvent::ThinkingDelta {
                message_id: next_message,
                content_index: next_index,
                delta: next_delta,
            },
        ) if message_id == next_message && content_index == next_index => {
            delta.push_str(next_delta);
            true
        }
        _ => false,
    }
}
