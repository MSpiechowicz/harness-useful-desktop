//! The started chat of an agent run ([`Chat`]) and what the run sees of it ([`Watch`]): the
//! prompt, the session id, the settles, the assistant text, the questions, the end. See
//! [`super::agent_run`].

use std::sync::Arc;
use std::time::Duration;

use harness_business::model::{AgentAction, AgentReply, HarnessId, RunSession, UiOutcome, UiResponse};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{Instant, sleep, sleep_until};

use super::agent_run::{AgentTiming, Order, PROMPT_FAILED, StateHost};
use super::attention::Attention;
use super::job::{RunOutcome, StopReason, StopSignal};
use super::progress::RunProgress;
use super::relay::{RelaySink, Signal};
use crate::commands::agent::stop_soon;
use crate::hooks::chat::{self as chat_hooks, ChatHooks};
use crate::state::AppState;

/// How the watched chat ended.
pub(super) enum Ended {
    /// Settled, and the hooks are done.
    Done,
    /// The run was stopped.
    Stopped(StopReason),
    /// `maxMinutes` passed.
    TimeUp,
    /// A question waited unseen too long.
    Unanswered,
    /// The agent exited.
    Exited,
    /// The model or the prompt was refused.
    Refused(RunOutcome),
}

/// The started chat of a run.
pub(super) struct Chat<'a> {
    pub(super) state: &'a AppState,
    pub(super) key: String,
    pub(super) hooks: Option<Arc<ChatHooks>>,
    pub(super) relay: Arc<RelaySink>,
}

impl Chat<'_> {
    /// Switches to the routine's model, when it sets one.
    pub(super) async fn set_model(&self, order: &Order) -> Result<(), RunOutcome> {
        let Some(model) = &order.model else {
            return Ok(());
        };

        let action = AgentAction::SetModel {
            provider: model.provider.clone(),
            model_id: model.id.clone(),
        };

        match self.state.agents.manager().send(&self.key, action).await {
            Ok(_) => Ok(()),
            Err(error) => {
                tracing::info!(session = %self.key, code = ?error.code(), "the routine's model is not available");
                let named = format!("{}/{}", model.provider, model.id);

                Err(RunOutcome::failed(&format!("The model {named} is not available")))
            }
        }
    }

    /// Sends the prompt through the chat's hooks; the relay is told in event order when the agent
    /// takes it.
    async fn prompt(&self, order: &Order) -> harness_business::Result<AgentReply> {
        self.state
            .agents
            .manager()
            .on_prompt_taken(&self.key, self.relay.prompt_taken())?;

        let action = AgentAction::Prompt {
            message: order.prompt.clone(),
            images: Vec::new(),
            streaming_behavior: None,
        };

        chat_hooks::send(self.state, &self.key, action).await
    }

    /// True when the hooks have nothing left to do (a chat without hooks never has).
    fn hooks_idle(&self) -> bool {
        self.hooks.as_ref().is_none_or(|hooks| hooks.is_idle())
    }

    /// Answers every open request with "cancelled".
    pub(super) async fn cancel<'s>(&self, open: impl Iterator<Item = &'s String>) {
        for id in open {
            let response = UiResponse {
                id: id.clone(),
                outcome: UiOutcome::Cancelled,
            };

            // A request the agent withdrew meanwhile needs nothing.
            self.state.agents.manager().respond(&self.key, response).await.ok();
        }
    }

    /// Stops the agent, or leaves the chat to the window that shows it (`may_stay`) until the
    /// window leaves or the agent exits.
    pub(super) fn end(self, host: &Arc<dyn StateHost>, may_stay: bool, signals: UnboundedReceiver<Signal>) {
        if may_stay && self.relay.finish() {
            tracing::info!(
                run = self.relay.run_id(),
                "a window keeps the chat of an ended routine run"
            );
            drop(tauri::async_runtime::spawn(linger(
                host.clone(),
                self.relay,
                self.key,
                signals,
            )));
            return;
        }

        self.state.routines.relays().remove(self.relay.run_id());
        stop_soon(self.state, &self.key);
    }
}

/// Keeps the chat of an ended run until its window leaves (then stops the agent) or the agent
/// exits.
async fn linger(host: Arc<dyn StateHost>, relay: Arc<RelaySink>, key: String, mut signals: UnboundedReceiver<Signal>) {
    let state = host.app_state();

    while let Some(signal) = signals.recv().await {
        match signal {
            Signal::Attached(false) => stop_soon(state, &key),
            Signal::Exited => break,
            _ => {}
        }
    }

    state.routines.relays().remove(relay.run_id());
}

