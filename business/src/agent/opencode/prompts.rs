//! Prompts of an [`OpenCodeSession`]: `POST /session/{id}/prompt_async` with the chat's agent,
//! model and variant, while the chat is idle.
//!
//! The prompt is announced to the [`Mapper`](crate::opencode_server::Mapper) before it is sent,
//! so the turn it starts answers it with a `promptResult`; a failed send takes it back. The
//! prompt-taken call runs on the `204` or when the turn starts, whichever the chat sees first, and
//! always under the chat's lock, so it keeps its place among the events. Steering, follow-ups,
//! aborts, slash commands and history are not available yet.

use super::OpenCodeSession;
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentReply, ImageAttachment};
use crate::opencode_server::Route;
use crate::opencode_server::wire::prompt_body;

impl OpenCodeSession {
    /// Runs a prompt or queue action.
    pub(super) async fn prompt_action(&self, action: AgentAction) -> Result<AgentReply> {
        match action {
            AgentAction::Prompt { message, images, .. } => self.prompt(&message, &images).await,
            AgentAction::Steer { .. } | AgentAction::FollowUp { .. } | AgentAction::RemoveQueued { .. } => {
                Err(not_yet("Steering and follow-up messages"))
            }
            AgentAction::Abort => Err(not_yet("Stopping a reply")),
            _ => Err(not_yet("Loading earlier messages")),
        }
    }

    async fn prompt(&self, message: &str, images: &[ImageAttachment]) -> Result<AgentReply> {
        let chat = &self.chat;

        let (session, body) = {
            let mut live = chat.lock();

            if live.mapper.is_working() {
                return Err(BusinessError::Unsupported(
                    "the agent is working; send the message once it is done".to_owned(),
                ));
            }

            if slash_command(message).is_some_and(|name| live.commands.iter().any(|known| known == name)) {
                return Err(not_yet("Slash commands"));
            }

            let body = prompt_body(
                message,
                images,
                &live.agent,
                live.model.as_ref(),
                live.variant.as_deref(),
            );
            live.mapper.prompt_sent();

            (live.session.clone(), body)
        };

        let sent = chat
            .client
            .send(Route::PromptAsync { session: &session }, Some(&body))
            .await;
        let sent = sent.and_then(|response| response.error_for_status());

        let mut live = chat.lock();

        if let Err(error) = sent {
            live.mapper.prompt_failed();
            return Err(error.into());
        }

        chat.shared.prompt_taken_now();

        Ok(AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("started".to_owned()),
        })
    }
}

/// The command name of a `/name args` message.
fn slash_command(message: &str) -> Option<&str> {
    let rest = message.strip_prefix('/')?;

    rest.split_whitespace().next()
}

fn not_yet(what: &str) -> BusinessError {
    BusinessError::Unsupported(format!("{what} in OpenCode chats are not available yet"))
}
