//! Team hooks kept by Harness Memory: whether the installed server keeps them, and the
//! `harness-memory hooks …` commands (see [`memory_cli`](super::memory_cli)) that list, store and
//! remove them. The app never connects to the team database; the server routes the hooks as its
//! own config says (`hooks = "remote"`, valid only with a team `database_url`).
//!
//! [`MemoryHooks::support`] runs `hooks config --json` (`{"hooks": "remote"|"local",
//! "trustTeamHooks": bool}`). A usage error (exit 2), which is how an older server answers a
//! command it does not know, means it has no team hooks ([`HooksSupport::Unsupported`]). The answer
//! is cached until the binary at that path or the server's config file changes (the config holds
//! both keys); a failed probe is not cached.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use serde_json::Value;

use super::catalog::MEMORY_BINARY;
use super::memory_binary::resolve_memory;
use super::memory_cli::{CliAnswer, CliCall, USAGE_EXIT, run_memory_cli};
use super::memory_remote::memory_config_path;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::process::ProcessRunner;

mod cli;

pub use cli::{TeamList, delete_team_hook, list_team_hooks, project_key, put_team_hook};

/// How long `hooks config` may take.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// The output kept of the probe.
const PROBE_OUTPUT_BYTES: usize = 64 * 1024;

/// What `hooks config --json` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HooksConfig {
    /// `hooks = "remote"` with a team database: hooks are shared with the team.
    pub remote: bool,
    /// `trust_team_hooks = true`: team hooks run without local approval.
    pub trust: bool,
}

/// Whether the installed server keeps team hooks, which decides whether the `hooks` and
/// `trust_team_hooks` keys may be saved and whether this machine runs team hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HooksSupport {
    /// It answers `hooks config`: what its config says.
    Supported(HooksConfig),
    /// It predates team hooks (a usage error) and refuses both keys.
    Unsupported,
    /// Harness Memory is not installed.
    NotInstalled,
    /// The server could not be asked (the probe failed): saved keys stay as they are, and the
    /// team copies are neither refreshed nor removed (but no longer trusted).
    Unknown,
}

impl HooksSupport {
    /// Whether the `hooks` and `trust_team_hooks` keys may be written.
    pub fn keeps_hooks(self) -> bool {
        matches!(self, Self::Supported(_))
    }

    /// The config while hooks are shared with the team.
    pub fn sharing(self) -> Option<HooksConfig> {
        match self {
            Self::Supported(config) if config.remote => Some(config),
            _ => None,
        }
    }
}

/// The probe of the installed server's team hooks. See the module docs.
#[derive(Debug, Default)]
pub struct MemoryHooks {
    probe: Mutex<Option<Probe>>,
}

/// A file's modification time and size.
type Stamp = (Option<SystemTime>, u64);

/// A successful probe of one binary, as it and the config file were when probed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Probe {
    path: PathBuf,
    modified: SystemTime,
    config: Option<Stamp>,
    answer: Answer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    Supported(HooksConfig),
    Unsupported,
}

impl MemoryHooks {
    /// No probe yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the installed server keeps team hooks, and what its config says (see the module
    /// docs).
    pub async fn support(&self, env: &Env, runner: &dyn ProcessRunner) -> HooksSupport {
        match self.probed(env, runner).await {
            Ok((_, Answer::Supported(config))) => HooksSupport::Supported(config),
            Ok((_, Answer::Unsupported)) => HooksSupport::Unsupported,
            Err(support) => support,
        }
    }

    /// The installed server's path while it shares hooks with the team, with its config.
    /// `unsupported` while sharing is off or the server has no team hooks, `notFound` without
    /// Harness Memory, `process` when it could not be asked.
    pub async fn sharing_server(&self, env: &Env, runner: &dyn ProcessRunner) -> Result<(PathBuf, HooksConfig)> {
        match self.probed(env, runner).await {
            Ok((path, Answer::Supported(config))) if config.remote => Ok((path, config)),
            Ok((_, Answer::Supported(_))) => Err(BusinessError::Unsupported(
                "sharing hooks with the team is off".to_owned(),
            )),
            Ok((_, Answer::Unsupported)) => Err(BusinessError::Unsupported(format!(
                "this {MEMORY_BINARY} predates team hooks. Update Harness Memory."
            ))),
            Err(HooksSupport::NotInstalled) => Err(BusinessError::NotFound(format!("executable {MEMORY_BINARY}"))),
            Err(_) => Err(BusinessError::Process("Harness Memory could not be asked".to_owned())),
        }
    }

