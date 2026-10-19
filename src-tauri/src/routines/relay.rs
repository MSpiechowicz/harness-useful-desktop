//! The chat of an agent run without a page of its own (see "Agent runs" in
//! `docs/routines-format.md`).
//!
//! - [`RelaySink`] is the run's [`ChatSink`]: it keeps the chat's events for a window that opens
//!   it later (at most [`MAX_REPLAY`] events and [`MAX_REPLAY_BYTES`] of them as JSON: the 3
//!   startup events, a notice for the ones left out, the newest), forwards them to the attached
//!   window, and tells the run's monitor what matters to it ([`Signal`]).
//! - `routine_run_attach` attaches a window: it gets the kept events in order, then the live
//!   ones. A window whose channel fails (closed tab or page) is detached; the agent keeps running.
//! - [`Relays`] holds the relays of the live runs (from before their chat starts), and of the
//!   ended runs whose chat a window still shows, by run id: a page reload spares their agents
//!   ([`Relays::running_keys`]) and detaches every window ([`Relays::detach_all`]).
//! - A click on a "Routine needs you" notification opens the chat of the run it named, while that
//!   run still waits ([`RoutineRuntime::on_notification_click`]). Other notifications only bring
//!   the window to the front.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use harness_business::agent::{EventSink, PromptTaken};
use harness_business::model::{
    AgentEnvelope, AgentEvent, ChatBlock, MessageRole, NoticeLevel, PromptStatus, RoutinesEvent,
};
use harness_business::{BusinessError, Result};
use tauri::ipc::Channel;
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::{RoutineRuntime, notify};
use crate::agent_sink::ChatSink;

/// Events kept for a window that attaches later.
pub const MAX_REPLAY: usize = 20_000;

/// Bytes of events (as JSON) kept for a window that attaches later.
pub const MAX_REPLAY_BYTES: usize = 32 * 1024 * 1024;

/// The events every chat starts with (`ready`, `state`, `commandsUpdated`), always kept.
const STARTUP_EVENTS: usize = 3;

/// The notice in place of the events left out.
pub const TRIMMED_NOTICE: &str = "Earlier events of this routine run are not shown.";

/// What the run's monitor hears about its chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// The harness session id (startup, or a new session).
    Session(String),
    /// The agent is idle.
    Settled,
    /// The agent took the run's prompt: every signal before this one came before it.
    PromptTaken,
    /// A prompt finished; `failed` when with an error.
    PromptDone {
        /// The prompt ended with an error.
        failed: bool,
        /// The harness handled the prompt itself (a `/command`): no run and no settle follow.
        handled: bool,
    },
    /// The text of an assistant message.
    Said(String),
    /// The agent asks the user (request id).
    Asked(String),
    /// The agent withdrew request `id`, or a window answered it.
    Closed(String),
    /// The agent exited.
    Exited,
    /// A window attached (`true`) or left (`false`).
    Attached(bool),
}

