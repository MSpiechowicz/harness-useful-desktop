//! One chat's hooks: its `HookSession`, fed by every send ([`send`]) and by the events its
//! [`HookSink`] passes on, and the task that carries out what the engine asks for.
//!
//! - A user message takes the chat's lock, asks the engine for context, runs the command hooks
//!   of the firing, puts the context in front of the prompt and sends it before it lets go, so a
//!   later message (or a hook follow-up) never overtakes it and other chats never wait. A message
//!   refused before it reached the agent is undone in the engine (`sessionStart` fires on the next
//!   prompt again).
//! - A new chat whose project has the Git branch step starts its lookup at once, so the first
//!   message finds it ready (see [`branch_prefetch`]).
//! - The context of a prompt carries what the app looked up for its steps (see [`context`]): the
//!   memory notes and knowledge pointers, the branch against main and the limits that are close.
//!   They run next to the firings' commands and never hold the message back.
//! - A subagent the agent starts makes the app check the limits again and steer the running turn
//!   when one newly crossed a level (see [`limits`]).
//! - A new session arms `sessionStart` again, a switch, fork or clone disarms it, once the agent
//!   did it.
//! - Events are handled one after another by the chat's task: `toolEnd` commands run without the
//!   lock, a follow-up is composed, then sent under the lock while no user message went out since
//!   the run settled. `turnEnd` commands of a run that earned no follow-up and `sessionEnd`
//!   commands (once, when the agent exits) run with their output discarded.
//! - The sink counts the settles as they arrive, before they wait behind slow commands in the
//!   task's queue, and right before a message goes out (after its context commands ran) the send
//!   tells the engine that count: a message sent after a settle the task has not handled yet
//!   belongs to the next run (see `HookSession::settles_seen`).
//! - A chat moved to another project ([`moved`]) runs the hooks of its new project from then on.
//! - A command hook that gives no output (non-zero exit, timeout, could not start, no budget left)
//!   puts a short notice in the chat, without the command or its output.
//! - [`ChatHooks::is_idle`] tells a routine run whether the hooks still have work: an event the
//!   task has not handled, or a follow-up whose run has not settled.
//! - The memory-save hook follows the chat's memory guards, read at each settle, and every user
//!   message reads them too: an eligible prompt carries a "Memory settings" part (shown with the
//!   hooks' context) when they changed since the agent was told.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use harness_business::env::Env;
use harness_business::hooks::{
    ChatContext, EventOutcome, Firing, FollowUpPlan, HookSession, HookSet, ResolvedPart, SendOrigin, compose_follow_up,
    handled_without_run, project_identity, run_firing,
};
use harness_business::model::{AgentAction, AgentEvent, AgentReply, AgentSessionInfo, HarnessId};
use harness_business::process::TokioRunner;
use harness_business::{BusinessError, Result};
use tokio::sync::mpsc;

use self::counts::Counts;
use super::HookRuntime;
use crate::agents::Agents;
use crate::state::AppState;

mod branch_prefetch;
mod context;
mod counts;
mod limits;
mod limits_cache;
mod lookups;
mod memory;
mod report;
mod sink;
#[cfg(test)]
mod tests_lookups;
#[cfg(test)]
mod tests_prefetch;

pub use memory::check_again;
pub use sink::{HookEvents, HookSink, ModelProvider};

use branch_prefetch::{BranchPrefetch, wanted_at_start};
use context::with_context;
use limits::{LimitsWatch, Watch};
use lookups::BranchLookup;
use report::report;

/// The hook state of one tracked chat.
pub struct ChatHooks {
    key: String,
    harness: HarnessId,
    /// The chat's Harness Memory key (`HARNESS_MEMORY_SESSION`), when it has one.
    memory_key: Option<String>,
    project: Mutex<Option<String>>,
    counts: Arc<Counts>,
    /// What the chat was told about its plan limits.
    limits: LimitsWatch,
    /// The provider of the chat's current model (from its sink).
    provider: ModelProvider,
    /// The branch lookup started with the chat, for its first message.
    branch: BranchPrefetch,
    session: tokio::sync::Mutex<HookSession>,
}