    async fn probed(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
    ) -> std::result::Result<(PathBuf, Answer), HooksSupport> {
        let Some(binary) = resolve_memory(env) else {
            return Err(HooksSupport::NotInstalled);
        };

        let modified = std::fs::metadata(&binary.path)
            .and_then(|metadata| metadata.modified())
            .ok();
        let config = memory_config_path(env).and_then(|path| stamp(&path));

        if let Some(modified) = modified
            && let Some(answer) = self.cached(&binary.path, modified, config)
        {
            return Ok((binary.path, answer));
        }

        let answer = ask(env, runner, &binary.path).await.ok_or(HooksSupport::Unknown)?;

        if let Some(modified) = modified {
            self.remember(Probe {
                path: binary.path.clone(),
                modified,
                config,
                answer,
            });
        }

        Ok((binary.path, answer))
    }

    fn cached(&self, path: &Path, modified: SystemTime, config: Option<Stamp>) -> Option<Answer> {
        let probe = self.probe.lock().unwrap_or_else(PoisonError::into_inner);

        probe
            .as_ref()
            .filter(|probe| probe.path == path && probe.modified == modified && probe.config == config)
            .map(|probe| probe.answer)
    }

    fn remember(&self, probe: Probe) {
        *self.probe.lock().unwrap_or_else(PoisonError::into_inner) = Some(probe);
    }
}

/// `hooks config --json` on `server`; `None` when the answer cannot be used (not cached).
async fn ask(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Option<Answer> {
    let call = CliCall {
        name: "hooks config",
        args: ["hooks", "config", "--json"].map(OsString::from).into(),
        timeout: PROBE_TIMEOUT,
        output_bytes: PROBE_OUTPUT_BYTES,
        stdin: None,
    };

    match run_memory_cli(env, runner, server, call).await {
        Ok(CliAnswer::Done(stdout)) => {
            let config = parse_config(&stdout);

            if config.is_none() {
                tracing::warn!("harness-memory hooks config answered output this app cannot read");
            }

            config.map(Answer::Supported)
        }
        Ok(CliAnswer::Failed { status, message, .. })
            if status == Some(USAGE_EXIT) || names_unknown_command(&message) =>
        {
            Some(Answer::Unsupported)
        }
        Ok(CliAnswer::Failed { status, .. }) => {
            tracing::warn!(?status, "harness-memory hooks config failed");
            None
        }
        Err(error) => {
            tracing::warn!(code = ?error.code(), "harness-memory hooks config did not run");
            None
        }
    }
}

/// `{"hooks": "remote"|"local", "trustTeamHooks": bool}`; any other `hooks` value is local and a
/// missing `trustTeamHooks` false.
fn parse_config(stdout: &str) -> Option<HooksConfig> {
    let value: Value = serde_json::from_str(stdout).ok()?;
    let hooks = value.get("hooks")?.as_str()?;

    Some(HooksConfig {
        remote: hooks == "remote",
        trust: value.get("trustTeamHooks").and_then(Value::as_bool).unwrap_or(false),
    })
}

/// True when the server's line says it does not know the command (a server whose usage errors do
/// not exit 2).
fn names_unknown_command(message: &str) -> bool {
    let message = message.to_lowercase();

    ["unrecognized subcommand", "unknown subcommand", "unknown command"]
        .iter()
        .any(|marker| message.contains(marker))
}

fn stamp(path: &Path) -> Option<Stamp> {
    let metadata = std::fs::metadata(path).ok()?;

    Some((metadata.modified().ok(), metadata.len()))
}
