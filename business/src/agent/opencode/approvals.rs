//! Approval modes and helper chats of an [`OpenCodeSession`]: the session permission rules
//! ([`modes`](crate::opencode_server::modes)) set with `PATCH /session/{id} {permission}`.
//!
//! - **Stored rules:** OpenCode *appends* a `PATCH`ed list to the session's rules and keeps them in
//!   its database, also for its own apps later. Every list the chat sets holds a catch-all, so the
//!   last one decides alone, and the chat never sends a list the session already ends with (the
//!   stored list still grows by one list per mode or agent change, and per release).
//! - **Approval mode:** the rules depend on the agent, so they are set at startup (before `ready`)
//!   for the chat's agent and again before the first turn after the agent changed (agents change
//!   only while idle, and OpenCode reads the rules once per run). "Always allow" lives in the
//!   server's memory for the chat's lifetime; the app never saves it.
//! - **Release:** when the chat leaves a session it set a mode on (stop, new chat, fork, clone, a
//!   lost event stream), it appends the [neutral](crate::opencode_server::modes::neutral) list
//!   of the same agent, so the session decides as the user's OpenCode settings do. This is best
//!   effort within [`CLEANUP_TIMEOUT`]; after a crash the rules stay until the session is opened
//!   in the app again.
//! - **No override:** nothing is set, the user's OpenCode settings apply. A session that already
//!   holds rules (a crashed chat's, say) gets the neutral list first, and again for another agent.
//! - **Helper chat** (naming a chat): every tool is denied, and the session is deleted when the chat
//!   stops. A helper chat never resumes a session.
//! - A session the chat moves to (fork, clone, new chat) holds no rules: it gets the chat's mode
//!   ([`Chat::rules_for_new`]) before it is used.

use std::time::Duration;

use serde_json::{Value, json};

use super::Chat;
use crate::error::Result;
use crate::model::ApprovalMode;
use crate::opencode_server::modes::{Rule, agent_rules, deny_all, neutral, session_rules};
use crate::opencode_server::{OpenCodeClient, Route, SessionId};

/// Deadline of the calls that clean up as the chat leaves a session (the neutral rules, deleting a
/// helper chat's session).
pub(super) const CLEANUP_TIMEOUT: Duration = Duration::from_secs(3);

/// The session rules a launch asks for.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LaunchRules {
    /// The per-chat approval mode.
    pub approval: Option<ApprovalMode>,
    /// A helper chat: everything is denied.
    pub utility: bool,
}

/// Rules the chat set on its session, or found it ending with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SessionRules {
    /// The agent they are for.
    pub agent: String,
    /// The list the session's stored rules end with.
    pub list: Vec<Rule>,
    /// An approval mode's: released when the chat leaves the session.
    pub overrides: bool,
}

impl LaunchRules {
    /// The rules the chat keeps on a session for `agent` (its rules from the `GET /agent` list
    /// `agents`): a helper chat's, the approval mode's or, without an override, the neutral list
    /// when the session `holds` rules already; `None` when nothing is to be set.
    pub(super) fn wanted(&self, agents: &Value, agent: &str, holds: bool) -> Option<SessionRules> {
        let own = agent_rules(agents, agent);

        let (list, overrides) = if self.utility {
            (deny_all(), false)
        } else if let Some(list) = session_rules(self.approval, &own) {
            (list, true)
        } else if holds {
            (neutral(&own), false)
        } else {
            return None;
        };

        Some(SessionRules {
            agent: agent.to_owned(),
            list,
            overrides,
        })
    }
}

/// Appends `rules` to the stored rules of `session`, within `limit` (the client's default
/// without one).
pub(super) async fn patch_rules(
    client: &OpenCodeClient,
    session: &SessionId,
    rules: &[Rule],
    limit: Option<Duration>,
) -> Result<()> {
    let body = json!({ "permission": rules });
    let route = Route::SessionPatch { session };

    let response = match limit {
        Some(limit) => client.send_with_timeout(route, Some(&body), limit).await?,
        None => client.send(route, Some(&body)).await?,
    };

    response.error_for_status()?;
    Ok(())
}

/// Deletes `session` (a helper chat's, or one the chat created and could not move to); a failure
/// only leaves it behind.
pub(super) async fn delete_session(client: &OpenCodeClient, session: &SessionId) {
    let deleted = client
        .send_with_timeout(Route::SessionDelete { session }, None, CLEANUP_TIMEOUT)
        .await;

    if let Err(error) = deleted.and_then(|response| response.error_for_status()) {
        tracing::debug!(%error, "an OpenCode session the chat no longer uses was not deleted");
    }
}

impl Chat {
    /// Before a turn on `session`: sets the rules for the chat's agent unless the session already
    /// ends with them.
    pub(super) async fn rules_for_turn(&self, session: &SessionId) -> Result<()> {
        let wanted = {
            let live = self.lock();
            let wanted = self
                .rules
                .wanted(&self.catalog.agent_rules, &live.agent, live.rules.is_some());
            let current = live.rules.as_ref().map(|rules| &rules.list);

            wanted.filter(|wanted| current != Some(&wanted.list))
        };

        let Some(wanted) = wanted else {
            return Ok(());
        };

        patch_rules(&self.client, session, &wanted.list, None).await?;

        let mut live = self.lock();
        if live.session == *session {
            live.rules = Some(wanted);
        }

        Ok(())
    }

    /// Sets the chat's rules on `session`, a session the chat moves to (it holds none), before it
    /// is used; the caller keeps them once the move commits.
    pub(super) async fn rules_for_new(&self, session: &SessionId) -> Result<Option<SessionRules>> {
        let wanted = {
            let live = self.lock();
            self.rules.wanted(&self.catalog.agent_rules, &live.agent, false)
        };

        if let Some(wanted) = &wanted {
            patch_rules(&self.client, session, &wanted.list, None).await?;
        }

        Ok(wanted)
    }

    /// The chat left `session`, which holds `rules`: an approval mode's give way to the neutral
    /// list of the same agent (best effort, see the module docs).
    pub(super) async fn release_rules(&self, session: &SessionId, rules: Option<SessionRules>) {
        let Some(rules) = rules.filter(|rules| rules.overrides) else {
            return;
        };

        let list = neutral(&agent_rules(&self.catalog.agent_rules, &rules.agent));
        let released = patch_rules(&self.client, session, &list, Some(CLEANUP_TIMEOUT)).await;

        if let Err(error) = released {
            tracing::warn!(
                %error,
                "the OpenCode session keeps the chat's approval rules until the app opens it again"
            );
        }
    }

    /// The chat stops using its session: a helper chat's is deleted, an approval mode's rules are
    /// released. Runs once.
    pub(super) async fn leave_session(&self) {
        let (session, rules) = {
            let mut live = self.lock();
            (live.session.clone(), live.rules.take())
        };

        if self.rules.utility {
            if rules.is_some() {
                delete_session(&self.client, &session).await;
            }
            return;
        }

        self.release_rules(&session, rules).await;
    }
}
