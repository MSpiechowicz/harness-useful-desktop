//! Keeps an approval mode exact while an OpenCode 2 chat runs (`probe.rs` sets it up at startup).
//!
//! OpenCode 2 reloads its agents at runtime: an agent file added or changed, or a configuration
//! change, rebuilds the list (`config/plugin/agent.ts`) and announces it (`agent.updated`, after a
//! `config.updated`). The mode's configuration names only the agents the probe listed, so a new
//! agent gets none of the mode's rules, and the `subagent` tool runs any agent OpenCode lists now.
//!
//! - **Check again:** on `agent.updated` and `config.updated`, after the event stream was opened
//!   again (one may have been missed), and when a `subagent` ask names an agent the probe did not
//!   list, `GET /api/agent` is read and checked as at startup ([`check_applied`]), asked again for
//!   at most [`AGENT_WAIT`] while OpenCode applies the configuration. When the agents no longer have
//!   the mode's rules, or the list cannot be read, the chat stops with [`NOTICE`] (`stream.rs`): a
//!   new chat probes again.
//! - **"Always allow"** never answers a `subagent` ask for an agent the probe did not list: it shows
//!   as a dialog.
//! - **Not covered:** saved "Always allow" rules added to the project while the chat runs (another
//!   OpenCode client of the project answering "Always"). OpenCode 2 applies them on every check
//!   (`permission.ts`) but announces no change, so only the startup reads them.

use std::time::Duration;

use serde_json::Value;

use super::Chat;
use super::probe::ModeLaunch;
use super::startup::{AGENT_POLL, AGENT_WAIT, get_data};
use crate::opencode_server_v2::RouteV2;
use crate::opencode_server_v2::events::EventKind;
use crate::opencode_server_v2::mode_config::{check_applied, listed_agents};
use crate::opencode_server_v2::wire::PermissionRequest;

/// Shown when the chat stops because the mode may no longer hold.
pub(super) const NOTICE: &str = "OpenCode reloaded its agents and the chat cannot confirm that the approval mode \
                                 still covers all of them, so this chat stops. Start a new chat to apply the \
                                 mode again.";

/// The action of the `subagent` tool's asks; their resources are agent ids.
const SUBAGENT: &str = "subagent";

/// Longest a check may take, its reads included.
const CHECK_LIMIT: Duration = Duration::from_secs(10);

/// What the startup learned about the mode: the probe's agents and the mode's configuration.
pub(super) struct ModeGuard {
    probed: Value,
    config: Value,
    /// The ids of the probe's agents.
    ids: Vec<String>,
}

impl ModeGuard {
    /// The guard of a chat started with `moded`.
    pub(super) fn new(moded: &ModeLaunch) -> Self {
        let listed = listed_agents(&moded.agents).unwrap_or_default();

        Self {
            probed: moded.agents.clone(),
            config: moded.config.clone(),
            ids: listed.into_iter().map(|(id, _)| id).collect(),
        }
    }
}

/// True for the events after which OpenCode may list other agents or rules.
pub(super) fn rechecks(kind: &EventKind) -> bool {
    matches!(kind, EventKind::AgentsUpdated | EventKind::ConfigUpdated)
}

/// True when `permission` is a `subagent` ask of a chat with a mode that names an agent the probe
/// did not list.
pub(super) fn unknown_agent(chat: &Chat, permission: &PermissionRequest) -> bool {
    let Some(guard) = &chat.guard else {
        return false;
    };

    if permission.action != SUBAGENT {
        return false;
    }

    permission
        .resources
        .iter()
        .any(|resource| !guard.ids.contains(resource))
}

/// Checks that the server's agents still have the mode's rules (see the module docs); `Ok` for a
/// chat without a mode. The error says why not.
pub(super) async fn recheck(chat: &Chat) -> Result<(), String> {
    let Some(guard) = &chat.guard else {
        return Ok(());
    };

    let checked = tokio::time::timeout(CHECK_LIMIT, applied(chat, guard)).await;
    let result = checked.unwrap_or_else(|_| Err("OpenCode did not list its agents in time".to_owned()));

    if let Err(why) = &result {
        tracing::warn!(%why, "the approval mode of an OpenCode 2 chat no longer holds");
    }

    result
}

/// Reads `GET /api/agent` until it has the mode's rules, for at most [`AGENT_WAIT`].
async fn applied(chat: &Chat, guard: &ModeGuard) -> Result<(), String> {
    let until = tokio::time::Instant::now() + AGENT_WAIT;
    let directory = chat.directory.as_str();

    loop {
        let listed = get_data::<Value>(&chat.client, RouteV2::AgentList { directory }).await;
        let checked = match listed {
            Ok(listed) => check_applied(&guard.probed, &listed, &guard.config),
            Err(error) => Err(error.to_string()),
        };

        match checked {
            Ok(()) => return Ok(()),
            Err(why) if tokio::time::Instant::now() >= until => return Err(why),
            Err(_) => tokio::time::sleep(AGENT_POLL).await,
        }
    }
}
