//! Memory recall in chats: the first message of a new chat is sent with the notes the installed
//! `harness-memory` server finds for it in front (see
//! [`harness_business::ecosystem::memory_recall`]).
//!
//! - Every chat but a utility agent is tracked. A chat is armed when it starts fresh (not resumed)
//!   and after a successful `NewSession`. A successful switch, fork or clone disarms it; stopping
//!   it, its channel closing, or a page reload forgets it.
//! - Arming starts the server in the background while the `memoryEnabled` and `memoryRecall` prefs
//!   are on, so the first message usually pays only for the search; `agent_start` never waits. A
//!   warm server unused for [`WARM_IDLE`] is stopped (the recall then starts its own), and at most
//!   [`WARM_MAX`] run at once: arming another chat stops the oldest.
//! - The first user message decides: a prompt without `streamingBehavior` that is not a command or
//!   a shell line is recalled; any other prompt, steer or follow-up only disarms. Such a prompt
//!   refused before it reached the agent (it failed validation, or the chat is gone) arms the chat
//!   again; one the agent received and failed is never recalled twice. Each chat has its own lock,
//!   held until the recalled message is sent, so a later message cannot overtake it and other
//!   chats never wait.
//! - A recall never fails the send: on any problem the message goes out as written. Logs carry the
//!   outcome and the note count only.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use harness_business::ecosystem::memory_recall::recall_eligible;
use harness_business::ecosystem::{RecallOutcome, RecallReason, WarmRecall};
use harness_business::model::{AgentAction, AgentReply};
use harness_business::{BusinessError, Result};
use tauri::async_runtime::{self, JoinHandle};

use crate::state::{AppState, SharedPrefs};

/// How long a warm server waits for its chat's first message before it is stopped.
pub const WARM_IDLE: Duration = Duration::from_secs(60);

/// Most warm servers at once, over every chat.
pub const WARM_MAX: usize = 4;

/// The tracked chats and their recall state. See the module docs.
pub struct RecallGate {
    slots: Mutex<Slots>,
    idle: Duration,
    max_warm: usize,
}

#[derive(Default)]
struct Slots {
    chats: HashMap<String, Entry>,
    /// The order of the next inserted slot.
    next: u64,
}

struct Entry {
    /// Insertion order: the lowest is the oldest.
    order: u64,
    slot: Arc<async_runtime::Mutex<Slot>>,
}

/// One chat's recall state.
#[derive(Default)]
struct Slot {
    armed: bool,
    warm: Option<WarmTask>,
}

impl Default for RecallGate {
    fn default() -> Self {
        Self::new(WARM_IDLE, WARM_MAX)
    }
}

impl RecallGate {
    /// A gate whose warm servers stop after `idle` unused, at most `max_warm` at once.
    pub fn new(idle: Duration, max_warm: usize) -> Self {
        Self {
            slots: Mutex::default(),
            idle,
            max_warm,
        }
    }

    /// Puts `slot` in for chat `key`; a previous slot and its server are dropped. The oldest warm
    /// servers beyond the limit are stopped.
    fn insert(&self, key: &str, slot: Slot) {
        let mut slots = self.lock();
        let order = slots.next;
        slots.next += 1;

        let slot = Arc::new(async_runtime::Mutex::new(slot));
        slots.chats.insert(key.to_owned(), Entry { order, slot });

        self.limit_warm(&slots);
    }

    /// Stops the oldest warm servers beyond [`RecallGate::max_warm`]. A slot in use (its message is
    /// being recalled) has already taken its server and is skipped.
    fn limit_warm(&self, slots: &Slots) {
        let mut warm: Vec<_> = slots
            .chats
            .values()
            .filter_map(|entry| {
                let slot = entry.slot.try_lock().ok()?;
                let alive = slot.warm.as_ref().is_some_and(WarmTask::is_alive);

                alive.then_some((entry.order, slot))
            })
            .collect();

        warm.sort_by_key(|(order, _)| *order);
        let excess = warm.len().saturating_sub(self.max_warm);

        for (_, mut slot) in warm.into_iter().take(excess) {
            slot.warm = None;
        }
    }

    /// Disarms chat `key` and drops its warm server. Waits for a message being recalled in it.
    pub async fn disarm(&self, key: &str) {
        let Some(slot) = self.slot(key) else {
            return;
        };

        let mut slot = slot.lock().await;
        slot.armed = false;
        slot.warm = None;
    }

    /// Forgets chat `key` (it stopped); its warm server is dropped.
    pub fn forget(&self, key: &str) {
        self.lock().chats.remove(key);
    }

