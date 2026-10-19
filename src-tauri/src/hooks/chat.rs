//! One chat's hooks: its `HookSession`, fed by every send ([`send`]) and by the events its
//! [`HookSink`] passes on, and the task that carries out what the engine asks for.
//!
//! - A user message takes the chat's lock, asks the engine for context, runs the command hooks
//!   of the firing, puts the context in front of the prompt and sends it before it lets go, so a
//!   later message (or a hook follow-up) never overtakes it and other chats never wait. A message
//!   refused before it reached the agent is undone in the engine (`sessionStart` fires on the next
//!   prompt again).
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

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use harness_business::agent::EventSink;
use harness_business::env::Env;
use harness_business::hooks::{
    ChatContext, CommandFailure, CommandFailureKind, EventOutcome, Firing, FollowUpPlan, HookSession, HookSet,
    ResolvedPart, SendOrigin, SendPlan, compose_follow_up, compose_prompt, handled_without_run, project_identity,
    run_firing, run_firings,
};
use harness_business::model::{AgentAction, AgentEnvelope, AgentEvent, AgentReply, AgentSessionInfo, NoticeLevel};
use harness_business::process::TokioRunner;
use harness_business::{BusinessError, Result};
use serde_json::Value;
use tokio::sync::mpsc;

use self::counts::Counts;
use super::HookRuntime;
use crate::agents::Agents;
use crate::state::AppState;

mod counts;

/// Forwards a chat's events to its sink (the webview, or a routine run's relay) first, then hands
/// the ones the hooks react to (tool ends, prompt results, settles, the exit) to the chat's hook
/// task without waiting. Counts the settles before the sink sees them.
pub struct HookSink {
    inner: Arc<dyn EventSink>,
    events: mpsc::UnboundedSender<AgentEvent>,
    counts: Arc<Counts>,
}

/// The events a [`HookSink`] passed on, and its counts, for [`track`].
pub struct HookEvents {
    received: mpsc::UnboundedReceiver<AgentEvent>,
    counts: Arc<Counts>,
}

impl HookSink {
    /// A sink around `inner`, and the events it passes on.
    pub fn new(inner: Arc<dyn EventSink>) -> (Self, HookEvents) {
        let (events, received) = mpsc::unbounded_channel();
        let counts = Arc::new(Counts::default());

        let sink = Self {
            inner,
            events,
            counts: counts.clone(),
        };

        (sink, HookEvents { received, counts })
    }
}

impl EventSink for HookSink {
    fn emit(&self, envelope: AgentEnvelope) {
        let relevant = hook_relevant(&envelope.event);

        // Counted before anyone can react to it, so a message sent after it starts the next run.
        let settled = matches!(envelope.event, AgentEvent::Settled { .. });
        self.counts.arrived(settled, relevant.is_some());

        self.inner.emit(envelope);

        // A closed receiver means the chat is not tracked: nothing will handle it.
        if let Some(event) = relevant
            && self.events.send(event).is_err()
        {
            self.counts.handled();
        }
    }
}

/// The part of `event` the engine reads, when it reads it (a tool's result is left out).
fn hook_relevant(event: &AgentEvent) -> Option<AgentEvent> {
    match event {
        AgentEvent::ToolEnd {
            tool_call_id,
            name,
            is_error,
            touched_paths,
            ..
        } => Some(AgentEvent::ToolEnd {
            tool_call_id: tool_call_id.clone(),
            name: name.clone(),
            is_error: *is_error,
            result: Value::Null,
            touched_paths: touched_paths.clone(),
        }),
        AgentEvent::PromptResult { .. } | AgentEvent::Settled { .. } | AgentEvent::Exited { .. } => Some(event.clone()),
        _ => None,
    }
}

/// The hook state of one tracked chat.
pub struct ChatHooks {
    key: String,
    project: Mutex<Option<String>>,
    counts: Arc<Counts>,
    session: tokio::sync::Mutex<HookSession>,
}

