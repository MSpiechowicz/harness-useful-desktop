//! The first of two starts of an OpenCode 2 chat with an approval mode
//! ([`mode_config`](crate::opencode_server_v2::mode_config)): `opencode serve` runs once as the
//! launch asks, without the mode, only to report every agent's rules (`GET /api/agent`, read once
//! the list settled) for the directory the chat will work in (the project, or the resumed
//! session's directory, which may be a folder inside it with its own configuration), and stops.
//! The mode's configuration is added to the `OPENCODE_CONFIG_CONTENT` the launch already carries
//! (the app's environment or the launch spec, see [`merge_content`]), and the chat's real server
//! starts with it.
//!
//! Refused with a clear error: inherited content that is not a JSON object or configures an agent
//! the OpenCode 1 way (dropping it would drop the user's settings), a resumed session that is not
//! in the project or holds permission rules of its own other than denies (OpenCode applies them
//! after the mode), an agent list [`mode_config`] cannot use, and content larger than
//! [`MAX_CONFIG_CONTENT`] (the system's limit on one environment variable).
//!
//! After the chat's server started, [`check_mode`] waits until its agents have the mode's rules
//! ([`check_applied`]) and checks that the project has no saved "Always allow" rules for calls the
//! mode asks before ([`saved_overrides`]): OpenCode applies those after every agent's rules.
//!
//! The probe server gets its own random password and is stopped like the chat's server; its
//! output and exit never reach the chat. A failed probe is killed and reaped, one given up on
//! (deadline, stop) is killed. It counts against the chat's startup budget.

use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use super::startup::{
    AGENT_POLL, AGENT_WAIT, Spawned, agent_list, get_data, listening_client, ready, resumed, spawn_server,
};
use crate::agent::{EventSink, SessionOptions};
use crate::error::{BusinessError, Result};
use crate::model::{AgentEnvelope, ApprovalMode};
use crate::opencode_server::OpenCodeClient;
use crate::opencode_server::mapper::project_dir;
use crate::opencode_server::mode_config::inherited_config;
use crate::opencode_server_v2::RouteV2;
use crate::opencode_server_v2::mode_config::{
    check_applied, merge_content, mode_config, saved_overrides, stored_overrides,
};
use crate::process::{ExecSpec, ProcessSpawner};

/// The variable OpenCode reads its last configuration layer from.
const CONFIG_CONTENT: &str = "OPENCODE_CONFIG_CONTENT";

/// Largest `OPENCODE_CONFIG_CONTENT` the chat passes on, in bytes: well below the size of one
/// environment variable the system accepts (128 KiB on Linux, 32 767 characters on Windows).
pub(super) const MAX_CONFIG_CONTENT: usize = if cfg!(windows) { 30 * 1024 } else { 100 * 1024 };

/// What the probe learned and what the chat's server starts with.
pub(super) struct ModeLaunch {
    /// The mode.
    pub mode: ApprovalMode,
    /// The agents without the mode (`GET /api/agent` `data`).
    pub agents: Value,
    /// The mode as configuration (`agents.<id>.permissions`).
    pub config: Value,
    /// `OPENCODE_CONFIG_CONTENT` for the chat's server.
    pub content: String,
}

/// Where the probe runs.
pub(super) struct Probe<'a> {
    pub spawner: &'a dyn ProcessSpawner,
    pub spec: &'a ExecSpec,
    pub key: &'a str,
    pub project: &'a Path,
    /// The session the chat resumes.
    pub resume: Option<&'a str>,
    pub options: &'a SessionOptions,
}

/// Events of the probe server go nowhere.
struct Discard;

impl EventSink for Discard {
    fn emit(&self, _envelope: AgentEnvelope) {}
}

/// Refuses the mode with `why`.
pub(super) fn refused(why: &str) -> BusinessError {
    BusinessError::Unsupported(format!(
        "OpenCode cannot apply the approval mode exactly: {why}. Start the chat without an approval mode"
    ))
}

/// Runs the probe for `mode` (see the module docs).
pub(super) async fn mode_launch(probe: Probe<'_>, mode: ApprovalMode) -> Result<ModeLaunch> {
    let inherited = inherited_content(probe.spec);
    let base = inherited_config(inherited.as_deref())
        .map_err(|error| refused(&format!("the inherited {CONFIG_CONTENT} cannot be read ({error})")))?;

    let agents = probe_agents(&probe).await?;
    let config = mode_config(mode, &agents).map_err(|error| refused(&error))?;
    let content = merge_content(&base, &config)
        .map_err(|error| refused(&format!("the inherited {CONFIG_CONTENT} cannot take it ({error})")))?
        .to_string();

    if content.len() > MAX_CONFIG_CONTENT {
        return Err(refused(&format!(
            "its configuration takes {} KiB, more than the {} KiB one environment variable may hold",
            content.len().div_ceil(1024),
            MAX_CONFIG_CONTENT / 1024
        )));
    }

    Ok(ModeLaunch {
        mode,
        agents,
        config,
        content,
    })
}

