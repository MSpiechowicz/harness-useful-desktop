//! Prompts, steering, follow-ups, withdrawing a queued message and abort of a
//! [`ClaudeSession`].
//!
//! - **Idle:** a prompt (also a steer or follow-up sent once the turn is over) is a user message
//!   that starts a turn.
//! - **Working:** a steer is a user message with `priority: next`, read in the running turn once
//!   its tool calls finish. A follow-up (or a prompt without a streaming behaviour) has `priority:
//!   later` and runs as its own turn after the running one. Both wait in the CLI's queue, which the
//!   chat shows until the CLI takes them (`queueUpdate`), and either can be withdrawn there
//!   (`cancel_async_message`) until then.
//! - Every message gets a fresh `uuid` and counts with the mapper (`sent`) before it is written; a
//!   message whose write fails is taken back. A prompt's taking (the CLI queued or started it)
//!   runs the prompt-taken call in event order.
//! - **Abort:** `interrupt` (the queue is kept: a waiting follow-up runs next). An ask still open
//!   once the CLI answered, failed or timed out is closed and denied, so a stopped turn never
//!   waits for an answer. A failed interrupt is reported after that. A running `!command` is
//!   killed.
//!
//! [`ClaudeSession`]: super::ClaudeSession

use super::io::send_kind;
use super::{ClaudeSession, Queued};
use crate::claude_stream::wire::control::CancelReply;
use crate::claude_stream::wire::outbound::{
    HostRequest, ImagePart, Priority, UserMessage, new_message_uuid, permission_deny, user_message,
};
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply, ImageAttachment, StreamingBehavior};
use crate::pi_family::rpc::pending::{ABORT_TIMEOUT, DEFAULT_TIMEOUT};

/// What a permission prompt or question still open at an abort is answered with.
const STOPPED: &str = "The user stopped the turn.";

impl ClaudeSession {
    /// Runs a prompt, queue or abort action.
    pub(super) async fn prompt_action(&self, action: AgentAction) -> Result<AgentReply> {
        match action {
            AgentAction::Prompt {
                message,
                images,
                streaming_behavior,
            } => {
                let busy = streaming_behavior.unwrap_or(StreamingBehavior::FollowUp);
                let queued = self.deliver(message, &images, busy, true).await?;
                let disposition = if queued { "queued" } else { "started" };

                Ok(AgentReply::Prompt {
                    agent_invoked: Some(true),
                    disposition: Some(disposition.to_owned()),
                })
            }
            AgentAction::Steer { message, images } => {
                self.deliver(message, &images, StreamingBehavior::Steer, false).await?;
                Ok(AgentReply::Ack)
            }
            AgentAction::FollowUp { message, images } => {
                self.deliver(message, &images, StreamingBehavior::FollowUp, false)
                    .await?;
                Ok(AgentReply::Ack)
            }
            AgentAction::RemoveQueued { message, queue } => self.remove_queued(&message, queue).await,
            AgentAction::Abort => self.abort().await,
            _ => Err(BusinessError::Internal("not a prompt action".to_owned())),
        }
    }

    /// Sends `text` with `images`: a prompt while idle, else into the CLI's queue as `busy` says.
    /// `prompt` marks a prompt action, whose taking runs the prompt-taken call. True when the
    /// message was queued.
    async fn deliver(
        &self,
        text: String,
        images: &[ImageAttachment],
        busy: StreamingBehavior,
        prompt: bool,
    ) -> Result<bool> {
        let run = self.run();
        let chat = &run.chat;
        let uuid = new_message_uuid();

        let message = {
            let mut live = chat.lock();

            if live.retired {
                return Err(moved());
            }

            let queue = live.mapper.is_working().then_some(busy);

            live.mapper.sent(&uuid, send_kind(queue));
            live.counts.user += 1;
            live.counts.total += 1;

            if prompt {
                live.awaiting_take.push(uuid.clone());
            }

            if let Some(queue) = queue {
                live.queue.push(Queued {
                    uuid: uuid.clone(),
                    text: text.clone(),
                    queue,
                });
                chat.shared.emit(live.queue_update());
            }

            UserMessage {
                uuid: uuid.clone(),
                text,
                images: images.iter().map(image_part).collect(),
                priority: queue.map(priority),
                should_query: true,
            }
        };

        if let Err(error) = chat.write(&user_message(&message)).await {
            chat.unsent(&uuid);
            return Err(error);
        }

        Ok(message.priority.is_some())
    }

    /// Withdraws the first message in `queue` whose text is `text`, unless the CLI took it.
    async fn remove_queued(&self, text: &str, queue: StreamingBehavior) -> Result<AgentReply> {
        let run = self.run();
        let chat = &run.chat;
        let uuid = {
            let live = chat.lock();
            let found = live
                .queue
                .iter()
                .find(|queued| queued.queue == queue && queued.text == text);

            match found {
                Some(queued) => queued.uuid.clone(),
                None => return Ok(AgentReply::QueueRemoval { removed: false }),
            }
        };

        let request = HostRequest::CancelAsyncMessage {
            message_uuid: uuid.clone(),
        };
        let reply = chat.control(&request, DEFAULT_TIMEOUT).await?;
        let removed = serde_json::from_value::<CancelReply>(reply).is_ok_and(|reply| reply.cancelled);

        if removed {
            let mut live = chat.lock();
            chat.dequeue(&mut live, std::slice::from_ref(&uuid));

            let mapped = live.mapper.withdrawn(&uuid);
            chat.apply(&mut live, mapped);
        }

        Ok(AgentReply::QueueRemoval { removed })
    }

    /// Interrupts the running turn, then denies what it still asks; nothing to do while idle.
    async fn abort(&self) -> Result<AgentReply> {
        let run = self.run();
        let chat = &run.chat;

        {
            let mut live = chat.lock();

            if let Some(shell) = live.shell.take() {
                shell.stop();
            }

            if !live.mapper.is_working() && live.dialogs.is_empty() {
                return Ok(AgentReply::Ack);
            }

            live.mapper.abort_requested();
        }

        let interrupted = chat
            .control(&HostRequest::Interrupt { cancel_queued: false }, ABORT_TIMEOUT)
            .await;

        // Whatever the interrupt's answer, no ask stays open: a failed write skips no other deny.
        let open = chat.lock().dialogs.drain();
        let mut denied = Ok(());

        for (dialog, request_id) in &open {
            chat.shared.emit(AgentEvent::UiCancel {
                request_id: dialog.clone(),
            });

            if let Err(error) = chat.write(&permission_deny(request_id, STOPPED)).await {
                denied = Err(error);
            }
        }

        interrupted?;
        denied?;

        Ok(AgentReply::Ack)
    }
}

/// Why a message reached a process the chat has left (it moved to another session meanwhile).
fn moved() -> BusinessError {
    BusinessError::Unsupported("the chat moved to another session. Send the message again".to_owned())
}

fn priority(queue: StreamingBehavior) -> Priority {
    match queue {
        StreamingBehavior::Steer => Priority::Next,
        StreamingBehavior::FollowUp => Priority::Later,
    }
}

fn image_part(image: &ImageAttachment) -> ImagePart {
    ImagePart {
        media_type: image.mime_type.clone(),
        data: image.data.clone(),
    }
}
