//! Sending messages to OpenCode 2 for an [`OpenCodeV2Session`](super::OpenCodeV2Session).
//!
//! - A message that starts a turn goes to `POST /api/session/{id}/prompt` (`{text, files}`; images
//!   as `data:` URLs, which the server inlines like a file), or, as `/name args` for one of the
//!   server's commands, to `POST …/command` (`{name, text}`). `/name args` for one of its skills is
//!   a prompt of `args` with the skill attached (`skills: [{id}]`). Each answers once OpenCode
//!   admitted the input; the session's model, variant and agent apply (`settings.rs`).
//! - Commands and skills are sent in the background, as on OpenCode 1: one OpenCode refuses counts
//!   as taken, then fails with an error result in the chat.
//! - Steering goes to `prompt` with `delivery: steer` and an id the app chose, so the chat knows
//!   it when the run takes it.
//! - Held follow-ups the mapper releases ([`Mapped::Send`](crate::opencode_server_v2::Mapped)) are
//!   sent in the background; one that fails is reported by the mapper. An abort asked for before
//!   the run started is sent again when it starts ([`Mapped::Abort`]).

use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use super::{Chat, Live, stream};
use crate::error::BusinessError;
use crate::opencode_server::HttpError;
use crate::opencode_server::wire::{Outgoing, slash_command};
use crate::opencode_server_v2::wire::{CommandBody, Delivery, FileRef, PromptBody};
use crate::opencode_server_v2::{MessageId, RouteV2, SessionId};

/// How a message is sent.
pub(super) enum Request {
    /// `prompt` with this body.
    Prompt(PromptBody),
    /// `command` with this body.
    Command(CommandBody),
    /// `prompt` with this body, which attaches a skill.
    Skill(PromptBody),
}

impl Request {
    /// Prepares `message` as the next turn's: a server command, a skill or a prompt. The caller
    /// counts it as a prompt on its way.
    pub(super) fn next_turn(live: &Live, message: &Outgoing) -> Self {
        let Some((name, arguments)) = slash_command(&message.text) else {
            return Self::Prompt(prompt_body(message));
        };

        if live.commands.iter().any(|command| command == name) {
            return Self::Command(CommandBody {
                name: name.to_owned(),
                text: arguments.to_owned(),
            });
        }

        if live.skills.iter().any(|skill| skill == name) {
            let mut body = prompt_body(message).with_skill(name);
            body.text = arguments.to_owned();

            return Self::Skill(body);
        }

        Self::Prompt(prompt_body(message))
    }

    /// Steering `message` (named `id`) for the running turn.
    pub(super) fn steering(message: &Outgoing, id: MessageId) -> Self {
        Self::Prompt(prompt_body(message).with_id(id).with_delivery(Delivery::Steer))
    }

    /// Sends the request; `Ok` for a `2xx` answer (any other is [`HttpError::Status`]).
    pub(super) async fn post(&self, chat: &Chat, session: &SessionId) -> Result<(), HttpError> {
        let directory = chat.directory.as_str();

        let sent = match self {
            Self::Prompt(body) | Self::Skill(body) => {
                let route = RouteV2::Prompt { session, directory };
                chat.client.send(route, Some(&json(body))).await
            }
            Self::Command(body) => {
                let route = RouteV2::Command { session, directory };
                chat.client.send(route, Some(&json(body))).await
            }
        };

        sent?.error_for_status()?;
        Ok(())
    }

    /// True for a server command or a skill.
    fn runs_command(&self) -> bool {
        matches!(self, Self::Command(_) | Self::Skill(_))
    }
}

/// The prompt body of `message`: its text and each image as a `data:` URL file named
/// `image-<n>.<ext>`.
fn prompt_body(message: &Outgoing) -> PromptBody {
    let files = message
        .images
        .iter()
        .enumerate()
        .map(|(index, image)| FileRef {
            uri: format!("data:{};base64,{}", image.mime_type, image.data),
            name: Some(image_name(index, &image.mime_type)),
        })
        .collect();

    PromptBody::text(message.text.clone()).with_files(files)
}

/// `image-<n>.<ext>` for the `n`th (1-based) image.
fn image_name(index: usize, mime_type: &str) -> String {
    let extension = mime_type.strip_prefix("image/").unwrap_or("bin");

    format!("image-{}.{extension}", index + 1)
}

/// The JSON of a request body (plain structs: serializing cannot fail).
fn json(body: &impl Serialize) -> Value {
    serde_json::to_value(body).unwrap_or(Value::Null)
}

/// Sends `message`, already counted as a prompt on its way, in the background. The answer runs
/// the prompt-taken call; a failure goes to the mapper.
pub(super) fn send(chat: &Arc<Chat>, live: &mut Live, message: Outgoing) {
    let request = Request::next_turn(live, &message);
    let session = live.session.clone();
    let chat = chat.clone();

    tokio::spawn(async move {
        let sent = request.post(&chat, &session).await;
        let mut live = chat.lock();

        match sent {
            Ok(()) => chat.shared.prompt_taken_now(),
            Err(error) => {
                // A command or skill OpenCode answered with an error was taken, as on OpenCode 1:
                // the prompt-taken call runs first.
                let taken = matches!(error, HttpError::Status { .. }) && request.runs_command();
                let error = BusinessError::from(error);
                let mapped = live.mapper.prompt_failed_late(&error.to_string(), taken);
                stream::apply(&chat, &mut live, mapped);
            }
        }
    });
}

/// Asks OpenCode in the background to interrupt the chat's running turn (an abort the user asked
/// for before the turn started); a failure leaves the turn running, as the user can abort again.
pub(super) fn abort(chat: &Arc<Chat>, live: &Live) {
    let session = live.session.clone();
    let chat = chat.clone();

    tokio::spawn(async move {
        let route = RouteV2::Interrupt {
            session: &session,
            directory: &chat.directory,
        };
        let sent = chat.client.send(route, None).await;

        if let Err(error) = sent.and_then(|response| response.error_for_status()) {
            tracing::debug!(%error, "OpenCode was not asked again to interrupt the turn");
        }
    });
}
