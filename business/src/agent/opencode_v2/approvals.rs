//! Approval modes, "Always allow" and helper chats of an [`OpenCodeV2Session`](super::OpenCodeV2Session).
//!
//! - **Approval mode:** configuration of the chat's own server, never set on the session
//!   ([`mode_config`](crate::opencode_server_v2::mode_config), `probe.rs`): every agent, subagents
//!   and hidden ones included, gets the mode's rules for the server's lifetime, so switching
//!   agents, forks, clones and new chats need nothing, and nothing outlives the chat.
//! - **"Always allow"** lasts as long as the chat. OpenCode 2 would save an `always` answer for the
//!   project, for every later chat (`permission.ts`), so the chat answers `once` and keeps the
//!   request's action and `save` patterns itself ([`Allowed`]). A later ask of the chat or its
//!   subagents they cover is answered `once` without a dialog, as are the asks already waiting
//!   that they cover. OpenCode never asks for a call its rules deny, so a deny still wins. When
//!   such an answer does not reach OpenCode, the ask shows as a dialog.
//! - **No override:** nothing is set or read, the user's OpenCode settings apply.
//! - **Helper chat** (naming a chat): its session is created with rules that deny every tool
//!   (`POST /api/session {permissions}`, which the sessions started from it inherit) and deleted
//!   when the chat stops or loses its server's events. A helper chat never resumes a session.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::{Chat, Live, stream};
use crate::error::{BusinessError, Result};
use crate::model::ApprovalMode;
use crate::opencode_server::{HttpError, OpenCodeClient};
use crate::opencode_server_v2::events::{Event, EventKind};
use crate::opencode_server_v2::modes::{Rule, decide};
use crate::opencode_server_v2::wire::{Decision, PermissionReplyBody, PermissionRequest};
use crate::opencode_server_v2::{PermissionId, RouteV2, SessionId};

/// Deadline of the clean-up calls when the chat stops (deleting a helper chat's session).
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(3);

/// The permissions a launch asks for.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LaunchRules {
    /// The per-chat approval mode.
    pub approval: Option<ApprovalMode>,
    /// A helper chat: everything is denied.
    pub utility: bool,
}

impl LaunchRules {
    /// The approval mode the server is configured with; never for a helper chat, which denies
    /// everything anyway.
    pub(super) fn mode(&self) -> Option<ApprovalMode> {
        self.approval.filter(|_| !self.utility)
    }
}

/// What the chat's "Always allow" answers allow (see the module docs).
#[derive(Debug, Default)]
pub(super) struct Allowed {
    /// One `allow` rule per action and `save` pattern.
    rules: Vec<Rule>,
}

impl Allowed {
    /// Allows `permission`'s action on its `save` patterns from now on.
    pub(super) fn remember(&mut self, permission: &PermissionRequest) {
        for pattern in &permission.save {
            let rule = Rule::new(&permission.action, pattern, "allow");

            if !self.rules.contains(&rule) {
                self.rules.push(rule);
            }
        }
    }

    /// True when the remembered answers allow every resource `permission` asks for.
    pub(super) fn covers(&self, permission: &PermissionRequest) -> bool {
        let action = permission.action.as_str();

        !permission.resources.is_empty()
            && permission
                .resources
                .iter()
                .all(|resource| decide(&self.rules, action, resource) == "allow")
    }
}

/// Answers `event` `once` without a dialog when it is a permission ask of the chat (or one of its
/// subagents) that the chat's "Always allow" answers cover; true when it does.
pub(super) fn answered_by_always(chat: &Arc<Chat>, live: &Live, event: &Event) -> bool {
    let EventKind::PermissionAsked(permission) = &event.kind else {
        return false;
    };

    if !live.mapper.is_chat_session(&permission.session_id) || !live.allowed.covers(permission) {
        return false;
    }

    let directory = asker_directory(chat, live, &permission.session_id);
    tokio::spawn(allow_once(
        chat.clone(),
        permission.clone(),
        directory,
        Some(event.clone()),
    ));

    true
}