impl ChatHooks {
    fn new(
        chat: ChatContext,
        harness: HarnessId,
        memory_key: Option<String>,
        counts: Arc<Counts>,
        provider: ModelProvider,
    ) -> Self {
        Self {
            key: chat.session_id.clone(),
            harness,
            memory_key,
            project: Mutex::new(chat.project.clone()),
            counts,
            limits: LimitsWatch::default(),
            provider,
            branch: BranchPrefetch::default(),
            session: tokio::sync::Mutex::new(HookSession::new(chat)),
        }
    }

    /// The harness the chat runs on.
    pub fn harness(&self) -> HarnessId {
        self.harness
    }

    /// The dashboard providers whose limits the chat sees: its current model's, else its harness's.
    pub fn limit_providers(&self) -> Vec<String> {
        let provider = self.provider.lock().unwrap_or_else(PoisonError::into_inner).clone();

        harness_business::ecosystem::limit_providers(self.harness, provider.as_deref())
    }

    /// The chat's session key.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The chat's Harness Memory key, when it has one.
    pub fn memory_key(&self) -> Option<&str> {
        self.memory_key.as_deref()
    }

    /// The chat's project root, when it has one.
    pub fn project(&self) -> Option<String> {
        self.project.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The settles the chat's sink has seen.
    fn settles_seen(&self) -> u64 {
        self.counts.settles()
    }

    /// True while the hooks have nothing left to do: the task handled every event the sink passed
    /// on, and the run of the last follow-up settled. A settle seen before this is true may still
    /// earn a follow-up, so a routine run waits for both.
    pub fn is_idle(&self) -> bool {
        self.counts.is_idle()
    }
}

/// Tracks the chat `info` started (`fresh`: not a resumed session; `memory_key`: its Harness
/// Memory key) and starts its hook task over `events`. A chat that stopped meanwhile is not
/// tracked.
pub async fn track(
    state: &AppState,
    info: &AgentSessionInfo,
    fresh: bool,
    memory_key: Option<String>,
    events: HookEvents,
) {
    let folder = PathBuf::from(&info.project);
    let project = project_root(&folder).await;
    let useful_skills = state.hooks.useful_skills_for(info.harness).await;

    let chat = Arc::new(ChatHooks::new(
        ChatContext {
            session_id: info.key.clone(),
            folder,
            project,
            fresh,
            utility: false,
            useful_skills,
        },
        info.harness,
        memory_key,
        events.counts.clone(),
        events.provider.clone(),
    ));

    if !register(state, &chat) {
        return;
    }

    limits_cache::warm(&state.dashboard, &state.env);
    prefetch_branch(state, &chat, fresh).await;

    let watch = Watch {
        runtime: state.hooks.clone(),
        agents: state.agents.clone(),
        env: state.env.clone(),
        dashboard: state.dashboard.clone(),
        chat: chat.clone(),
    };

    // Ends with the sink, which is gone once the chat's agent is.
    drop(tauri::async_runtime::spawn(watch.run(events.started)));

    let task = ChatTask {
        runtime: state.hooks.clone(),
        agents: state.agents.clone(),
        env: state.env.clone(),
        chat,
    };

    state
        .hooks
        .keep_task(tauri::async_runtime::spawn(task.run(events.received)));
}

/// Starts the Git branch lookup of a chat that starts (`fresh`) when its project has the step, so
/// the first message finds the note ready (see [`branch_prefetch`]).
async fn prefetch_branch(state: &AppState, chat: &ChatHooks, fresh: bool) {
    if !fresh {
        return;
    }

    let hooks = state.hooks.hook_set(chat.project().as_deref()).await;

    if !wanted_at_start(fresh, &hooks) {
        return;
    }

    let folder = chat.session.lock().await.chat().folder.clone();
    let lookup = BranchLookup::of(state).note(folder.clone());

    chat.branch.start(&folder, lookup);
}

/// Chat `key` moved to `folder` (`agent_move_project`): its hooks run there and are those of its
/// new project from now on, and its memory guards file names the repository there. Nothing
/// happens for a chat that is not tracked.
pub async fn moved(state: &AppState, key: &str, folder: PathBuf) {
    let Some(chat) = state.hooks.chat(key) else {
        return;
    };

    let project = project_root(&folder).await;
    chat.memory_moved(state, &folder).await;

    let mut session = chat.session.lock().await;
    session.move_to(folder, project.clone());
    *chat.project.lock().unwrap_or_else(PoisonError::into_inner) = project;
}

/// The project root of `folder` (`project_identity`, off the async runtime); `None` when it has
/// none.
async fn project_root(folder: &Path) -> Option<String> {
    let lookup = folder.to_path_buf();

    tauri::async_runtime::spawn_blocking(move || project_identity(&lookup).ok())
        .await
        .ok()
        .flatten()
        .and_then(|root| root.into_os_string().into_string().ok())
}

/// Tracks `chat` while its session runs. A chat whose channel closed meanwhile (its forget may
/// have run before this insert) is not kept: the session leaves the manager before it is
/// forgotten, so the check after the insert sees it.
fn register(state: &AppState, chat: &Arc<ChatHooks>) -> bool {
    let manager = state.agents.manager();
    let live: HashSet<String> = manager.list().into_iter().map(|info| info.key).collect();

    if !state.hooks.insert(chat.clone(), &live) {
        return false;
    }

    if manager.info(chat.key()).is_err() {
        state.hooks.forget(chat.key());
        return false;
    }

    true
}

/// Runs `action` in chat `key`; a tracked chat's messages go through its hooks first.
pub async fn send(state: &AppState, key: &str, action: AgentAction) -> Result<AgentReply> {
    let manager = state.agents.manager();

    let chat = match &action {
        AgentAction::Prompt { .. } | AgentAction::Steer { .. } | AgentAction::FollowUp { .. } => state.hooks.chat(key),
        other if changes_session(other) => state.hooks.chat(key),
        _ => None,
    };

    let Some(chat) = chat else {
        return manager.send(key, action).await;
    };

    // Held until the action is sent, so nothing overtakes it.
    let mut session = chat.session.lock().await;

    if changes_session(&action) {
        let reply = manager.send(key, action.clone()).await?;
        session.on_send(&HookSet::empty(), &action, SendOrigin::User);

        return Ok(reply);
    }

    let started = std::time::Instant::now();
    let hooks = state.hooks.hook_set(chat.project().as_deref()).await;
    let hook_set_ms = started.elapsed().as_millis();
    session.set_memory_guards(chat.memory_guards(&state.env).await);
    let guards_ms = started.elapsed().as_millis() - hook_set_ms;

    // Looked up for each message (cached): Harness Skills installed or removed since the chat
    // started apply on its next message.
    session.set_useful_skills(state.hooks.useful_skills_for(chat.harness()).await);
    let skills_ms = started.elapsed().as_millis() - hook_set_ms - guards_ms;

    let mut plan = session.on_send(&hooks, &action, SendOrigin::User);

    let action = if plan.is_empty() {
        action
    } else {
        with_context(state, &chat, action, &plan, &hooks, &mut session).await
    };

    tracing::debug!(
        session = %key,
        total_ms = started.elapsed().as_millis(),
        hook_set_ms,
        guards_ms,
        skills_ms,
        "message prepared before sending"
    );

    // Read after the context commands ran: a settle that arrived meanwhile ends the run before
    // this message.
    session.settles_seen(chat.settles_seen());
    session.sending(&mut plan);

    let sent = manager.send(key, action).await;

    if let Err(error) = &sent
        && never_delivered(error)
    {
        session.send_failed(&plan);
    }

    sent
}

/// A new, other, forked or cloned session.
fn changes_session(action: &AgentAction) -> bool {
    matches!(
        action,
        AgentAction::NewSession | AgentAction::SwitchSession { .. } | AgentAction::Fork { .. } | AgentAction::Clone
    )
}

/// True for a send error raised before the message was written to the agent: the action failed
/// validation, or the chat is gone. Any later error may follow a delivered message.
fn never_delivered(error: &BusinessError) -> bool {
    matches!(
        error,
        BusinessError::InvalidInput(_)
            | BusinessError::NotFound(_)
            | BusinessError::Forbidden(_)
            | BusinessError::Unsupported(_)
    )
}

/// Carries out what a chat's events ask for.
struct ChatTask {
    runtime: Arc<HookRuntime>,
    agents: Arc<Agents>,
    env: Arc<Env>,
    chat: Arc<ChatHooks>,
}

impl ChatTask {
    /// Handles the chat's events in order until its agent exited.
    async fn run(self, mut events: mpsc::UnboundedReceiver<AgentEvent>) {
        while let Some(event) = events.recv().await {
            let hooks = self.runtime.hook_set(self.chat.project().as_deref()).await;

            // The guards as they are when the run settles decide its memory-save hook.
            let guards = match event {
                AgentEvent::Settled { .. } => Some(self.chat.memory_guards(&self.env).await),
                _ => None,
            };

            let outcome = {
                let mut session = self.chat.session.lock().await;

                if let Some(guards) = guards {
                    session.set_memory_guards(guards);
                }

                session.on_event(&hooks, &event)
            };

            self.carry_out(outcome).await;
            self.chat.counts.handled();

            if matches!(event, AgentEvent::Exited { .. }) {
                self.runtime.forget_exited(&self.chat);
                break;
            }
        }
    }

