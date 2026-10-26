//! One event stream for a chat whose `claude` process is replaced (a new session, a switch, a fork
//! or a clone start the CLI again).
//!
//! Every process gets its own [`Port`] as its sink; the ports of one chat hand their events to the
//! chat's [`Relay`], which numbers them again, so `seq` keeps rising across processes. A port the
//! chat moved away from is retired: whatever its process still emits (its `exited` last) is
//! dropped, and only the process the chat runs on reaches the chat. A port retires only while its
//! process runs: once it exited, its `exited` reaches the chat and the move is refused.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::agent::EventSink;
use crate::agent::process::AgentProcess;
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
    /// True once retired; held while an event passes, so retiring and emitting never interleave.
    retired: Mutex<bool>,
}

impl Port {
    pub(super) fn new(relay: &Arc<Relay>) -> Arc<Self> {
        Arc::new(Self {
            relay: relay.clone(),
            retired: Mutex::new(false),
        })
    }

    /// Drops every event `process` (this port's) emits from now on; false, retiring nothing, once
    /// it exited. A process publishes its exit before it emits `exited`, so a port that retires has
    /// passed no `exited` on.
    pub(super) fn retire_running(&self, process: &AgentProcess) -> bool {
        let mut retired = self.retired.lock().unwrap_or_else(PoisonError::into_inner);

        if !process.is_running() {
            return false;
        }

        *retired = true;
        true
    }
}

impl EventSink for Port {
    fn emit(&self, mut envelope: AgentEnvelope) {
        let retired = self.retired.lock().unwrap_or_else(PoisonError::into_inner);

        if *retired {
            return;
        }

        envelope.seq = self.relay.seq.fetch_add(1, Ordering::SeqCst) + 1;
        self.relay.sink.emit(envelope);
    }
}
