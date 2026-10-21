//! Dialogs of an [`OpenCodeV2Session`]: answers to OpenCode 2's permission asks and forms (see the
//! mapper's `asks`). An answer goes to the asking session's route with that session's directory (a
//! subagent's own); a form with several fields shows its next field first and sends the answers
//! together.
//!
//! - **Permission:** `once` and `reject` as chosen. `Always allow` is sent as `once` and kept by
//!   the chat (`approvals.rs`), so OpenCode saves nothing for later chats. A `reject` makes
//!   OpenCode reject every other ask of that session too; its `permission.replied` events close
//!   their dialogs.
//! - **Form:** `POST …/form/{id}/reply {answer: {key: value}}`; a dismissed dialog cancels the form
//!   (`DELETE …/form/{id}`).
//! - **Unknown agents:** with an approval mode, an answer that allows a `subagent` ask for an agent
//!   the mode's probe did not list (`guard.rs`) fails and shows the dialog again: only `reject`
//!   reaches OpenCode.
//! - An answer that did not reach OpenCode (no answer, or a `5xx`) fails and shows the dialog again
//!   ([`Mapper::reopen`](crate::opencode_server_v2::Mapper::reopen)), as OpenCode still waits; a
//!   `4xx` means the ask is gone, and the dialog stays closed.

use std::sync::Arc;

use serde_json::Value;

use super::approvals::{PermissionAnswer, always_allowed, asker_directory, undelivered};
use super::{Chat, OpenCodeV2Session, guard, stream};
use crate::error::{BusinessError, Result};
use crate::model::{MAX_ACTION_TEXT_BYTES, UiOutcome, UiResponse};
use crate::opencode_server::HttpError;
use crate::opencode_server_v2::mapper::{Answer, Reply, ReplyKind};
use crate::opencode_server_v2::wire::{Decision, PermissionRequest};
use crate::opencode_server_v2::{FormId, Mapped, RouteV2, SessionId};

impl OpenCodeV2Session {
    /// Answers an open dialog; fails for ids that are not open (answered, cancelled or unknown).
    pub(super) async fn answer_dialog(&self, response: UiResponse) -> Result<()> {
        if let UiOutcome::Value { value } = &response.outcome
            && value.len() > MAX_ACTION_TEXT_BYTES
        {
            return Err(BusinessError::InvalidInput(format!(
                "value exceeds {MAX_ACTION_TEXT_BYTES} bytes"
            )));
        }

        let chat = &self.chat;
        let (reply, directory, always) = {
            let mut live = chat.lock();

            let reply = match live.mapper.answer(&response)? {
                Answer::Next(events) => {
                    stream::apply(chat, &mut live, events.into_iter().map(Mapped::event).collect());
                    return Ok(());
                }
                Answer::Send(reply) => reply,
            };

            if allows_unknown_agent(chat, &reply, &live.mapper.pending_permissions()) {
                let reopened = live.mapper.reopen(request_id(&reply.kind));
                stream::apply(chat, &mut live, reopened.into_iter().map(Mapped::event).collect());

                return Err(BusinessError::Forbidden(UNKNOWN_AGENT.to_owned()));
            }

            let directory = asker_directory(chat, &live, &reply.session);
            let always = always_answer(&reply, &live.mapper.pending_permissions());

            (reply, directory, always)
        };

        let delivered = send(chat, &reply, directory).await?;

        if let Err(error) = &delivered
            && undelivered(error)
        {
            let mut live = chat.lock();
            let reopened = live.mapper.reopen(request_id(&reply.kind));
            stream::apply(chat, &mut live, reopened.into_iter().map(Mapped::event).collect());
        }

        delivered?;

        if let Some(permission) = always {
            always_allowed(chat, &permission);
        }

        Ok(())
    }
}

/// Why an answer that allows a `subagent` ask for an unknown agent fails.
const UNKNOWN_AGENT: &str = "This subagent is not covered by the chat's approval mode, so it can only be rejected.";

/// True when `reply` allows (once or always) a `subagent` ask, among the `pending` ones, for an
/// agent the approval mode's probe did not list.
fn allows_unknown_agent(chat: &Chat, reply: &Reply, pending: &[PermissionRequest]) -> bool {
    let ReplyKind::Permission { id, decision } = &reply.kind else {
        return false;
    };

    if *decision == Decision::Reject {
        return false;
    }

    pending
        .iter()
        .find(|permission| permission.id == *id)
        .is_some_and(|permission| guard::unknown_agent(chat, permission))
}

/// The request an `Always allow` answer in `reply` is for, among the `pending` ones.
fn always_answer(reply: &Reply, pending: &[PermissionRequest]) -> Option<PermissionRequest> {
    let ReplyKind::Permission {
        id,
        decision: Decision::Always,
    } = &reply.kind
    else {
        return None;
    };

    pending.iter().find(|permission| permission.id == *id).cloned()
}

/// Sends `reply`; an invalid id or body fails at once, the HTTP outcome comes back as it is.
async fn send(chat: &Arc<Chat>, reply: &Reply, directory: String) -> Result<std::result::Result<(), HttpError>> {
    let session = &reply.session;

    match &reply.kind {
        ReplyKind::Permission { id, decision } => {
            // The chat keeps an `always` itself: OpenCode would save it for later chats.
            let decision = match decision {
                Decision::Always => Decision::Once,
                other => *other,
            };
            let answer = PermissionAnswer::new(session, id, decision, directory)?;

            Ok(answer.send(&chat.client).await)
        }
        ReplyKind::Form { id, body } => {
            let session = SessionId::parse(session)?;
            let form = FormId::parse(id)?;
            let route = RouteV2::FormReply {
                session: &session,
                form: &form,
                directory: &directory,
            };
            let body = serde_json::to_value(body)
                .map_err(|error| BusinessError::Internal(format!("an OpenCode request body: {error}")))?;

            Ok(sent(chat.client.send(route, Some(&body)).await))
        }
        ReplyKind::FormCancel { id } => {
            let session = SessionId::parse(session)?;
            let form = FormId::parse(id)?;
            let route = RouteV2::FormCancel {
                session: &session,
                form: &form,
                directory: &directory,
            };

            Ok(sent(chat.client.send(route, None::<&Value>).await))
        }
    }
}

/// The outcome of a sent request.
fn sent(
    response: std::result::Result<crate::opencode_server::http::Response, HttpError>,
) -> std::result::Result<(), HttpError> {
    response.and_then(|response| response.error_for_status()).map(|_| ())
}

/// The request `reply` answers.
fn request_id(reply: &ReplyKind) -> &str {
    match reply {
        ReplyKind::Permission { id, .. } | ReplyKind::Form { id, .. } | ReplyKind::FormCancel { id } => id,
    }
}
