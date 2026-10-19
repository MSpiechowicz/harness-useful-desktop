//! Approval modes and helper chats of an [`OpenCodeSession`]: the session permission rules
//! ([`modes`](crate::opencode_server::modes)) set with `PATCH /session/{id} {permission}`.
//!
//! - **Approval mode:** the rules depend on the agent, so they are set at startup (before `ready`)
//!   for the chat's agent and again before the first turn after the agent changed (agents change
//!   only while idle, and OpenCode reads the rules once per run). Without an override nothing is
//!   set and the user's OpenCode settings apply. "Always allow" lives in the server's memory for
//!   the chat's lifetime; the app never saves it.
//! - **Helper chat** (naming a chat): every tool is denied, and the session is deleted when the chat
//!   stops. A helper chat never resumes a session.
//! - A session the chat moves to (fork, clone, new chat) gets the same rules with
//!   `Chat::apply_rules`.

use serde_json::{Value, json};

use super::{Chat, OpenCodeSession};
use crate::error::Result;
use crate::model::ApprovalMode;
use crate::opencode_server::modes::{agent_rules, deny_all, session_rules};
use crate::opencode_server::{OpenCodeClient, Route, SessionId};

/// The session rules a launch asks for.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LaunchRules {
    /// The per-chat approval mode.
    pub approval: Option<ApprovalMode>,
    /// A helper chat: everything is denied.
    pub utility: bool,
}

impl LaunchRules {
    /// The `PATCH /session/{id}` body for `agent` (rules from the `GET /agent` list `agents`);
    /// `None` when the user's settings apply.
    pub(super) fn body(&self, agents: &Value, agent: &str) -> Option<Value> {
        let rules = if self.utility {
            deny_all()
        } else {
            session_rules(self.approval, &agent_rules(agents, agent))?
        };

        Some(json!({ "permission": rules }))
    }
}

/// Sets the session rules `body` on `session`.
pub(super) async fn patch_rules(client: &OpenCodeClient, session: &SessionId, body: &Value) -> Result<()> {
    client
        .send(Route::SessionPatch { session }, Some(body))
        .await?
        .error_for_status()?;

    Ok(())
}

impl Chat {
    /// Before a turn on `session`: sets the rules when the agent changed since they were set.
    pub(super) async fn rules_for_turn(&self, session: &SessionId) -> Result<()> {
        let due = {
            let live = self.lock();
            live.rules_agent.as_deref() != Some(live.agent.as_str())
        };

        if !due {
            return Ok(());
        }

        self.apply_rules(session).await
    }

    /// Gives `session` the chat's rules for its current agent; nothing without an override. Also
    /// for a session the chat moves to (fork, clone, new chat), before it is used.
    pub(super) async fn apply_rules(&self, session: &SessionId) -> Result<()> {
        let (body, agent) = {
            let live = self.lock();
            (
                self.rules.body(&self.catalog.agent_rules, &live.agent),
                live.agent.clone(),
            )
        };

        let Some(body) = body else {
            return Ok(());
        };

        patch_rules(&self.client, session, &body).await?;
        self.lock().rules_agent = Some(agent);

        Ok(())
    }
}

impl OpenCodeSession {
    /// Deletes a helper chat's session (it is never resumed); a failure only leaves it behind.
    pub(super) async fn delete_helper_session(&self) {
        let chat = &self.chat;

        if !chat.rules.utility {
            return;
        }

        let session = chat.lock().session.clone();
        let deleted = chat.client.send(Route::SessionDelete { session: &session }, None).await;

        if let Err(error) = deleted.and_then(|response| response.error_for_status()) {
            tracing::debug!(%error, "the helper chat's OpenCode session was not deleted");
        }
    }
}
