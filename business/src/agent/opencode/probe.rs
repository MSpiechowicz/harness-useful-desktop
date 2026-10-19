//! The first of two starts of an OpenCode chat with an approval mode
//! ([`mode_config`](crate::opencode_server::mode_config)): `opencode serve` runs once as the
//! launch asks, without the mode, only to report every agent's rules (`GET /agent` for the
//! project), and stops. The mode's configuration is merged into the `OPENCODE_CONFIG_CONTENT` the
//! launch already carries (the app's environment or the launch spec), ours winning, and the chat's
//! real server starts with it. Content that is not a JSON object refuses the mode before anything
//! starts: dropping it would drop the user's settings.
//!
//! The probe server gets its own random password and is stopped like the chat's server; its
//! output and exit never reach the chat. It counts against the chat's startup deadline.

use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use super::startup::{Spawned, get_json, listening_client, spawn_server};
use crate::agent::{EventSink, SessionOptions};
use crate::error::{BusinessError, Result};
use crate::model::{AgentEnvelope, ApprovalMode};
use crate::opencode_server::Route;
use crate::opencode_server::mode_config::{inherited_config, merge_deep, mode_config};
use crate::process::{ExecSpec, ProcessSpawner};

/// The variable OpenCode reads its last configuration layer from.
pub(super) const CONFIG_CONTENT: &str = "OPENCODE_CONFIG_CONTENT";

/// What the probe learned and what the chat's server starts with.
pub(super) struct ModeLaunch {
    /// The agents without the mode (`GET /agent`).
    pub agents: Value,
    /// The mode as configuration (`agent.<name>.permission`).
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
    pub options: &'a SessionOptions,
}

/// Events of the probe server go nowhere.
struct Discard;

impl EventSink for Discard {
    fn emit(&self, _envelope: AgentEnvelope) {}
}

/// Refuses the mode with `why`.
fn refused(why: &str) -> BusinessError {
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
    let content = merge_deep(&base, &config).to_string();

    Ok(ModeLaunch {
        agents,
        config,
        content,
    })
}

/// Starts the server as the launch asks, reads `GET /agent` and stops it.
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
        let project = probe
            .project
            .to_str()
            .ok_or_else(|| BusinessError::InvalidInput("the project path is not valid UTF-8".to_owned()))?;

        get_json(&client, Route::Agents { directory: project }).await
    };

    match read.await {
        Ok(agents) => {
            process.stop().await;
            Ok(agents)
        }
        Err(error) => Err(process.fail_start(error).await),
    }
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