/// Starts the server as the launch asks, reads the settled `GET /api/agent` and stops it.
async fn probe_agents(probe: &Probe<'_>) -> Result<Value> {
    let Spawned {
        process,
        password,
        listening,
    } = spawn_server(
        probe.spawner,
        probe.spec,
        format!("{}-probe", probe.key),
        Arc::new(Discard),
        probe.options,
    )?;

    let read = async {
        let client = listening_client(listening, password, probe.options.startup_timeout).await?;
        ready(&client).await?;
        let directory = chat_directory(&client, probe).await?;

        agent_list(&client, &directory, true).await
    };

    match read.await {
        Ok(agents) => {
            process.stop().await;
            Ok(agents)
        }
        Err(error) => Err(process.fail_start(error).await),
    }
}

/// The agents of the chat's server, started with the mode (`GET /api/agent` `data` in
/// `directory`), once they have the mode's rules ([`check_applied`]): OpenCode applies the
/// configuration a moment after it first lists them, so the list is asked again for at most
/// [`AGENT_WAIT`]. Then the saved rules of `project` (see the module docs).
pub(super) async fn check_mode(
    client: &OpenCodeClient,
    moded: &ModeLaunch,
    directory: &str,
    project: &str,
) -> Result<Value> {
    let until = tokio::time::Instant::now() + AGENT_WAIT;

    let listed = loop {
        let listed: Value = get_data(client, RouteV2::AgentList { directory }).await?;

        match check_applied(&moded.agents, &listed, &moded.config) {
            Ok(()) => break listed,
            Err(why) if tokio::time::Instant::now() >= until => return Err(refused(&why)),
            Err(_) => tokio::time::sleep(AGENT_POLL).await,
        }
    };

    let saved: Value = get_data(client, RouteV2::PermissionSaved { project }).await?;
    let actions = saved_overrides(moded.mode, &saved);

    if !actions.is_empty() {
        return Err(refused(&format!(
            "this project has saved \"Always allow\" rules in OpenCode (for {}), which it applies after the mode",
            actions.join(", ")
        )));
    }

    Ok(listed)
}

/// The directory the chat will work in: the project, or the resumed session's (refused outside the
/// project, or with permission rules of its own that are not denies).
async fn chat_directory(client: &OpenCodeClient, probe: &Probe<'_>) -> Result<String> {
    let project = probe
        .project
        .to_str()
        .ok_or_else(|| BusinessError::InvalidInput("the project path is not valid UTF-8".to_owned()))?;

    let Some(id) = probe.resume else {
        return Ok(project.to_owned());
    };

    let info = resumed(client, id, project).await?;

    if project_dir(&info.location.directory, probe.project).is_none() {
        return Err(BusinessError::NotFound(format!("session {id} in this project")));
    }

    if stored_overrides(&info.permissions) {
        return Err(refused(
            "the session holds permission rules of its own (from another client), which OpenCode applies after \
             the mode",
        ));
    }

    Ok(info.location.directory)
}

/// The `OPENCODE_CONFIG_CONTENT` the launch carries: set in the spec, or inherited from the app's
/// environment unless the spec removes it.
fn inherited_content(spec: &ExecSpec) -> Option<String> {
    let name = OsString::from(CONFIG_CONTENT);

    if let Some((_, value)) = spec.env.iter().rev().find(|(key, _)| *key == name) {
        return Some(value.to_string_lossy().into_owned());
    }

    let removed = spec
        .env_remove_prefixes
        .iter()
        .any(|prefix| CONFIG_CONTENT.starts_with(&*prefix.to_string_lossy()));

    if removed {
        return None;
    }

    std::env::var(CONFIG_CONTENT).ok()
}

/// `spec` with `OPENCODE_CONFIG_CONTENT` set to `content` (any earlier value replaced).
pub(super) fn with_content(spec: &mut ExecSpec, content: &str) {
    let name = OsString::from(CONFIG_CONTENT);

    spec.env.retain(|(key, _)| *key != name);
    spec.env.push((name, content.into()));
}