    async fn carry_out(&self, outcome: EventOutcome) {
        match outcome {
            EventOutcome::Nothing => {}
            EventOutcome::ToolCommands(firing) => {
                let result = self.run_firing(&firing).await;
                let follow_up = self.chat.session.lock().await.on_tool_output(&firing, &result);

                if let Some(plan) = follow_up {
                    self.follow_up(plan).await;
                }
            }
            EventOutcome::FollowUp(plan) => self.follow_up(plan).await,
            EventOutcome::TurnCommands(firing) | EventOutcome::SessionEnd(firing) => {
                self.run_firing(&firing).await;
            }
        }
    }

    /// Runs `firing` and reports its failures; returns what its hooks contribute.
    async fn run_firing(&self, firing: &Firing) -> Vec<ResolvedPart> {
        let result = run_firing(&TokioRunner, &self.env, firing).await;
        report(&self.agents, self.chat.key(), &result.failures);

        result.parts
    }

    /// Composes the follow-up of a settled run and sends it, unless a user message went out
    /// meanwhile.
    async fn follow_up(&self, plan: FollowUpPlan) {
        let parts = self.run_firing(&plan.firing).await;

        let Some(message) = compose_follow_up(&parts) else {
            return;
        };

        let mut session = self.chat.session.lock().await;

        if !session.take_follow_up(plan.ticket) {
            tracing::debug!(session = %self.chat.key(), "hook follow-up dropped: the user sent a message");
            return;
        }

        let action = AgentAction::Prompt {
            message,
            images: Vec::new(),
            streaming_behavior: None,
        };
        session.on_send(&HookSet::empty(), &action, SendOrigin::Hook);

        // Still under the lock and before this task reads the next event, so a `Handled` result
        // for the follow-up is never taken for a user message's.
        let settles = self.chat.settles_seen();

        match self.agents.manager().send(self.chat.key(), action).await {
            Ok(reply) => {
                if !handled_without_run(&reply) {
                    self.chat.counts.follow_up_sent(settles);
                }

                session.follow_up_replied(&reply);
                tracing::info!(session = %self.chat.key(), parts = parts.len(), "hook follow-up sent");
            }
            Err(error) => tracing::warn!(session = %self.chat.key(), code = ?error.code(), "hook follow-up not sent"),
        }
    }
}
