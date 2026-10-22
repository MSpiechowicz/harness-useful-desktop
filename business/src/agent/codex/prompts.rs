//! Prompts, steering, follow-ups, withdrawing a queued message and abort of a
//! [`CodexSession`](super::CodexSession).
//!
//! - **Idle:** a prompt (also a steer or follow-up sent once the turn is over) starts a turn
//!   (`turn/start` with the text and the images as `localImage` files, the chat's model and effort
//!   when it picked them, and `summary: auto` so reasoning text streams). Follow-ups still held
//!   after an abort go first, the new message joins the queue behind them.
//! - **Working:** a steer goes into the running turn at once (`turn/steer{expectedTurnId}`) and
//!   stays listed until Codex takes it or the turn ends. When Codex has no active turn any more
//!   (`-32600`), it is sent as a follow-up instead. A follow-up (or a prompt without a streaming
//!   behaviour) is held by the chat, not by Codex's queue, so it can be withdrawn, and starts its
//!   own turn once the running one ended (plan decision 4). While a compaction runs, steering is
//!   held like a follow-up.
//! - Every message gets a fresh `clientUserMessageId` and every turn counts with the mapper
//!   (`sent`) before it is written; one Codex does not take is taken back (`withdrawn`), and so is
//!   the message from the chat's counts. The prompt-taken call runs when Codex answered a prompt
//!   action's `turn/start` (`io.rs`).
//! - **Abort:** the open asks are cancelled (their dialogs close), then `turn/interrupt` stops the
//!   running turn; one whose id is not known yet is interrupted once it is, unless Codex never
//!   starts it (the stop then ends with it). Held follow-ups stay held.

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::oneshot;

use super::dialogs::Outcome;
use super::io::{Answer, refused};
use super::{Chat, CodexSession, Live, Queued};
use crate::codex_rpc::StartKind;
use crate::codex_rpc::wire::INVALID_REQUEST;
use crate::codex_rpc::wire::outbound::{Request, TurnStart, TurnSteer, next_request_id};
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply, ImageAttachment, NoticeLevel, StreamingBehavior};
use crate::pi_family::rpc::RpcResponse;
use crate::pi_family::rpc::pending::{ABORT_TIMEOUT, DEFAULT_TIMEOUT, PROMPT_ACK_TIMEOUT};

/// The reasoning summary every turn asks for, so reasoning shows as thinking (Verified fact 10).
const SUMMARY: &str = "auto";

/// A request written under the chat's lock and awaited after it.
pub(super) struct Call {
    key: String,
    frame: Value,
    receiver: oneshot::Receiver<Result<RpcResponse>>,
}

/// What placing a message led to.
enum Placed {
    /// It waits in the chat's queue.
    Queued,
    /// A turn starts (with it, or with an older follow-up while it waits).
    Turn { call: Call, queued: bool },
    /// It steers the running turn.
    Steer { call: Call, message: Queued },
}

impl CodexSession {
    /// Runs a prompt, queue or abort action.
    pub(super) async fn prompt_action(&self, action: AgentAction) -> Result<AgentReply> {
        let chat = &self.chat;

        match action {
            AgentAction::Prompt {
                message,
                images,
                streaming_behavior,
            } => {
                let busy = streaming_behavior.unwrap_or(StreamingBehavior::FollowUp);
                let disposition = match chat.deliver(message, &images, busy, true).await? {
                    true => "queued",
                    false => "started",
                };

                Ok(AgentReply::Prompt {
                    agent_invoked: Some(true),
                    disposition: Some(disposition.to_owned()),
                })
            }
            AgentAction::Steer { message, images } => {
                let steered = chat.deliver(message, &images, StreamingBehavior::Steer, false);
                steered.await.map(|_| AgentReply::Ack)
            }
            AgentAction::FollowUp { message, images } => {
                let queued = chat.deliver(message, &images, StreamingBehavior::FollowUp, false);
                queued.await.map(|_| AgentReply::Ack)
            }
            AgentAction::RemoveQueued { message, queue } => Ok(chat.remove_queued(&message, queue)),
            AgentAction::Abort => chat.abort().await,
            _ => Err(BusinessError::Internal("not a prompt action".to_owned())),
        }
    }
}

impl Chat {
    /// Sends `text` with `images`: a turn while idle, else steering or a follow-up as `busy` says.
    /// `prompt` marks a prompt action, whose taking runs the prompt-taken call. True when the
    /// message waits (queued or steering).
    async fn deliver(
        self: &Arc<Self>,
        text: String,
        images: &[ImageAttachment],
        busy: StreamingBehavior,
        prompt: bool,
    ) -> Result<bool> {
        let input = self.input(&text, images).await?;
        let message = Queued {
            client_id: uuid::Uuid::new_v4().to_string(),
            text,
            input,
            queue: busy,
            prompt,
        };

        let placed = {
            let mut live = self.lock();
            live.counts.user += 1;
            live.counts.total += 1;

            let placed = self.place(&mut live, message);
            if placed.is_err() {
                uncount(&mut live);
            }

            placed?
        };

        match placed {
            Placed::Queued => Ok(true),
            Placed::Turn { call, queued } => {
                self.send_turn(call).await?;
                Ok(queued)
            }
            Placed::Steer { call, message } => self.steer(call, message).await,
        }
    }