/// After an "Always allow" answer to `permission`: remembers it and answers the waiting asks of
/// the chat it covers.
pub(super) fn always_allowed(chat: &Arc<Chat>, permission: &PermissionRequest) {
    let live = &mut *chat.lock();
    live.allowed.remember(permission);

    let pending = live.mapper.pending_permissions().into_iter();
    let covered = pending.filter(|waiting| waiting.id != permission.id && live.allowed.covers(waiting));

    for waiting in covered.collect::<Vec<_>>() {
        let directory = asker_directory(chat, live, &waiting.session_id);
        tokio::spawn(allow_once(chat.clone(), waiting, directory, None));
    }
}

/// The directory of the session that asked: a subagent's own, else the chat's.
pub(super) fn asker_directory(chat: &Chat, live: &Live, session: &str) -> String {
    live.mapper
        .subagent_directory(session)
        .unwrap_or(&chat.directory)
        .to_owned()
}

/// Answers `permission` `once`; when the answer does not reach OpenCode, which still waits, the
/// `shown` event (the ask) is mapped as a dialog.
async fn allow_once(chat: Arc<Chat>, permission: PermissionRequest, directory: String, shown: Option<Event>) {
    let answer = PermissionAnswer::new(&permission.session_id, &permission.id, Decision::Once, directory);

    let undelivered = match answer {
        Ok(answer) => match answer.send(&chat.client).await {
            Ok(()) => return,
            Err(error) => {
                tracing::debug!(%error, "an OpenCode permission ask the chat allows was not answered");
                undelivered(&error)
            }
        },
        Err(error) => {
            tracing::debug!(%error, "an OpenCode permission ask the chat allows cannot be answered");
            true
        }
    };

    let Some(event) = shown.filter(|_| undelivered) else {
        return;
    };

    let mut live = chat.lock();
    let mapped = live.mapper.map(&event);
    stream::apply(&chat, &mut live, mapped);
}

/// A permission answer ready to send: `POST …/permission/{id}/reply {decision}`.
pub(super) struct PermissionAnswer {
    session: SessionId,
    id: PermissionId,
    body: Value,
    directory: String,
}

impl PermissionAnswer {
    /// `decision` for ask `id` of `session` (the session's directory goes with it).
    pub(super) fn new(session: &str, id: &str, decision: Decision, directory: String) -> Result<Self> {
        let body = PermissionReplyBody {
            decision,
            message: None,
        };
        let body = serde_json::to_value(body)
            .map_err(|error| BusinessError::Internal(format!("an OpenCode request body: {error}")))?;

        Ok(Self {
            session: SessionId::parse(session)?,
            id: PermissionId::parse(id)?,
            body,
            directory,
        })
    }

    /// Sends the answer.
    pub(super) async fn send(&self, client: &OpenCodeClient) -> std::result::Result<(), HttpError> {
        let route = RouteV2::PermissionReply {
            session: &self.session,
            permission: &self.id,
            directory: &self.directory,
        };

        client.send(route, Some(&self.body)).await?.error_for_status()?;
        Ok(())
    }
}

/// True when an answer did not reach OpenCode: no answer, or a server error. A `4xx` means the
/// ask is gone.
pub(super) fn undelivered(error: &HttpError) -> bool {
    match error {
        HttpError::Status { status } => *status >= 500,
        _ => true,
    }
}

impl Chat {
    /// Deletes a helper chat's session (it is never resumed); nothing for other chats. A failure
    /// only leaves the session behind.
    pub(super) async fn delete_helper_session(&self) {
        if !self.rules.utility {
            return;
        }

        let session = self.lock().session.clone();
        let route = RouteV2::SessionDelete {
            session: &session,
            directory: &self.directory,
        };
        let deleted = self.client.send_with_timeout(route, None, CLEANUP_TIMEOUT).await;

        if let Err(error) = deleted.and_then(|response| response.error_for_status()) {
            tracing::debug!(%error, "a helper chat's OpenCode session was not deleted");
        }
    }
}

/// True when a created helper session ends with the deny-all rule (its `permissions`).
pub(super) fn denies_all(permissions: &Value) -> bool {
    let rules: Vec<Rule> = serde_json::from_value(permissions.clone()).unwrap_or_default();

    rules
        .last()
        .is_some_and(|last| last.action == "*" && last.resource == "*" && last.is("deny"))
}