/// What the run saw of its chat.
pub(super) struct Watch {
    pub(super) progress: RunProgress,
    harness: HarnessId,
    pub(super) attention: Attention,
    idle_poll: Duration,
    /// The session id last recorded.
    session: Option<String>,
    /// The agent took the prompt (told in event order, see [`RelaySink::prompt_taken`]).
    taken: bool,
    /// The agent settled since it took the prompt.
    settled: bool,
    /// The harness handled the prompt itself, without a run: it never settles for it. Its result
    /// comes in event order right before [`Signal::PromptTaken`], so taking the prompt keeps it.
    handled: bool,
    /// The last prompt ended with an error.
    pub(super) failed: bool,
    /// The last assistant text.
    pub(super) said: Option<String>,
}

impl Watch {
    /// Nothing seen yet of the chat of `progress`'s run (`harness`).
    pub(super) fn new(progress: RunProgress, harness: HarnessId, timing: AgentTiming) -> Self {
        Self {
            progress,
            harness,
            attention: Attention::new(timing.ui_wait),
            idle_poll: timing.idle_poll,
            session: None,
            taken: false,
            settled: false,
            handled: false,
            failed: false,
            said: None,
        }
    }

    /// Sends the prompt and follows the chat until it ends.
    pub(super) async fn until_end(
        &mut self,
        chat: &Chat<'_>,
        order: &Order,
        signals: &mut UnboundedReceiver<Signal>,
        stop: &mut StopSignal,
        deadline: Instant,
    ) -> Ended {
        // The prompt's answer may wait for a question to be answered, so it is followed here.
        let prompt = chat.prompt(order);
        tokio::pin!(prompt);
        let mut prompted = false;

        loop {
            let unanswered = self.attention.deadline();

            tokio::select! {
                reason = stop.stopped() => return Ended::Stopped(reason),
                () = sleep_until(deadline) => return Ended::TimeUp,
                () = until(unanswered) => return Ended::Unanswered,
                sent = &mut prompt, if !prompted => {
                    prompted = true;

                    if sent.is_err() {
                        return Ended::Refused(RunOutcome::failed(PROMPT_FAILED));
                    }

                }
                signal = signals.recv() => match signal {
                    Some(signal) => {
                        if let Some(ended) = self.hear(chat, signal).await {
                            return ended;
                        }
                    }
                    None => return Ended::Exited,
                },
                () = sleep(self.idle_poll), if self.taken && (self.settled || self.handled) => {
                    if chat.hooks_idle() {
                        return Ended::Done;
                    }
                }
            }
        }
    }

    /// Takes in `signal`; how the chat ended when it did.
    async fn hear(&mut self, chat: &Chat<'_>, signal: Signal) -> Option<Ended> {
        match signal {
            Signal::Session(id) => self.record_session(id).await,
            Signal::Settled => self.settled = true,
            // What came before the prompt (a settle at startup) says nothing about its run.
            Signal::PromptTaken => {
                self.taken = true;
                self.settled = false;
                self.failed = false;
            }
            Signal::PromptDone { failed, handled } => {
                self.failed = failed;
                self.handled |= handled;
            }
            Signal::Said(text) => self.said = Some(text),
            Signal::Asked(id) => {
                self.attention.asked(id);
                self.update_waiting(chat);
            }
            Signal::Closed(id) => {
                self.attention.closed(&id);
                self.update_waiting(chat);
            }
            Signal::Attached(attached) => self.attention.attached(attached),
            Signal::Exited => return Some(Ended::Exited),
        }

        None
    }

    async fn record_session(&mut self, id: String) {
        if self.session.as_ref() == Some(&id) {
            return;
        }

        self.session = Some(id.clone());
        self.progress
            .session(RunSession {
                harness: self.harness,
                id,
            })
            .await;
    }

    fn update_waiting(&self, chat: &Chat<'_>) {
        let waiting = self.attention.waiting();

        chat.relay.set_waiting(waiting);
        self.progress.needs_you(waiting);
    }
}

/// Waits until `instant`; forever without one.
async fn until(instant: Option<Instant>) {
    match instant {
        Some(instant) => sleep_until(instant).await,
        None => std::future::pending().await,
    }
}