    /// Where `message` goes now (see the module docs).
    fn place(self: &Arc<Self>, live: &mut Live, mut message: Queued) -> Result<Placed> {
        if !live.mapper.is_working() {
            if !live.has_follow_ups() {
                let call = self.begin_turn(live, message)?;
                return Ok(Placed::Turn { call, queued: false });
            }

            message.queue = StreamingBehavior::FollowUp;
            live.queue.push(message);

            let held = live
                .queue
                .iter()
                .position(|queued| queued.queue == StreamingBehavior::FollowUp);
            let first = live.queue.remove(held.unwrap_or_default());
            let call = self.begin_turn(live, first);
            self.queue_changed(live);

            return Ok(Placed::Turn {
                call: call?,
                queued: true,
            });
        }

        let turn = live.running_turn().filter(|_| !live.mapper.is_compacting());

        if let (StreamingBehavior::Steer, Some(turn), Some(thread_id)) = (message.queue, turn, live.thread_id.clone()) {
            let request = Request::TurnSteer(TurnSteer {
                thread_id,
                expected_turn_id: turn,
                client_user_message_id: Some(message.client_id.clone()),
                input: message.input.clone(),
            });
            let call = self.prepare(&request)?;

            live.queue.push(message.clone_entry());
            self.shared.emit(live.queue_update());

            return Ok(Placed::Steer { call, message });
        }

        message.queue = StreamingBehavior::FollowUp;
        live.queue.push(message);
        self.queue_changed(live);

        Ok(Placed::Queued)
    }

    /// Steers the running turn with `message`; when Codex has no active turn any more, it is sent
    /// as a follow-up. True: it waits (steering or queued).
    async fn steer(self: &Arc<Self>, call: Call, mut message: Queued) -> Result<bool> {
        let error = match self.await_call(call, "turn/steer", DEFAULT_TIMEOUT).await {
            Ok(Ok(_)) => return Ok(true),
            Ok(Err(error)) if error.code == INVALID_REQUEST => None,
            Ok(Err(error)) => Some(refused("turn/steer", &error)),
            Err(error) => Some(error),
        };

        self.unlist(&message.client_id);

        if let Some(error) = error {
            uncount(&mut self.lock());
            return Err(error);
        }

        message.queue = StreamingBehavior::FollowUp;
        let placed = self.place(&mut self.lock(), message)?;

        match placed {
            Placed::Turn { call, queued } => {
                self.send_turn(call).await?;
                Ok(queued)
            }
            Placed::Queued | Placed::Steer { .. } => Ok(true),
        }
    }

    /// Takes steering `client_id` off the list.
    fn unlist(&self, client_id: &str) {
        let mut live = self.lock();
        let before = live.queue.len();
        live.queue.retain(|queued| queued.client_id != client_id);

        if live.queue.len() < before {
            self.shared.emit(live.queue_update());
        }
    }

    /// Withdraws the first follow-up whose text is `text`; steering already went to Codex.
    fn remove_queued(self: &Arc<Self>, text: &str, queue: StreamingBehavior) -> AgentReply {
        if queue == StreamingBehavior::Steer {
            return AgentReply::QueueRemoval { removed: false };
        }

        let mut live = self.lock();
        let found = live
            .queue
            .iter()
            .position(|queued| queued.queue == queue && queued.text == text);

        let Some(position) = found else {
            return AgentReply::QueueRemoval { removed: false };
        };

        live.queue.remove(position);
        uncount(&mut live);
        self.queue_changed(&mut live);

        AgentReply::QueueRemoval { removed: true }
    }

    /// Shows the queue and tells the mapper whether follow-ups wait (which may settle the chat:
    /// events only, nothing is owed to Codex).
    fn queue_changed(self: &Arc<Self>, live: &mut Live) {
        self.shared.emit(live.queue_update());

        let mapped = live.mapper.follow_ups_waiting(live.has_follow_ups());
        self.apply(live, mapped);
    }

    /// After a turn ended: the oldest follow-up starts the next turn, in the background.
    pub(super) fn next_follow_up(self: &Arc<Self>, live: &mut Live) {
        if live.mapper.is_working() {
            return;
        }

        let Some(position) = live
            .queue
            .iter()
            .position(|queued| queued.queue == StreamingBehavior::FollowUp)
        else {
            return;
        };

        let message = live.queue.remove(position);

        match self.begin_turn(live, message) {
            Ok(call) => {
                let chat = self.clone();
                tokio::spawn(async move { chat.dispatch(call).await });
            }
            Err(error) => tracing::debug!(%error, "a Codex follow-up could not be sent"),
        }

        self.queue_changed(live);
    }

