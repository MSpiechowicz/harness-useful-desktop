//! Dialogs of an [`OpenCodeSession`]: answers to OpenCode's permission asks and questions (see
//! the mapper's `asks`). An answer goes to the request's route with the directory of the session
//! that asked (`/permission/*` and `/question/*` are not session-scoped); a question with several
//! parts shows its next part first. An answer that did not reach OpenCode (no answer, or a `5xx`)
//! fails and shows the dialog again ([`Mapper::reopen`](crate::opencode_server::Mapper::reopen)),
//! as OpenCode still waits; a `404` means the ask is gone, and the dialog stays closed.

use serde_json::json;

use super::{OpenCodeSession, stream};
use crate::error::{BusinessError, Result};
use crate::model::{MAX_ACTION_TEXT_BYTES, UiOutcome, UiResponse};
use crate::opencode_server::mapper::{Answer, ReplyKind};
use crate::opencode_server::{HttpError, Mapped, PermissionId, QuestionId, Route};

impl OpenCodeSession {
    /// Answers an open dialog; fails for ids that are not open (answered, cancelled or unknown).
    pub async fn respond(&self, response: UiResponse) -> Result<()> {
        self.process.ensure_running()?;

        if let UiOutcome::Value { value } = &response.outcome
            && value.len() > MAX_ACTION_TEXT_BYTES
        {
            return Err(BusinessError::InvalidInput(format!(
                "value exceeds {MAX_ACTION_TEXT_BYTES} bytes"
            )));
        }

        let chat = &self.chat;
        let (reply, directory) = {
            let mut live = chat.lock();

            let reply = match live.mapper.answer(&response)? {
                Answer::Next(events) => {
                    stream::apply(chat, &mut live, events.into_iter().map(Mapped::Event).collect());
                    return Ok(());
                }
                Answer::Send(reply) => reply,
            };

            let directory = live
                .mapper
                .subagent_directory(&reply.session)
                .unwrap_or(&chat.directory)
                .to_owned();

            (reply.kind, directory)
        };

        let request = request_id(&reply).to_owned();
        let sent = match &reply {
            ReplyKind::Permission { id, reply } => {
                let id = PermissionId::parse(id)?;
                let route = Route::PermissionReply {
                    id: &id,
                    directory: &directory,
                };
                chat.client.send(route, Some(&json!({ "reply": reply }))).await
            }
            ReplyKind::Question { id, answers } => {
                let id = QuestionId::parse(id)?;
                let route = Route::QuestionReply {
                    id: &id,
                    directory: &directory,
                };
                chat.client.send(route, Some(&json!({ "answers": answers }))).await
            }
            ReplyKind::QuestionReject { id } => {
                let id = QuestionId::parse(id)?;
                let route = Route::QuestionReject {
                    id: &id,
                    directory: &directory,
                };
                chat.client.send(route, Some(&json!({}))).await
            }
        };

        let delivered = sent.and_then(|response| response.error_for_status());

        if let Err(error) = &delivered
            && undelivered(error)
        {
            let mut live = chat.lock();
            let reopened = live.mapper.reopen(&request);
            stream::apply(chat, &mut live, reopened.into_iter().map(Mapped::Event).collect());
        }

        delivered?;
        Ok(())
    }
}

/// True when an answer did not reach OpenCode: no answer, or a server error.
fn undelivered(error: &HttpError) -> bool {
    match error {
        HttpError::Status { status } => *status >= 500,
        _ => true,
    }
}

/// The request `reply` answers.
fn request_id(reply: &ReplyKind) -> &str {
    match reply {
        ReplyKind::Permission { id, .. } | ReplyKind::Question { id, .. } | ReplyKind::QuestionReject { id } => id,
    }
}
