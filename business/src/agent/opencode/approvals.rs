//! Approval modes and helper chats of an [`OpenCodeSession`].
//!
//! - **Approval mode:** set as configuration of the chat's own server, never on the session
//!   ([`mode_config`](crate::opencode_server::mode_config), `probe.rs`): every agent, subagents
//!   and hidden ones included, gets the mode's rules for the server's lifetime, so switching
//!   agents, forks, clones and new chats need nothing, and nothing outlives the chat. "Always
//!   allow" lives in the server's memory for the chat's lifetime and outranks the mode there; the
//!   app never saves it.
//! - **No override:** nothing is set or read, the user's OpenCode settings apply.
//! - **Helper chat** (naming a chat): its throwaway session denies every tool
//!   (`PATCH /session/{id} {permission}`, set before `ready`) and is deleted when the chat stops.
//!   A helper chat never resumes a session.

use std::time::Duration;

use serde_json::json;

use super::Chat;
use crate::error::Result;
use crate::model::ApprovalMode;
use crate::opencode_server::modes::deny_all;
use crate::opencode_server::{OpenCodeClient, Route, SessionId};

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

/// Makes helper session `session` deny every tool.
pub(super) async fn deny_helper_tools(client: &OpenCodeClient, session: &SessionId) -> Result<()> {
    let body = json!({ "permission": deny_all() });

    client
        .send(Route::SessionPatch { session }, Some(&body))
        .await?
        .error_for_status()?;

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
    /// Deletes a helper chat's session (it is never resumed); nothing for other chats.
    pub(super) async fn delete_helper_session(&self) {
        if !self.rules.utility {
            return;
        }

        let session = self.lock().session.clone();
        delete_session(&self.client, &session).await;
    }
}