    /// Sends a follow-up's turn; one Codex does not take is reported and the next one goes.
    async fn dispatch(self: Arc<Self>, call: Call) {
        let Err(error) = self.send_turn(call).await else {
            return;
        };

        self.shared.emit(AgentEvent::Notice {
            level: NoticeLevel::Error,
            message: format!("Codex did not take a queued message: {error}"),
        });

        self.next_follow_up(&mut self.lock());
    }

    /// Counts a turn with the mapper and prepares its `turn/start` for `message`.
    fn begin_turn(&self, live: &mut Live, message: Queued) -> Result<Call> {
        let thread_id = live
            .thread_id
            .clone()
            .ok_or_else(|| BusinessError::Internal("a Codex chat without a thread".to_owned()))?;

        let request = Request::TurnStart(Box::new(TurnStart {
            thread_id,
            client_user_message_id: Some(message.client_id),
            input: message.input,
            model: live.model.clone(),
            effort: live.effort.clone(),
            summary: Some(SUMMARY.to_owned()),
            ..TurnStart::default()
        }));
        let call = self.prepare(&request)?;

        live.mapper.sent(StartKind::Prompt);
        live.turn_requests.push((call.key.clone(), message.prompt));

        Ok(call)
    }

    /// Registers `request` under a fresh id; written by [`Chat::await_call`].
    fn prepare(&self, request: &Request) -> Result<Call> {
        let id = next_request_id();
        let (key, receiver) = self.register(&id)?;

        Ok(Call {
            key,
            frame: request.frame(&id),
            receiver,
        })
    }

    async fn await_call(&self, call: Call, method: &str, limit: std::time::Duration) -> Result<Answer> {
        self.finish_call(&call.key, &call.frame, method, call.receiver, limit)
            .await
    }

    /// Writes a turn's `turn/start` and waits until Codex took it; a turn it did not take is
    /// taken back from the mapper.
    async fn send_turn(self: &Arc<Self>, call: Call) -> Result<()> {
        let key = call.key.clone();
        let answer = self.await_call(call, "turn/start", PROMPT_ACK_TIMEOUT).await;

        let error = match answer {
            Ok(Ok(_)) => return Ok(()),
            Ok(Err(error)) => refused("turn/start", &error),
            Err(error) => error,
        };

        let owed = {
            let mut live = self.lock();
            live.turn_requests.retain(|(request, _)| *request != key);
            uncount(&mut live);

            let mapped = live.mapper.withdrawn();
            live.forget_abort();
            self.apply(&mut live, mapped)
        };
        self.write_later(owed);

        Err(error)
    }

    /// Cancels the open asks, then interrupts the running turn; nothing to do while idle.
    async fn abort(self: &Arc<Self>) -> Result<AgentReply> {
        let (target, cancelled) = {
            let mut live = self.lock();

            if !live.mapper.is_working() && live.dialogs.is_empty() {
                return Ok(AgentReply::Ack);
            }

            live.mapper.abort_requested();

            let turn = live.running_turn();
            live.abort_pending = turn.is_none() && live.mapper.is_working();

            let mut out = Outcome::default();
            live.dialogs.cancel_all(&mut out);
            out.events.into_iter().for_each(|event| self.shared.emit(event));

            (turn.zip(live.thread_id.clone()), out.answers)
        };

        let mut written = Ok(());

        for answer in &cancelled {
            if let Err(error) = self.write(answer).await {
                written = Err(error);
            }
        }

        if let Some((turn_id, thread_id)) = target {
            let request = Request::TurnInterrupt { thread_id, turn_id };

            // A refusal of a turn that already ended is no failure.
            if let Err(error) = self.call(&request, ABORT_TIMEOUT).await?
                && self.lock().mapper.turn_id().is_some()
            {
                return Err(refused(request.method(), &error));
            }
        }

        written?;

        Ok(AgentReply::Ack)
    }

    /// Writes `owed` frames from a task of its own (the caller may hold no lock).
    fn write_later(self: &Arc<Self>, owed: Vec<Value>) {
        if owed.is_empty() {
            return;
        }

        let chat = self.clone();
        tokio::spawn(async move {
            for frame in owed {
                if chat.write(&frame).await.is_err() {
                    break;
                }
            }
        });
    }
}

/// Takes a message Codex did not take back from the chat's counts.
fn uncount(live: &mut Live) {
    live.counts.user = live.counts.user.saturating_sub(1);
    live.counts.total = live.counts.total.saturating_sub(1);
}

impl Live {
    /// A stop of a turn that never began ends once nothing is starting any more, so the next
    /// turn is not interrupted for it.
    pub(super) fn forget_abort(&mut self) {
        if !self.mapper.is_working() {
            self.abort_pending = false;
        }
    }
}

impl Queued {
    /// The queue entry of steering (its input stays with the request).
    fn clone_entry(&self) -> Self {
        Self {
            client_id: self.client_id.clone(),
            text: self.text.clone(),
            input: Vec::new(),
            queue: self.queue,
            prompt: self.prompt,
        }
    }
}
