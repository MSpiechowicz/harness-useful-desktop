//! Where a session's events go, and the task that numbers and batches them.
//!
//! [`EventSink`] is implemented by the desktop shell (a Tauri channel) and by tests
//! ([`CollectingSink`]). Every session has one emitter task that owns the sequence counter, so
//! `seq` is strictly increasing however many tasks produce events. Until startup completes the
//! emitter holds events back: the stream always begins with `ready`, `state` and
//! `commandsUpdated`, and a session that fails to start emits nothing.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep_until};

use super::coalesce::DeltaCoalescer;
use crate::model::{AgentEnvelope, AgentEvent};

/// Receives a session's events in order.
pub trait EventSink: Send + Sync {
    /// Delivers one event. Must not block for long (it runs on the session's emitter task).
    fn emit(&self, envelope: AgentEnvelope);
}

/// Events produced before startup completes that are kept at most.
const MAX_HELD: usize = 1024;

/// Messages to the emitter task.
pub(crate) enum EmitMsg {
    /// An event to deliver (held back until [`EmitMsg::Open`]).
    Event(Box<AgentEvent>),
    /// Startup completed: deliver these first, then the held events.
    Open(Vec<AgentEvent>),
    /// Deliver what is buffered and stop. Held events of an unopened session are dropped.
    Close,
}

/// Starts the emitter task of session `key`.
pub(crate) fn spawn_emitter(
    key: String,
    sink: Arc<dyn EventSink>,
    window: Duration,
) -> (mpsc::UnboundedSender<EmitMsg>, JoinHandle<()>) {
    let (sender, receiver) = mpsc::unbounded_channel();
    let task = tokio::spawn(run_emitter(key, sink, window, receiver));

    (sender, task)
}

struct Delivery {
    key: String,
    sink: Arc<dyn EventSink>,
    seq: u64,
}

impl Delivery {
    fn send(&mut self, events: Vec<AgentEvent>) {
        for event in events {
            self.seq += 1;
            self.sink.emit(AgentEnvelope {
                session_key: self.key.clone(),
                seq: self.seq,
                event,
            });
        }
    }
}

async fn run_emitter(
    key: String,
    sink: Arc<dyn EventSink>,
    window: Duration,
    mut receiver: mpsc::UnboundedReceiver<EmitMsg>,
) {
    let mut delivery = Delivery { key, sink, seq: 0 };
    let mut coalescer = DeltaCoalescer::new(window);
    let mut held: Option<Vec<AgentEvent>> = Some(Vec::new());

    loop {
        let message = match coalescer.deadline() {
            Some(deadline) => tokio::select! {
                message = receiver.recv() => message,
                () = sleep_until(deadline) => {
                    delivery.send(coalescer.flush());
                    continue;
                }
            },
            None => receiver.recv().await,
        };

        match message {
            Some(EmitMsg::Event(event)) => match &mut held {
                Some(buffer) if buffer.len() < MAX_HELD => buffer.push(*event),
                Some(_) => {}
                None => delivery.send(coalescer.push(*event, Instant::now())),
            },
            Some(EmitMsg::Open(first)) => {
                // The catalog in `first` supersedes catalog updates pushed during startup.
                let startup = held.take().unwrap_or_default().into_iter();
                let later = startup.filter(|event| !matches!(event, AgentEvent::CommandsUpdated { .. }));

                for event in first.into_iter().chain(later) {
                    delivery.send(coalescer.push(event, Instant::now()));
                }
            }
            Some(EmitMsg::Close) | None => {
                delivery.send(coalescer.flush());
                break;
            }
        }
    }
}

/// An [`EventSink`] that keeps every envelope (tests, fixture export).
#[derive(Debug)]
pub struct CollectingSink {
    envelopes: Mutex<Vec<AgentEnvelope>>,
    count: watch::Sender<usize>,
}

impl Default for CollectingSink {
    fn default() -> Self {
        Self {
            envelopes: Mutex::new(Vec::new()),
            count: watch::Sender::new(0),
        }
    }
}

impl CollectingSink {
    /// A new, empty sink.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Every envelope received so far.
    pub fn envelopes(&self) -> Vec<AgentEnvelope> {
        self.envelopes.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The events received so far.
    pub fn events(&self) -> Vec<AgentEvent> {
        self.envelopes().into_iter().map(|envelope| envelope.event).collect()
    }

    /// Waits until an event satisfies `predicate` (at most `limit`) and returns it.
    pub async fn wait_for(&self, limit: Duration, predicate: impl Fn(&AgentEvent) -> bool) -> Option<AgentEvent> {
        let mut changes = self.count.subscribe();

        let search = async {
            loop {
                if let Some(found) = self.events().into_iter().find(|event| predicate(event)) {
                    return Some(found);
                }

                changes.changed().await.ok()?;
            }
        };

        tokio::time::timeout(limit, search).await.ok().flatten()
    }
}

impl EventSink for CollectingSink {
    fn emit(&self, envelope: AgentEnvelope) {
        let count = {
            let mut envelopes = self.envelopes.lock().unwrap_or_else(PoisonError::into_inner);
            envelopes.push(envelope);
            envelopes.len()
        };

        self.count.send_replace(count);
    }
}
