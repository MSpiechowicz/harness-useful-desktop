//! Sending messages to `opencode serve` for an [`OpenCodeSession`](super::OpenCodeSession).
//!
//! - A message that starts a turn goes with the chat's agent, model (once the user picked one)
//!   and variant, remembered as the turn's settings. `/name args` for one of the server's commands
//!   goes to `POST /session/{id}/command`, which answers only when the turn is over, so it is sent
//!   in the background; any other text to `prompt_async`.
//! - Queued follow-ups the mapper releases ([`Mapped::Send`](crate::opencode_server::Mapped)) are
//!   sent in the background; one that fails is reported by the mapper.
//! - Stranded steering ([`Mapped::CheckStranded`](crate::opencode_server::Mapped)): when the
//!   session's newest message is unanswered steering, it is deleted and sent again under the same
//!   id, so the chat shows it once and OpenCode runs a turn for it.

use std::sync::Arc;

use serde_json::Value;

use super::startup::get_json;
use super::{Chat, Live, stream};
use crate::error::{BusinessError, Result};
use crate::opencode_server::wire::{Outgoing, command_body, newest_message, prompt_body, slash_command};
use crate::opencode_server::{MessageId, Route, SessionId};

/// How a message is sent.
pub(super) enum Request {
    /// `prompt_async` with this body.
    Prompt(Value),
    /// `command` with this body (answers when the turn is over).
    Command(Value),
}

impl Request {
    /// Prepares `message` as the next turn's: a server command or a prompt, with the chat's
    /// settings (remembered as the turn's). The caller counts it as a prompt on its way.
    pub(super) fn next_turn(live: &mut Live, message: &Outgoing) -> Self {
        let command = live
            .is_command(&message.text)
            .then(|| slash_command(&message.text))
            .flatten();
        let settings = live.next_turn().settings();

        match command {
            Some((name, arguments)) => Self::Command(command_body(name, arguments, message, &settings)),
            None => Self::Prompt(prompt_body(message, &settings)),
        }
    }

    /// Sends the request; `Ok` for a `2xx` answer.
    pub(super) async fn post(&self, chat: &Chat, session: &SessionId) -> Result<()> {
        let sent = match self {
            Self::Prompt(body) => chat.client.send(Route::PromptAsync { session }, Some(body)).await,
            Self::Command(body) => chat.client.send(Route::Command { session }, Some(body)).await,
        };

        sent?.error_for_status()?;
        Ok(())
    }

    /// True for a server command.
    pub(super) fn is_command(&self) -> bool {
        matches!(self, Self::Command(_))
    }
}

/// Sends `message`, already counted as a prompt on its way, in the background. A prompt's `204`
/// runs the prompt-taken call (a command's answer comes after its turn, whose start runs it); a
/// failure goes to the mapper.
pub(super) fn send(chat: &Arc<Chat>, live: &mut Live, message: Outgoing) {
    let request = Request::next_turn(live, &message);
    let session = live.session.clone();
    let chat = chat.clone();

    tokio::spawn(async move {
        let sent = match chat.rules_for_turn(&session).await {
            Ok(()) => request.post(&chat, &session).await,
            Err(error) => Err(error),
        };
        let mut live = chat.lock();

        match sent {
            Ok(()) if !request.is_command() => chat.shared.prompt_taken_now(),
            Ok(()) => {}
            Err(error) => {
                let mapped = live.mapper.prompt_failed_late(&error.to_string());
                stream::apply(&chat, &mut live, mapped);
            }
        }
    });
}

/// Checks in the background whether the newest of `steering` was left without a reply, and sends
/// it again or lets the turn end.
pub(super) fn check_stranded(chat: &Arc<Chat>, steering: Vec<Outgoing>) {
    let chat = chat.clone();

    tokio::spawn(async move {
        let session = chat.lock().session.clone();
        let stranded = find_stranded(&chat, &session, steering).await;

        let resend = {
            let mut live = chat.lock();
            let resend = stranded.filter(|message| {
                let id = message.id.as_deref().unwrap_or_default();
                live.mapper.resend_stranded(id)
            });

            match resend {
                Some(message) => {
                    let body = prompt_body(&message, &live.turn.settings());
                    Some((message, body))
                }
                None => {
                    let mapped = live.mapper.stranded_checked();
                    stream::apply(&chat, &mut live, mapped);
                    None
                }
            }
        };

        let Some((message, body)) = resend else {
            return;
        };

        if resend_stranded(&chat, &session, &message, body).await.is_err() {
            let mut live = chat.lock();
            let mapped = live.mapper.resend_failed();
            stream::apply(&chat, &mut live, mapped);
        }
    });
}

/// The steering message that is the session's newest message, if it is one.
async fn find_stranded(chat: &Chat, session: &SessionId, steering: Vec<Outgoing>) -> Option<Outgoing> {
    let route = Route::Messages {
        session,
        limit: Some(2),
        before: None,
    };
    let page = get_json(&chat.client, route).await.ok()?;
    let (role, id) = newest_message(&page)?;

    if role != "user" {
        return None;
    }

    steering
        .into_iter()
        .find(|message| message.id.as_deref() == Some(id.as_str()))
}

/// Deletes the stranded message and sends it again under its id.
async fn resend_stranded(chat: &Chat, session: &SessionId, message: &Outgoing, body: Value) -> Result<()> {
    let id = message.id.as_deref().unwrap_or_default();
    let id = MessageId::parse(id).map_err(|_| BusinessError::Internal("steering without an id".to_owned()))?;

    chat.client
        .send(Route::MessageDelete { session, message: &id }, None)
        .await?
        .error_for_status()?;

    Request::Prompt(body).post(chat, session).await
}
