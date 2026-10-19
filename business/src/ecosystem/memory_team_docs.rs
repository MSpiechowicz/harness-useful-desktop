//! Team documents kept by Harness Memory (team hooks and team routines): whether the installed server keeps a
//! kind, and the `harness-memory <plural> …` commands (see [`memory_cli`](super::memory_cli)) that
//! list, store and remove them. The app never connects to the team database; the server routes
//! the documents as its own config says (`hooks = "remote"`, valid only with a team
//! `database_url`). [`memory_hooks`](super::memory_hooks) is this module for hooks.
//!
//! [`MemoryTeamDocs::support`] runs `<plural> config --json` (`{"<plural>": "remote"|"local",
//! "<trust key>": bool}`, e.g. `{"hooks": …, "trustTeamHooks": …}`). A usage error (exit 2),
//! which is how an older server answers a command it does not know, means it has no team
//! documents of that kind ([`TeamDocsSupport::Unsupported`]). The answer is cached until the
//! binary at that path or the server's config file changes (the config holds both keys); a failed
//! probe is not cached.

use std::ffi::OsString;
use std::marker::PhantomData;
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
use crate::signed_docs::DocKind;

mod cli;

pub use cli::{
    DOCS_TIMEOUT, SharedDoc, TeamDocsRemoved, TeamList, delete_mine_team_docs, delete_team_doc, get_team_doc,
    list_team_docs, project_key, put_shared, put_team_doc,
};

/// How long `<plural> config` may take.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// The output kept of the probe.
const PROBE_OUTPUT_BYTES: usize = 64 * 1024;

/// What `<plural> config --json` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeamDocsConfig {
    /// `<plural> = "remote"` with a team database: the kind is shared with the team.
    pub remote: bool,
    /// The trust key is on: team documents are used without local approval.
    pub trust: bool,
}

/// Whether the installed server keeps team documents of a kind, which decides whether its config
/// keys may be saved and whether this machine uses the team's documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamDocsSupport {
    /// It answers `<plural> config`: what its config says.
    Supported(TeamDocsConfig),
    /// It predates the kind's team documents (a usage error) and refuses both keys.
    Unsupported,
    /// Harness Memory is not installed.
    NotInstalled,
    /// The server could not be asked (the probe failed): saved keys stay as they are, and the
    /// team copies are neither refreshed nor removed (but no longer trusted).
    Unknown,
}

impl TeamDocsSupport {
    /// Whether the kind's config keys may be written.
    pub fn keeps_docs(self) -> bool {
        matches!(self, Self::Supported(_))
    }

    /// Whether the `hooks` and `trust_team_hooks` keys may be written ([`Self::keeps_docs`], the
    /// name hooks callers use).
    pub fn keeps_hooks(self) -> bool {
        self.keeps_docs()
    }

    /// The config while the kind is shared with the team.
    pub fn sharing(self) -> Option<TeamDocsConfig> {
        match self {
            Self::Supported(config) if config.remote => Some(config),
            _ => None,
        }
    }
}

/// The probe of the installed server's team documents of kind `K`. See the module docs.
#[derive(Debug, Default)]
pub struct MemoryTeamDocs<K: DocKind> {
    probe: Mutex<Option<Probe>>,
    kind: PhantomData<K>,
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
    Supported(TeamDocsConfig),
    Unsupported,
}

impl<K: DocKind> MemoryTeamDocs<K> {
    /// No probe yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the installed server keeps team documents of this kind, and what its config says
    /// (see the module docs).
    pub async fn support(&self, env: &Env, runner: &dyn ProcessRunner) -> TeamDocsSupport {
        match self.probed(env, runner).await {
            Ok((_, Answer::Supported(config))) => TeamDocsSupport::Supported(config),
            Ok((_, Answer::Unsupported)) => TeamDocsSupport::Unsupported,
            Err(support) => support,
        }
    }

    /// The installed server's path while it shares this kind with the team, with its config.
    /// `unsupported` while sharing is off or the server has no team documents of this kind,
    /// `notFound` without Harness Memory, `process` when it could not be asked.
    pub async fn sharing_server(&self, env: &Env, runner: &dyn ProcessRunner) -> Result<(PathBuf, TeamDocsConfig)> {
        match self.probed(env, runner).await {
            Ok((path, Answer::Supported(config))) if config.remote => Ok((path, config)),
            Ok((_, Answer::Supported(_))) => Err(BusinessError::Unsupported(format!(
                "sharing {} with the team is off",
                K::PLURAL
            ))),
            Ok((_, Answer::Unsupported)) => Err(BusinessError::Unsupported(format!(
                "this {MEMORY_BINARY} predates remote {}. Update Harness Memory.",
                K::PLURAL
            ))),
            Err(TeamDocsSupport::NotInstalled) => Err(BusinessError::NotFound(format!("executable {MEMORY_BINARY}"))),
            Err(_) => Err(BusinessError::Process("Harness Memory could not be asked".to_owned())),
        }
    }

    async fn probed(
        &self,
        env: &Env,
        runner: &dyn ProcessRunner,
    ) -> std::result::Result<(PathBuf, Answer), TeamDocsSupport> {
        let Some(binary) = resolve_memory(env) else {
            return Err(TeamDocsSupport::NotInstalled);
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

        let answer = ask::<K>(env, runner, &binary.path)
            .await
            .ok_or(TeamDocsSupport::Unknown)?;

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

/// `<plural> config --json` on `server`; `None` when the answer cannot be used (not cached).
async fn ask<K: DocKind>(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Option<Answer> {
    let name = format!("{} config", K::PLURAL);
    let call = CliCall {
        name: &name,
        args: [K::PLURAL, "config", "--json"].map(OsString::from).into(),
        timeout: PROBE_TIMEOUT,
        output_bytes: PROBE_OUTPUT_BYTES,
        stdin: None,
    };

    match run_memory_cli(env, runner, server, call).await {
        Ok(CliAnswer::Done(stdout)) => {
            let config = parse_config::<K>(&stdout);

            if config.is_none() {
                tracing::warn!("harness-memory {name} answered output this app cannot read");
            }

            config.map(Answer::Supported)
        }
        Ok(CliAnswer::Failed { status, message, .. })
            if status == Some(USAGE_EXIT) || names_unknown_command(&message) =>
        {
            Some(Answer::Unsupported)
        }
        Ok(CliAnswer::Failed { status, .. }) => {
            tracing::warn!(?status, "harness-memory {name} failed");
            None
        }
        Err(error) => {
            tracing::warn!(code = ?error.code(), "harness-memory {name} did not run");
            None
        }
    }
}

/// `{"<plural>": "remote"|"local", "<trust key>": bool}`; any other `<plural>` value is local and
/// a missing trust key false.
fn parse_config<K: DocKind>(stdout: &str) -> Option<TeamDocsConfig> {
    let value: Value = serde_json::from_str(stdout).ok()?;
    let target = value.get(K::PLURAL)?.as_str()?;

    Some(TeamDocsConfig {
        remote: target == "remote",
        trust: value.get(K::TRUST_KEY).and_then(Value::as_bool).unwrap_or(false),
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
