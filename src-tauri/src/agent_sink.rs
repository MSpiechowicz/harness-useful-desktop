//! Delivers a session's events to the webview over the Tauri [`Channel`] passed to `agent_start`.
//! A routine run's chat has a sink of its own (`crate::routines::relay`); both are [`ChatSink`]s.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use harness_business::agent::EventSink;
use harness_business::model::{AgentEnvelope, HarnessId};
use tauri::ipc::Channel;

/// Called once with the session key when the channel stops accepting events.
pub type OnClosed = Box<dyn Fn(&str) + Send + Sync>;

/// Where a chat's events go.
pub trait ChatSink: EventSink {
    /// True once nobody can see the chat any more: a chat that is starting is stopped.
    fn is_closed(&self) -> bool;
}

/// An [`EventSink`] over a webview channel.
///
/// A failed send means the webview that created the channel is gone. Nobody can see the session
/// any more, so the sink drops every later event and calls its [`OnClosed`] hook once, which
/// stops the session. Only the session key, harness and event counts are logged.
pub struct ChannelSink {
    channel: Channel<AgentEnvelope>,
    harness: HarnessId,
    on_closed: OnClosed,
    closed: AtomicBool,
    delivered: AtomicU64,
}

impl ChannelSink {
    /// A sink sending to `channel`; `on_closed` runs when the channel closes.
    pub fn new(channel: Channel<AgentEnvelope>, harness: HarnessId, on_closed: OnClosed) -> Self {
        Self {
            channel,
            harness,
            on_closed,
            closed: AtomicBool::new(false),
            delivered: AtomicU64::new(0),
        }
    }
}

impl ChatSink for ChannelSink {
    /// True once a send failed.
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

impl EventSink for ChannelSink {
    fn emit(&self, envelope: AgentEnvelope) {
        if self.is_closed() {
            return;
        }

        let key = envelope.session_key.clone();

        if self.channel.send(envelope).is_ok() {
            self.delivered.fetch_add(1, Ordering::Relaxed);
            return;
        }

        let already_closed = self.closed.swap(true, Ordering::AcqRel);
        if already_closed {
            return;
        }

        tracing::warn!(
            session = %key,
            harness = ?self.harness,
            delivered = self.delivered.load(Ordering::Relaxed),
            "the event channel closed; stopping the agent session"
        );
        (self.on_closed)(&key);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::test_support::collecting_channel;
    use harness_business::model::AgentEvent;

    fn envelope(seq: u64) -> AgentEnvelope {
        AgentEnvelope {
            session_key: "key-1".to_owned(),
            seq,
            event: AgentEvent::TurnStart,
        }
    }

    fn sink_over(channel: Channel<AgentEnvelope>, closed_keys: &Arc<Mutex<Vec<String>>>) -> ChannelSink {
        let closed_keys = closed_keys.clone();

        ChannelSink::new(
            channel,
            HarnessId::Omp,
            Box::new(move |key| closed_keys.lock().unwrap().push(key.to_owned())),
        )
    }

    #[test]
    fn sends_envelopes_as_camel_case_json() {
        let (channel, received) = collecting_channel();
        let closed = Arc::new(Mutex::new(Vec::new()));
        let sink = sink_over(channel, &closed);

        sink.emit(envelope(1));
        sink.emit(envelope(2));

        let received = received.lock().unwrap();
        assert_eq!(received.len(), 2);
        assert_eq!(received[0]["sessionKey"], "key-1");
        assert_eq!(received[1]["seq"], 2);
        assert_eq!(received[1]["event"]["type"], "turnStart");
        assert!(!sink.is_closed());
        assert!(closed.lock().unwrap().is_empty());
    }

    #[test]
    fn a_closed_channel_reports_once_and_drops_later_events() {
        let attempts = Arc::new(Mutex::new(0));
        let counter = attempts.clone();
        let channel = Channel::new(move |_| {
            *counter.lock().unwrap() += 1;
            Err(tauri::Error::WebviewNotFound)
        });
        let closed = Arc::new(Mutex::new(Vec::new()));
        let sink = sink_over(channel, &closed);

        sink.emit(envelope(1));
        sink.emit(envelope(2));
        sink.emit(envelope(3));

        assert!(sink.is_closed());
        assert_eq!(*closed.lock().unwrap(), ["key-1"]);
        assert_eq!(*attempts.lock().unwrap(), 1);
    }
}
