//! The [`AgentManager`](super::AgentManager)'s bookkeeping: running and stopping agents, the
//! session ids they hold, start reservations and session claims.
//!
//! Session ids are held regardless of the harness: one folder can serve OMP and PI (a shared
//! `PI_CODING_AGENT_SESSION_DIR` or a project `sessionDir`), so an OMP agent and a PI agent of
//! one id would write the same file.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError, Weak};

use super::engine::AgentSession;
use super::sink::EventSink;
use crate::error::BusinessError;
use crate::model::{AgentEnvelope, AgentEvent, AgentSessionInfo};

/// The harness session id an agent works on, when known.
pub(super) type Bound = Arc<Mutex<Option<String>>>;

pub(super) fn bind(bound: &Bound, id: &str) {
    if id.is_empty() {
        return;
    }

    *bound.lock().unwrap_or_else(PoisonError::into_inner) = Some(id.to_owned());
}

fn is_bound(bound: &Bound, id: &str) -> bool {
    bound.lock().unwrap_or_else(PoisonError::into_inner).as_deref() == Some(id)
}

/// Binds the agent to the session id of every `state` it reports, then forwards the event.
pub(super) struct BindingSink {
    pub inner: Arc<dyn EventSink>,
    pub bound: Bound,
}

impl EventSink for BindingSink {
    fn emit(&self, envelope: AgentEnvelope) {
        if let AgentEvent::State { state } = &envelope.event {
            bind(&self.bound, &state.session_id);
        }

        self.inner.emit(envelope);
    }
}

pub(super) struct Entry {
    pub info: AgentSessionInfo,
    pub session: Arc<AgentSession>,
    pub bound: Bound,
    /// The chat has a memory identity and its harness supports a chat's own guards.
    pub guards_offered: bool,
}

impl Entry {
    /// The info with whether the chat's own guards apply as of now (a Codex chat learns it again
    /// for every new thread).
    pub fn info(&self) -> AgentSessionInfo {
        AgentSessionInfo {
            memory_guards: self.guards_offered && self.session.memory_passed_on(),
            ..self.info.clone()
        }
    }
}

/// An agent taken out of the manager that may still be running. Held weakly: a
/// [`Stopping`](super::Stopping) dropped without `finish` drops (and so kills) its agents.
struct Parked {
    bound: Bound,
    session: Weak<AgentSession>,
}

/// What holds a session id.
pub(super) enum Holder {
    /// A running agent, a start that claimed the id, or a [`SessionClaim`].
    Open,
    /// An agent that is stopping, or a start that every session was stopped during (it stops its
    /// own agent).
    Stopping,
}

/// Who claimed a session id.
pub(super) enum Claimant {
    /// A start reserved at this [`Slots::generation`].
    Start { generation: u64 },
    /// A [`SessionClaim`].
    Other,
}

#[derive(Default)]
pub(super) struct Slots {
    pub sessions: HashMap<String, Entry>,
    stopping: Vec<Parked>,
    /// Session ids of starts in progress and of [`SessionClaim`]s.
    pub claimed: HashMap<String, Claimant>,
    pub starting: usize,
    /// Bumped by every stop of all sessions; a start reserved before the bump must not register.
    pub generation: u64,
}

impl Slots {
    /// Forgets agents that exited.
    pub fn prune(&mut self) {
        self.sessions.retain(|_, entry| entry.session.is_running());
        self.stopping
            .retain(|parked| parked.session.upgrade().is_some_and(|session| session.is_running()));
    }

    /// What holds session `id`, ignoring the running agent of session key `except`.
    pub fn holder(&mut self, id: &str, except: Option<&str>) -> Option<Holder> {
        self.prune();

        let running = self
            .sessions
            .iter()
            .any(|(key, entry)| Some(key.as_str()) != except && is_bound(&entry.bound, id));

        if running {
            return Some(Holder::Open);
        }

        match self.claimed.get(id) {
            Some(Claimant::Start { generation }) if *generation != self.generation => return Some(Holder::Stopping),
            Some(_) => return Some(Holder::Open),
            None => {}
        }

        let stopping = self.stopping.iter().any(|parked| is_bound(&parked.bound, id));

        stopping.then_some(Holder::Stopping)
    }

    /// Claims session `id` for `claimant`; `None` while anything holds it.
    pub fn claim(&mut self, id: &str, claimant: Claimant) -> Option<String> {
        if self.holder(id, None).is_some() {
            return None;
        }

        self.claimed.insert(id.to_owned(), claimant);
        Some(id.to_owned())
    }

    /// Moves `entry` to the stopping agents; returns its session for [`Stopping`](super::Stopping).
    pub fn park(&mut self, entry: Entry) -> Arc<AgentSession> {
        self.stopping.push(Parked {
            bound: entry.bound,
            session: Arc::downgrade(&entry.session),
        });

        entry.session
    }
}

pub(super) fn open_elsewhere() -> BusinessError {
    BusinessError::Unsupported("this session is already open in another chat".to_owned())
}

pub(super) fn still_stopping() -> BusinessError {
    BusinessError::Timeout("the previous agent of this session is still stopping".to_owned())
}

/// Releases a reserved start slot and the session id it claimed.
pub(super) struct Reservation<'a> {
    pub slots: &'a Mutex<Slots>,
    pub claim: Option<String>,
    /// [`Slots::generation`] when the slot was reserved.
    pub generation: u64,
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        slots.starting = slots.starting.saturating_sub(1);

        if let Some(claim) = &self.claim {
            slots.claimed.remove(claim);
        }
    }
}

/// A session id held by something other than an agent (deleting the session's files, a switch
/// in flight): no agent may resume or switch to it until the claim is dropped. See
/// [`AgentManager::claim_session`](super::AgentManager::claim_session).
#[must_use = "the session id is released when the claim is dropped"]
pub struct SessionClaim<'a> {
    pub(super) slots: &'a Mutex<Slots>,
    pub(super) claim: String,
}

impl Drop for SessionClaim<'_> {
    fn drop(&mut self) {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        slots.claimed.remove(&self.claim);
    }
}
