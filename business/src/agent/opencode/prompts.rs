//! Prompts, steering, follow-ups and abort of an [`OpenCodeSession`].
//!
//! - **Idle:** a prompt (also a steer or follow-up sent once the turn is over) starts a turn
//!   ([`Request::next_turn`]). Follow-ups still held after an abort go first, the new message
//!   joins the queue behind them.
//! - **Working:** a steer goes to `prompt_async` at once, named by the app and with the turn's
//!   settings, and stays listed until its reply appears; it cannot be withdrawn. A follow-up (or
//!   a prompt without a streaming behaviour) is held and sent when the turn is over.
//! - Every message counts with the [`Mapper`](crate::opencode_server::Mapper) before it is sent,
//!   and a failed send takes it back. The prompt-taken call runs on the `204` or when the turn
//!   starts, whichever the chat sees first, always under the chat's lock, so it keeps its place
//!   among the events.
//! - **Abort:** `POST /session/{id}/abort`; the turn settles as aborted and held follow-ups stay
//!   held (OMP keeps its queue on an abort too).

use super::delivery::{Request, send};
use super::{OpenCodeSession, stream};
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentReply, ImageAttachment, StreamingBehavior};
use crate::opencode_server::wire::{Outgoing, prompt_body};
use crate::opencode_server::{MessageId, Route};

impl OpenCodeSession {
    /// Runs a prompt, queue, abort or history action.
    pub(super) async fn prompt_action(&self, action: AgentAction) -> Result<AgentReply> {
        match action {
            AgentAction::Prompt {
                message,
                images,
                streaming_behavior,
            } => self.prompt(Outgoing::new(&message, &images), streaming_behavior).await,
            AgentAction::Steer { message, images } => {
                self.busy_message(&message, &images, StreamingBehavior::Steer).await
            }
            AgentAction::FollowUp { message, images } => {
                self.busy_message(&message, &images, StreamingBehavior::FollowUp).await
            }
            AgentAction::RemoveQueued { message, queue } => self.remove_queued(&message, queue),
            AgentAction::Abort => self.abort().await,
            AgentAction::GetHistory { cursor, limit } => self.history(cursor.as_deref(), limit).await,
            _ => Err(BusinessError::Internal("not a prompt action".to_owned())),
        }
    }

    /// A prompt: starts a turn when idle, otherwise steers or is held as `behavior` says (held
    /// without one).
    async fn prompt(&self, message: Outgoing, behavior: Option<StreamingBehavior>) -> Result<AgentReply> {
        let working = self.chat.lock().mapper.is_working();

        if !working {
            return self.start_turn(message).await;
        }

        match behavior {
            Some(StreamingBehavior::Steer) => self.steer(message).await?,
            _ => self.hold(message),
        }

        Ok(prompt_reply("queued"))
    }

    /// A steer or follow-up: as `behavior` while working, a new turn otherwise.
    async fn busy_message(
        &self,
        text: &str,
        images: &[ImageAttachment],
        behavior: StreamingBehavior,
    ) -> Result<AgentReply> {
        let message = Outgoing::new(text, images);
        let working = self.chat.lock().mapper.is_working();

        match (working, behavior) {
            (false, _) => {
                self.start_turn(message).await?;
            }
            (true, StreamingBehavior::Steer) => self.steer(message).await?,
            (true, StreamingBehavior::FollowUp) => self.hold(message),
        }

        Ok(AgentReply::Ack)
    }

    /// Starts a turn with `message` (or with the held follow-ups, `message` behind them).
    async fn start_turn(&self, message: Outgoing) -> Result<AgentReply> {
        let chat = &self.chat;

        let (request, session) = {
            let mut live = chat.lock();

            if live.mapper.has_follow_ups() {
                let held = live.mapper.queue_follow_up(message);
                let next = live.mapper.next_follow_up();
                held.into_iter().for_each(|event| chat.shared.emit(event));

                if let Some((next, update)) = next {
                    update.into_iter().for_each(|event| chat.shared.emit(event));
                    send(chat, &mut live, next);
                }

                return Ok(prompt_reply("started"));
            }

            live.mapper.prompt_sent();

            if live.is_command(&message.text) {
                send(chat, &mut live, message);
                return Ok(prompt_reply("started"));
            }

            (Request::next_turn(&mut live, &message), live.session.clone())
        };

        let sent = request.post(chat, &session).await;
        let mut live = chat.lock();

        if let Err(error) = sent {
            live.mapper.prompt_failed();
            return Err(error);
        }

        chat.shared.prompt_taken_now();

        Ok(prompt_reply("started"))
    }

    /// Sends steering into the running turn.
    async fn steer(&self, mut message: Outgoing) -> Result<()> {
        let chat = &self.chat;
        let id = MessageId::ascending();
        message.id = Some(id.to_string());

        let (body, session) = {
            let mut live = chat.lock();
            let body = prompt_body(&message, &live.turn.settings());

            for event in live.mapper.steer_sent(message) {
                chat.shared.emit(event);
            }

            (body, live.session.clone())
        };

        let sent = Request::Prompt(body).post(chat, &session).await;
        let mut live = chat.lock();

        let mapped = match &sent {
            Ok(()) => live.mapper.steer_delivered(id.as_str()),
            Err(_) => live.mapper.steer_failed(id.as_str()),
        };
        stream::apply(chat, &mut live, mapped);

        sent
    }

    /// Holds a follow-up until the turn is over.
    fn hold(&self, message: Outgoing) {
        let mut live = self.chat.lock();

        for event in live.mapper.queue_follow_up(message) {
            self.chat.shared.emit(event);
        }
    }

    /// Withdraws a held follow-up; steering is already with the agent.
    fn remove_queued(&self, text: &str, queue: StreamingBehavior) -> Result<AgentReply> {
        if queue == StreamingBehavior::Steer {
            return Err(BusinessError::Unsupported(
                "OpenCode already has the steering message, it cannot be withdrawn".to_owned(),
            ));
        }

        let mut live = self.chat.lock();
        let removed = live.mapper.remove_follow_up(text);
        let done = removed.is_some();

        for event in removed.into_iter().flatten() {
            self.chat.shared.emit(event);
        }

        Ok(AgentReply::QueueRemoval { removed: done })
    }

    /// Stops the running turn; nothing to do while idle.
    async fn abort(&self) -> Result<AgentReply> {
        let session = {
            let mut live = self.chat.lock();

            if !live.mapper.is_working() {
                return Ok(AgentReply::Ack);
            }

            live.mapper.abort_requested();
            live.session.clone()
        };

        let empty = serde_json::json!({});
        self.chat
            .client
            .send(Route::Abort { session: &session }, Some(&empty))
            .await?
            .error_for_status()?;

        Ok(AgentReply::Ack)
    }
}

fn prompt_reply(disposition: &str) -> AgentReply {
    AgentReply::Prompt {
        agent_invoked: Some(true),
        disposition: Some(disposition.to_owned()),
    }
}