impl ChatHooks {
    fn new(chat: ChatContext, counts: Arc<Counts>) -> Self {
        Self {
            key: chat.session_id.clone(),
            project: Mutex::new(chat.project.clone()),
            counts,
            session: tokio::sync::Mutex::new(HookSession::new(chat)),
        }
    }

    /// The chat's session key.
    pub fn key(&self) -> &str {
        &self.key
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

/// Tracks the chat `info` started (`fresh`: not a resumed session) and starts its hook task over
/// `events`. A chat that stopped meanwhile is not tracked.
pub async fn track(state: &AppState, info: &AgentSessionInfo, fresh: bool, events: HookEvents) {
    let folder = PathBuf::from(&info.project);
    let project = project_root(&folder).await;

    let chat = Arc::new(ChatHooks::new(
        ChatContext {
            session_id: info.key.clone(),
            folder,
            project,
            fresh,
            utility: false,
        },
        events.counts.clone(),
    ));

    if !register(state, &chat) {
        return;
    }

    let task = ChatTask {
        runtime: state.hooks.clone(),
        agents: state.agents.clone(),
        env: state.env.clone(),
        chat,
    };

    state.hooks.keep_task(tauri::async_runtime::spawn(task.run(events)));
}

/// Chat `key` moved to `folder` (`agent_move_project`): its hooks run there and are those of its
/// new project from now on. Nothing happens for a chat that is not tracked.
pub async fn moved(state: &AppState, key: &str, folder: PathBuf) {
    let Some(chat) = state.hooks.chat(key) else {
        return;
    };

    let project = project_root(&folder).await;

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

    let hooks = state.hooks.hook_set(chat.project().as_deref()).await;
    let mut plan = session.on_send(&hooks, &action, SendOrigin::User);

    let action = if plan.is_empty() {
        action
    } else {
        with_context(state, key, action, &plan, &mut session).await
    };

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

/// `action` (an eligible prompt) with the context of `plan` in front of its text. A
/// `sessionStart` context that did not fit fires again on the next prompt.
async fn with_context(
    state: &AppState,
    key: &str,
    action: AgentAction,
    plan: &SendPlan,
    session: &mut HookSession,
) -> AgentAction {
    let AgentAction::Prompt {
        message,
        images,
        streaming_behavior,
    } = action
    else {
        return action;
    };

    let result = run_firings(&TokioRunner, &state.env, &plan.firings).await;
    report(&state.agents, key, &result.failures);

    let (message, delivered) = compose_prompt(&message, &result.parts);
    session.context_composed(plan, &result.parts, &delivered);

    tracing::debug!(session = %key, parts = delivered.len(), "hook context added");

    AgentAction::Prompt {
        message,
        images,
        streaming_behavior,
    }
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

/// Puts a notice in chat `key` for each command hook that gave no output.
fn report(agents: &Agents, key: &str, failures: &[CommandFailure]) {
    for failure in failures {
        tracing::info!(session = %key, hook = failure.hook_id, kind = failure.kind.as_str(), "hook command gave no output");

        // A chat that is gone has nobody to tell.
        agents
            .manager()
            .notice(key, NoticeLevel::Warning, failure_notice(failure))
            .ok();
    }
}

/// The notice for `failure`: the hook's name and what went wrong, never its command or output.
fn failure_notice(failure: &CommandFailure) -> String {
    let name = &failure.hook_name;

    match failure.kind {
        CommandFailureKind::Spawn => format!("The hook \"{name}\" could not start its command."),
        CommandFailureKind::Exit(Some(code)) => format!("The hook \"{name}\" failed with exit code {code}."),
        CommandFailureKind::Exit(None) => format!("The hook \"{name}\" was stopped before it finished."),
        CommandFailureKind::Timeout => format!("The hook \"{name}\" timed out."),
        CommandFailureKind::Skipped => format!("The hook \"{name}\" was skipped, its event ran out of time."),
    }
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
    async fn run(self, mut events: HookEvents) {
        while let Some(event) = events.received.recv().await {
            let hooks = self.runtime.hook_set(self.chat.project().as_deref()).await;
            let outcome = self.chat.session.lock().await.on_event(&hooks, &event);

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
