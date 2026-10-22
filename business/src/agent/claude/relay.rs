//! One event stream for a chat whose `claude` process is replaced (a new session, a switch, a fork
//! or a clone start the CLI again).
//!
//! Every process gets its own [`Port`] as its sink; the ports of one chat hand their events to the
//! chat's [`Relay`], which numbers them again, so `seq` keeps rising across processes. A port the
//! chat moved away from is retired: whatever its process still emits (its `exited` last) is
//! dropped, and only the process the chat runs on reaches the chat.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::agent::EventSink;
use crate::model::AgentEnvelope;

/// The chat's sink and its event counter.
pub(super) struct Relay {
    sink: Arc<dyn EventSink>,
    seq: AtomicU64,
}

impl Relay {
    pub(super) fn new(sink: Arc<dyn EventSink>) -> Arc<Self> {
        Arc::new(Self {
            sink,
            seq: AtomicU64::new(0),
        })
    }
}

/// The sink of one process of the chat.
pub(super) struct Port {
    relay: Arc<Relay>,
    retired: AtomicBool,
}

impl Port {
    pub(super) fn new(relay: &Arc<Relay>) -> Arc<Self> {
        Arc::new(Self {
            relay: relay.clone(),
            retired: AtomicBool::new(false),
        })
    }

    /// Drops every event the process emits from now on.
    pub(super) fn retire(&self) {
        self.retired.store(true, Ordering::SeqCst);
    }
}

impl EventSink for Port {
    fn emit(&self, mut envelope: AgentEnvelope) {
        if self.retired.load(Ordering::SeqCst) {
            return;
        }

        envelope.seq = self.relay.seq.fetch_add(1, Ordering::SeqCst) + 1;
        self.relay.sink.emit(envelope);
    }
}