    /// Forgets every chat (the page reloaded, so every chat stopped); their warm servers are
    /// dropped.
    pub fn clear(&self) {
        self.lock().chats.clear();
    }

    /// Forgets every chat that is not in `live` (stopped without a stop call).
    fn retain(&self, live: &HashSet<String>) {
        self.lock().chats.retain(|key, _| live.contains(key));
    }

    fn slot(&self, key: &str) -> Option<Arc<async_runtime::Mutex<Slot>>> {
        self.lock().chats.get(key).map(|entry| entry.slot.clone())
    }

    fn lock(&self) -> MutexGuard<'_, Slots> {
        // The map holds no invariant a panic could break.
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A server warming up in the background, then waiting for its chat's first message.
///
/// The server stays in [`WarmTask::server`] until it is taken with [`WarmTask::take`], or until it
/// was unused for the idle time. Dropping the task stops the server.
struct WarmTask {
    server: Arc<Mutex<Warm>>,
    task: JoinHandle<()>,
}

enum Warm {
    Starting,
    Ready(Box<WarmRecall>),
    /// Taken, failed to start, or stopped after the idle time.
    Gone,
}

impl WarmTask {
    fn spawn(warm_up: impl Future<Output = Option<WarmRecall>> + Send + 'static, idle: Duration) -> Self {
        let server = Arc::new(Mutex::new(Warm::Starting));
        let cell = server.clone();

        let task = async_runtime::spawn(async move {
            let Some(warm) = warm_up.await else {
                set(&cell, Warm::Gone);
                return;
            };

            set(&cell, Warm::Ready(Box::new(warm)));
            tokio::time::sleep(idle).await;
            set(&cell, Warm::Gone);
        });

        Self { server, task }
    }

    /// True while the server is starting or ready.
    fn is_alive(&self) -> bool {
        !matches!(*lock(&self.server), Warm::Gone)
    }