/// The signal `event` gives the monitor, if any.
fn signal_of(event: &AgentEvent) -> Option<Signal> {
    match event {
        AgentEvent::State { state } => Some(Signal::Session(state.session_id.clone())),
        AgentEvent::Settled { .. } => Some(Signal::Settled),
        AgentEvent::PromptResult { status, .. } => Some(Signal::PromptDone {
            failed: *status == PromptStatus::Error,
            handled: *status == PromptStatus::Handled,
        }),
        AgentEvent::MessageEnd { message, .. } if message.role == MessageRole::Assistant => {
            let text: Vec<&str> = message
                .blocks
                .iter()
                .filter_map(|block| match block {
                    ChatBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();

            let text = text.join("\n");
            (!text.trim().is_empty()).then_some(Signal::Said(text))
        }
        AgentEvent::UiRequest { request } => Some(Signal::Asked(request.id.clone())),
        AgentEvent::UiCancel { request_id } => Some(Signal::Closed(request_id.clone())),
        AgentEvent::Exited { .. } => Some(Signal::Exited),
        _ => None,
    }
}

/// See the module docs.
pub struct RelaySink {
    run_id: String,
    signals: mpsc::UnboundedSender<Signal>,
    relay: Mutex<Relay>,
}

struct Relay {
    /// The session key, from the first event.
    key: Option<String>,
    startup: Vec<AgentEnvelope>,
    /// In place of the events left out, once some were.
    trimmed: Option<AgentEnvelope>,
    /// The newest events, each with its size.
    tail: VecDeque<(AgentEnvelope, usize)>,
    /// The size of `tail`.
    tail_bytes: usize,
    /// The most bytes `tail` keeps.
    budget: usize,
    window: Option<Channel<AgentEnvelope>>,
    /// The run ended while a window was attached: the chat stays for it.
    ended: bool,
    /// Since when the run waits for an answer.
    waiting_since: Option<Instant>,
}

impl Relay {
    fn new(budget: usize) -> Self {
        Self {
            key: None,
            startup: Vec::new(),
            trimmed: None,
            tail: VecDeque::new(),
            tail_bytes: 0,
            budget,
            window: None,
            ended: false,
            waiting_since: None,
        }
    }

    fn keep(&mut self, envelope: AgentEnvelope) {
        if self.startup.len() < STARTUP_EVENTS && self.trimmed.is_none() && self.tail.is_empty() {
            self.startup.push(envelope);
            return;
        }

        let size = serde_json::to_vec(&envelope).map_or(0, |json| json.len());
        self.tail.push_back((envelope, size));
        self.tail_bytes += size;

        while self.over_limit() {
            let Some((dropped, size)) = self.tail.pop_front() else {
                break;
            };

            self.tail_bytes -= size;

            // Numbered like the newest event left out, so the replay stays in `seq` order.
            self.trimmed = Some(AgentEnvelope {
                session_key: dropped.session_key,
                seq: dropped.seq,
                event: AgentEvent::Notice {
                    level: NoticeLevel::Info,
                    message: TRIMMED_NOTICE.to_owned(),
                },
            });
        }
    }

    /// More events than [`MAX_REPLAY`], or more bytes than the budget (the newest event is
    /// always kept).
    fn over_limit(&self) -> bool {
        let events = self.startup.len() + usize::from(self.trimmed.is_some()) + self.tail.len();

        events > MAX_REPLAY || (self.tail_bytes > self.budget && self.tail.len() > 1)
    }

    fn replay(&self) -> impl Iterator<Item = &AgentEnvelope> {
        self.startup
            .iter()
            .chain(&self.trimmed)
            .chain(self.tail.iter().map(|(envelope, _)| envelope))
    }
}

impl RelaySink {
    /// The relay of run `run_id`, and what its monitor hears.
    pub fn new(run_id: &str) -> (Arc<Self>, mpsc::UnboundedReceiver<Signal>) {
        Self::with_budget(run_id, MAX_REPLAY_BYTES)
    }

    /// Like [`RelaySink::new`], keeping at most `budget` bytes of events.
    fn with_budget(run_id: &str, budget: usize) -> (Arc<Self>, mpsc::UnboundedReceiver<Signal>) {
        let (signals, received) = mpsc::unbounded_channel();

        let sink = Self {
            run_id: run_id.to_owned(),
            signals,
            relay: Mutex::new(Relay::new(budget)),
        };

        (Arc::new(sink), received)
    }

    fn lock(&self) -> MutexGuard<'_, Relay> {
        // Every change leaves the buffers complete, so a panic cannot leave them half-written.
        self.relay.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// What the session calls when its agent takes the prompt: the monitor hears
    /// [`Signal::PromptTaken`] in event order.
    pub fn prompt_taken(&self) -> PromptTaken {
        let signals = self.signals.clone();

        Box::new(move || {
            signals.send(Signal::PromptTaken).ok();
        })
    }

    /// The run.
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// The chat's session key, once its first event arrived.
    pub fn key(&self) -> Option<String> {
        self.lock().key.clone()
    }

    /// True while a window shows the chat.
    pub fn is_attached(&self) -> bool {
        self.lock().window.is_some()
    }

    /// Sends the kept events to `window` in order, then the live ones; a window attached before
    /// gets no more. Answers the session key. `notFound` before the chat started, `process` when
    /// the window is gone already.
    pub fn attach(&self, window: Channel<AgentEnvelope>) -> Result<String> {
        let mut relay = self.lock();

        let Some(key) = relay.key.clone() else {
            return Err(BusinessError::NotFound("the run's chat".to_owned()));
        };

        for envelope in relay.replay() {
            if window.send(envelope.clone()).is_err() {
                return Err(BusinessError::Process("the window closed".to_owned()));
            }
        }

        relay.window = Some(window);
        self.signals.send(Signal::Attached(true)).ok();

        Ok(key)
    }

    /// Detaches the window, if one is attached (its page is gone).
    pub fn detach(&self) {
        let attached = self.lock().window.take().is_some();

        if attached {
            self.signals.send(Signal::Attached(false)).ok();
        }
    }

    /// The run ended: true when a window shows the chat, which then stays for it until the window
    /// leaves or the agent exits.
    pub fn finish(&self) -> bool {
        let mut relay = self.lock();
        relay.ended = true;

        relay.window.is_some()
    }

    /// Records whether the run waits for an answer.
    pub fn set_waiting(&self, waiting: bool) {
        let mut relay = self.lock();

        match (waiting, relay.waiting_since) {
            (true, None) => relay.waiting_since = Some(Instant::now()),
            (false, _) => relay.waiting_since = None,
            (true, Some(_)) => {}
        }
    }

    /// A window answered request `id`.
    fn answered(&self, id: &str) {
        self.signals.send(Signal::Closed(id.to_owned())).ok();
    }
}

impl EventSink for RelaySink {
    fn emit(&self, envelope: AgentEnvelope) {
        if let Some(signal) = signal_of(&envelope.event) {
            // A monitor that is gone (the run ended) needs nothing.
            self.signals.send(signal).ok();
        }

        let mut relay = self.lock();
        relay.key.get_or_insert_with(|| envelope.session_key.clone());

        let failed = relay
            .window
            .as_ref()
            .is_some_and(|window| window.send(envelope.clone()).is_err());

        relay.keep(envelope);

        if failed {
            relay.window = None;
            drop(relay);

            tracing::info!(run = self.run_id, "a window left the chat of a routine run");
            self.signals.send(Signal::Attached(false)).ok();
        }
    }
}

impl ChatSink for RelaySink {
    /// Never: the chat runs whether a window shows it or not.
    fn is_closed(&self) -> bool {
        false
    }
}

/// See the module docs.
#[derive(Default)]
pub struct Relays {
    relays: Mutex<HashMap<String, Arc<RelaySink>>>,
    /// How many times the page reloaded ([`Relays::detach_all`]).
    reloads: AtomicU64,
}

impl Relays {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Arc<RelaySink>>> {
        // The map holds no invariant a panic could break.
        self.relays.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps `relay` (its chat started).
    pub fn insert(&self, relay: Arc<RelaySink>) {
        self.lock().insert(relay.run_id.clone(), relay);
    }

    /// Forgets the relay of run `run_id`.
    pub fn remove(&self, run_id: &str) {
        self.lock().remove(run_id);
    }

    /// The relay of run `run_id`.
    pub fn get(&self, run_id: &str) -> Option<Arc<RelaySink>> {
        self.lock().get(run_id).cloned()
    }

    /// The session keys of the runs going on: a page reload does not stop them.
    pub fn running_keys(&self) -> HashSet<String> {
        self.lock()
            .values()
            .filter_map(|relay| {
                let relay = relay.lock();
                relay.key.clone().filter(|_| !relay.ended)
            })
            .collect()
    }

    /// A window answered request `id` of chat `key`: its run no longer waits for it.
    pub fn answered(&self, key: &str, id: &str) {
        let relays: Vec<Arc<RelaySink>> = self.lock().values().cloned().collect();

        if let Some(relay) = relays.iter().find(|relay| relay.key().as_deref() == Some(key)) {
            relay.answered(id);
        }
    }

    /// How many times the page reloaded so far.
    pub fn reloads(&self) -> u64 {
        self.reloads.load(Ordering::SeqCst)
    }

    /// Detaches every window (the page reloaded).
    pub fn detach_all(&self) {
        self.reloads.fetch_add(1, Ordering::SeqCst);
        let relays: Vec<Arc<RelaySink>> = self.lock().values().cloned().collect();

        for relay in relays {
            relay.detach();
        }
    }

    /// True while run `run_id` waits for an answer.
    fn waiting(&self, run_id: &str) -> bool {
        self.get(run_id)
            .is_some_and(|relay| relay.lock().waiting_since.is_some())
    }
}

impl RoutineRuntime {
    /// The chats of the agent runs.
    pub fn relays(&self) -> &Relays {
        &self.relays
    }

    /// The user clicked a notification whose extra data is `extra`: for a "Routine needs you"
    /// notification ([`notify::needs_you_run`]) whose run still waits for an answer, the Routines
    /// view opens its chat (`open`). Any other notification opens nothing.
    pub fn on_notification_click(&self, extra: &HashMap<String, serde_json::Value>) {
        let Some(run_id) = notify::needs_you_run(extra) else {
            return;
        };

        if self.relays.waiting(&run_id) {
            self.emit(RoutinesEvent::Open { run_id });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::collecting_channel;

    fn envelope(seq: u64) -> AgentEnvelope {
        AgentEnvelope {
            session_key: "key-1".to_owned(),
            seq,
            event: AgentEvent::TurnStart,
        }
    }

    #[test]
    fn a_window_gets_the_startup_events_a_notice_and_the_newest_in_order_past_the_limit() {
        let (relay, _signals) = RelaySink::new("run");
        let last = MAX_REPLAY as u64 + 10;

        for seq in 1..=last {
            relay.emit(envelope(seq));
        }

        let (window, received) = collecting_channel();
        assert_eq!(relay.attach(window).expect("attached"), "key-1");
        relay.emit(envelope(last + 1));

        let received = received.lock().unwrap();
        let seqs: Vec<u64> = received
            .iter()
            .map(|message| message["seq"].as_u64().unwrap())
            .collect();

        assert_eq!(received.len(), MAX_REPLAY + 1);
        assert_eq!(seqs[..3], [1, 2, 3]);
        assert!(seqs.windows(2).all(|pair| pair[0] < pair[1]), "in seq order");
        assert_eq!(seqs.last(), Some(&(last + 1)), "then the live events");
        assert_eq!(received[3]["event"]["type"], "notice");
        assert_eq!(received[3]["event"]["message"], TRIMMED_NOTICE);
    }

    #[test]
    fn the_kept_events_stay_within_their_byte_budget() {
        let budget = 2_000;
        let (relay, _signals) = RelaySink::with_budget("run", budget);

        for seq in 1..=100 {
            relay.emit(AgentEnvelope {
                session_key: "key-1".to_owned(),
                seq,
                event: AgentEvent::Notice {
                    level: NoticeLevel::Info,
                    message: "x".repeat(100),
                },
            });
        }

        let (window, received) = collecting_channel();
        relay.attach(window).expect("attached");

        let received = received.lock().unwrap();
        let tail = &received[4..];
        let tail_bytes: usize = tail.iter().map(|message| message.to_string().len()).sum();

        assert!(
            tail.len() > 1 && tail_bytes <= budget,
            "{} events, {tail_bytes} bytes",
            tail.len()
        );
        assert_eq!(received[3]["event"]["message"], TRIMMED_NOTICE);
        assert_eq!(received.last().unwrap()["seq"], 100, "the newest are kept");
    }
}
