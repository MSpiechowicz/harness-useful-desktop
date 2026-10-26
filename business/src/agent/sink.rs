//! Where a session's events go, and the task that numbers and batches them.
//!
//! [`EventSink`] is implemented by the desktop shell (a Tauri channel) and by tests
//! ([`CollectingSink`]). Every session has one emitter task that owns the sequence counter, so
//! `seq` is strictly increasing however many tasks produce events. Until startup completes the
//! emitter holds events back: the stream always begins with `ready`, `state` and
//! `commandsUpdated`, and a session that fails to start emits nothing.
//!
//! A process can exit right after its handshake, before the engine opened the session: its
//! `exited` and `Close` then arrive while the events are still held. The emitter keeps them and
//! waits: `Open` delivers them (`exited` last) and ends the task, while [`EmitMsg::Discard`] or the
//! end of the channel (every sender dropped, so the start failed) drops them.

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
    /// The process exited: deliver what is buffered and stop. Before [`EmitMsg::Open`], keep the
    /// held events and stop once the session opens (or is discarded, or every sender is gone).
    Close,
    /// The session will never open (a failed or abandoned start, a stop or drop): drop the held
    /// events and stop. Ignored once open.
    Discard,
    /// Run this once every event sent before it was delivered (see [`PromptTaken`]).
    Call(PromptTaken),
}

/// Called in the session's event order at the moment the agent takes the next prompt: after every
/// event it sent before its answer, before any that follows.
pub type PromptTaken = Box<dyn FnOnce() + Send>;

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
    // The process closed before the session opened.
    let mut closed = false;

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

                if closed {
                    delivery.send(coalescer.flush());
                    break;
                }
            }
            Some(EmitMsg::Call(call)) => {
                if held.is_none() {
                    delivery.send(coalescer.flush());
                }

                call();
            }
            Some(EmitMsg::Close) if held.is_some() => closed = true,
            Some(EmitMsg::Discard) if held.is_some() => break,
            Some(EmitMsg::Discard) => {}
            // Held events of a session that never opened are dropped.
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::mpsc::UnboundedSender;
    use tokio::task::JoinHandle;

    use super::{CollectingSink, EmitMsg, spawn_emitter};
    use crate::model::AgentEvent;

    const LIMIT: Duration = Duration::from_secs(5);

    fn ready() -> AgentEvent {
        AgentEvent::Ready { protocol_version: None }
    }

    fn line(text: &str) -> AgentEvent {
        AgentEvent::Stderr { line: text.to_owned() }
    }

    fn exited() -> AgentEvent {
        AgentEvent::Exited {
            code: Some(0),
            signal: None,
        }
    }

    fn emitter() -> (Arc<CollectingSink>, UnboundedSender<EmitMsg>, JoinHandle<()>) {
        let sink = CollectingSink::new();
        let (sender, task) = spawn_emitter("chat".to_owned(), sink.clone(), Duration::ZERO);

        (sink, sender, task)
    }

    fn event(event: AgentEvent) -> EmitMsg {
        EmitMsg::Event(Box::new(event))
    }

    /// Waits until the emitter task ended; panics when it keeps running.
    async fn ended(task: JoinHandle<()>) {
        tokio::time::timeout(LIMIT, task)
            .await
            .expect("the emitter stopped")
            .expect("the emitter did not panic");
    }

    #[tokio::test]
    async fn a_process_that_exits_before_the_session_opens_still_delivers_its_events_on_open() {
        let (sink, sender, task) = emitter();

        sender.send(event(line("starting"))).ok();
        sender.send(event(exited())).ok();
        sender.send(EmitMsg::Close).ok();
        tokio::task::yield_now().await;
        assert!(sink.events().is_empty());

        sender.send(EmitMsg::Open(vec![ready()])).ok();
        ended(task).await;

        assert_eq!(sink.events(), vec![ready(), line("starting"), exited()]);
        let seqs: Vec<u64> = sink.envelopes().iter().map(|envelope| envelope.seq).collect();
        assert_eq!(seqs, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn a_closed_session_that_never_opens_emits_nothing() {
        let (sink, sender, task) = emitter();

        sender.send(event(exited())).ok();
        sender.send(EmitMsg::Close).ok();
        drop(sender);
        ended(task).await;

        assert!(sink.events().is_empty());
    }

    #[tokio::test]
    async fn a_discarded_session_stops_without_emitting_while_its_senders_live() {
        let (sink, sender, task) = emitter();

        sender.send(event(exited())).ok();
        sender.send(EmitMsg::Close).ok();
        sender.send(EmitMsg::Discard).ok();
        ended(task).await;

        sender.send(EmitMsg::Open(vec![ready()])).ok();
        assert!(sink.events().is_empty());
    }

    #[tokio::test]
    async fn an_open_session_ignores_discard_and_delivers_until_close() {
        let (sink, sender, task) = emitter();

        sender.send(EmitMsg::Open(vec![ready()])).ok();
        sender.send(EmitMsg::Discard).ok();
        sender.send(event(exited())).ok();
        sender.send(EmitMsg::Close).ok();
        ended(task).await;

        assert_eq!(sink.events(), vec![ready(), exited()]);
    }
}