    /// The warm server when it is ready. A warm-up still running is cancelled: the recall then
    /// starts its own server, within its own time limit.
    fn take(self) -> Option<WarmRecall> {
        match std::mem::replace(&mut *lock(&self.server), Warm::Gone) {
            Warm::Ready(warm) => Some(*warm),
            Warm::Starting | Warm::Gone => None,
        }
    }
}

impl Drop for WarmTask {
    fn drop(&mut self) {
        self.task.abort();
        set(&self.server, Warm::Gone);
    }
}

fn set(server: &Mutex<Warm>, state: Warm) {
    let previous = std::mem::replace(&mut *lock(server), state);

    // A server dropped here is killed outside the lock.
    drop(previous);
}

fn lock(server: &Mutex<Warm>) -> MutexGuard<'_, Warm> {
    // Every write replaces the whole value, so a panic cannot leave it half-written.
    server.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Tracks chat `key` in `project` (any chat but a utility agent): a fresh chat is armed, a resumed
/// one waits for a `NewSession`. Never waits.
pub fn track(state: &AppState, key: &str, project: &str, fresh: bool) {
    if fresh {
        arm(state, key, project);
    } else {
        register(state, key, Slot::default());
    }
}

/// Arms chat `key` in `project` and, when the prefs allow a recall and the server is found, warms
/// the server up in the background. Never waits.
fn arm(state: &AppState, key: &str, project: &str) {
    let prefs = state.prefs.clone();
    let recall = state.memory_recall.clone();
    let server = state.memory_server.clone();
    let env = state.env.clone();
    let runner = state.runner;
    let project = PathBuf::from(project);

    let warm_up = async move {
        if !recall_allowed(prefs).await {
            return None;
        }

        let server = server.get(&env, &runner).await?;
        recall.warm(&server, &project).await
    };

    let warm = WarmTask::spawn(warm_up, state.recall.idle);
    register(
        state,
        key,
        Slot {
            armed: true,
            warm: Some(warm),
        },
    );
}

/// Puts `slot` in for chat `key` while the chat runs, and forgets the chats that stopped without a
/// stop call. A chat whose channel closed meanwhile (its forget may have run before this insert)
/// keeps no slot: the session leaves the manager before the slot is forgotten, so the check after
/// the insert sees it.
fn register(state: &AppState, key: &str, slot: Slot) {
    let manager = state.agents.manager();
    let live: HashSet<String> = manager.list().into_iter().map(|info| info.key).collect();

    state.recall.retain(&live);

    if !live.contains(key) {
        return;
    }

    state.recall.insert(key, slot);

    if manager.info(key).is_err() {
        state.recall.forget(key);
    }
}

/// Runs `action` in chat `key`; the first message of an armed chat is recalled first.
pub async fn send(state: &AppState, key: &str, action: AgentAction) -> Result<AgentReply> {
    let manager = state.agents.manager();

    match &action {
        AgentAction::Prompt { .. } | AgentAction::Steer { .. } | AgentAction::FollowUp { .. } => {
            send_message(state, key, action).await
        }
        AgentAction::NewSession => {
            let reply = manager.send(key, action).await?;

            // Only tracked chats are armed again: utility agents never are.
            if state.recall.slot(key).is_some()
                && let Ok(info) = manager.info(key)
            {
                arm(state, key, &info.project);
            }

            Ok(reply)
        }
        AgentAction::SwitchSession { .. } | AgentAction::Fork { .. } | AgentAction::Clone => {
            let reply = manager.send(key, action).await?;
            state.recall.disarm(key).await;

            Ok(reply)
        }
        _ => manager.send(key, action).await,
    }
}

/// Sends a user message; when the chat is armed, disarms it and recalls the message first.
async fn send_message(state: &AppState, key: &str, action: AgentAction) -> Result<AgentReply> {
    let manager = state.agents.manager();

    let Some(entry) = state.recall.slot(key) else {
        return manager.send(key, action).await;
    };

    // Held until the message is sent, so a later message waits instead of overtaking it.
    let mut slot = entry.lock().await;
    let armed = std::mem::take(&mut slot.armed);
    let warm = slot.warm.take();

    if !armed {
        drop(slot);
        return manager.send(key, action).await;
    }

    let eligible = recall_candidate(&action);
    let action = recalled(state, key, action, warm).await;
    let sent = manager.send(key, action).await;

    // Refused before it reached the agent, so the next message decides again.
    if eligible && sent.as_ref().is_err_and(never_delivered) {
        slot.armed = true;
    }

    sent
}

/// True for a prompt a recall applies to: no `streamingBehavior`, not a command or a shell line.
fn recall_candidate(action: &AgentAction) -> bool {
    match action {
        AgentAction::Prompt {
            message,
            streaming_behavior: None,
            ..
        } => recall_eligible(message),
        _ => false,
    }
}

/// True for a send error raised before the message was written to the agent: the action failed
/// validation, or the chat is gone. Any later error (a timeout, a protocol error, the agent
/// refusing it or exiting) may follow a delivered message, which must not be recalled twice.
fn never_delivered(error: &BusinessError) -> bool {
    matches!(
        error,
        BusinessError::InvalidInput(_)
            | BusinessError::NotFound(_)
            | BusinessError::Forbidden(_)
            | BusinessError::Unsupported(_)
    )
}

/// `action` with the recall block in front of its text when it is an eligible prompt and the
/// server found notes; otherwise `action` as it is.
async fn recalled(state: &AppState, key: &str, action: AgentAction, warm: Option<WarmTask>) -> AgentAction {
    let (message, images) = match action {
        AgentAction::Prompt {
            message,
            images,
            streaming_behavior: None,
        } => (message, images),
        other => return other,
    };

    let message = match recall(state, key, &message, warm).await {
        RecallOutcome::Recalled { message, notes } => {
            tracing::debug!(outcome = "recalled", notes, "memory recall");
            message
        }
        RecallOutcome::Unchanged(reason) => {
            tracing::debug!(outcome = reason.as_str(), notes = 0, "memory recall");
            message
        }
    };

    AgentAction::Prompt {
        message,
        images,
        streaming_behavior: None,
    }
}

async fn recall(state: &AppState, key: &str, message: &str, warm: Option<WarmTask>) -> RecallOutcome {
    let skipped = RecallOutcome::Unchanged(RecallReason::Skipped);

    if !recall_eligible(message) {
        return skipped;
    }

    if !recall_allowed(state.prefs.clone()).await {
        return skipped;
    }

    let Some(server) = state.memory_server.get(&state.env, &state.runner).await else {
        return skipped;
    };

    let Ok(info) = state.agents.manager().info(key) else {
        return skipped;
    };

    let warm = warm.and_then(WarmTask::take);

    state
        .memory_recall
        .recall(warm, &server, Path::new(&info.project), message)
        .await
}

/// True while both `memoryEnabled` and `memoryRecall` are on.
async fn recall_allowed(prefs: SharedPrefs) -> bool {
    let read = async_runtime::spawn_blocking(move || {
        prefs.with(|store| {
            let prefs = store.load();
            prefs.memory_enabled && prefs.memory_recall
        })
    })
    .await;

    read.unwrap_or(false)
}

#[cfg(test)]
mod tests;
