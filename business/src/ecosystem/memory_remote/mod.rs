//! The shared team database of Harness Memory, set in the server's own config file
//! (`<config dir>/harness-memory/config.toml`, see [`memory_config_path`]).
//!
//! - [`memory_remote_get`]: what the file holds, with the URL redacted and the token reduced to
//!   whether one is saved.
//! - [`memory_remote_set`]: saves or removes the URL, the token, the note and script routing,
//!   `trust_team_scripts`, and the team hooks keys (`hooks = "remote"`, `trust_team_hooks`).
//! - [`memory_remote_test`]: runs `harness-memory check` against the saved or the given database.
//!
//! The app itself never connects to the database. The URL and the token are secrets: they are
//! never logged, never returned to the webview, and reach the server only through its environment.

mod check;
mod file;
mod url;

use std::path::PathBuf;

pub use check::{CHECK_TIMEOUT, memory_remote_test};
pub use url::{MAX_SECRET_BYTES, kind_of, redact_url, validate_token, validate_url};

use self::file::{ConfigFile, Routing, read_config, remove_config, render, write_config};
use self::url::kind_name;
use super::memory_hooks::HooksSupport;
use crate::env::{Env, OsKind};
use crate::error::{BusinessError, Result};
use crate::model::{MemoryNotesTarget, MemoryRemote, MemoryRemoteKind};
use crate::pi_family::tilde;

/// The config file inside the config folder.
const CONFIG_FILE: [&str; 2] = ["harness-memory", "config.toml"];

/// Shown for the path on Windows while `APPDATA` is unset.
const WINDOWS_PATH_HINT: &str = "%APPDATA%\\harness-memory\\config.toml";

/// A change of the team database settings (`memory_remote_set`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRemoteChange {
    /// False removes the team database settings (the file, since it holds nothing else).
    pub enabled: bool,
    /// The kind of database; required when `enabled`.
    pub kind: Option<MemoryRemoteKind>,
    /// The URL; `None` keeps the saved one (only when its scheme is of `kind`).
    pub url: Option<String>,
    /// The libSQL token; `None` keeps the saved one, an empty string removes it. Not saved for
    /// PostgreSQL and MongoDB, which take their credentials in the URL.
    pub token: Option<String>,
    /// Where repository notes go.
    pub repository_notes: MemoryNotesTarget,
    /// Where global notes go.
    pub global_notes: MemoryNotesTarget,
    /// Where scripts go. `Local` is saved only while the installed server supports scripts.
    pub scripts: MemoryNotesTarget,
    /// Run every script from the team database without asking. Saved (as `true`) only while the
    /// installed server asks before running a script ([`ScriptsSupport::Approvals`]) and scripts go
    /// to the team database.
    pub trust_team_scripts: bool,
    /// Share hooks with the team (`hooks = "remote"`); `None` keeps the saved value. Saved only
    /// while the installed server keeps team hooks ([`HooksSupport::Supported`]).
    pub share_hooks: Option<bool>,
    /// Run team hooks without local approval (`trust_team_hooks = true`); `None` keeps the saved
    /// value. Saved only while the server keeps team hooks and they are shared.
    pub trust_team_hooks: Option<bool>,
}

/// What the installed server supports, which decides the keys a save may write: the script keys
/// ([`ScriptsSupport`]) and the team hooks keys ([`HooksSupport`]). From a [`ScriptsSupport`]
/// alone, the hooks are [`HooksSupport::Unknown`] (their saved keys are kept).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySupport {
    /// Scripts and their approvals.
    pub scripts: ScriptsSupport,
    /// Team hooks.
    pub hooks: HooksSupport,
}

impl From<ScriptsSupport> for MemorySupport {
    fn from(scripts: ScriptsSupport) -> Self {
        Self {
            scripts,
            hooks: HooksSupport::Unknown,
        }
    }
}

/// Whether the installed server keeps scripts and asks the user before running one, which decides
/// what happens to the `scripts` and `trust_team_scripts` keys of the config file on a save (see
/// [`MemoryScripts::support`](super::MemoryScripts::support)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptsSupport {
    /// It asks the user before running a script it has not approved (its `script_run` takes no
    /// `sha`): both keys are written when wanted.
    Approvals,
    /// It offers `script_list` but predates approvals: `scripts = "local"` is written when
    /// scripts stay local, `trust_team_scripts` is dropped (the server refuses it).
    Supported,
    /// It predates scripts and refuses both keys: they are dropped.
    Unsupported,
    /// The server could not be asked (not found, or the probe failed): both saved keys are kept as
    /// they are, so a passing failure neither sends local scripts to the team database nor
    /// changes which scripts run without asking.
    Unknown,
}

impl ScriptsSupport {
    /// Whether the `scripts` key may be written.
    pub fn keeps_scripts(self) -> bool {
        matches!(self, Self::Approvals | Self::Supported)
    }

    /// Whether the `trust_team_scripts` key may be written.
    pub fn asks_before_running(self) -> bool {
        self == Self::Approvals
    }
}

/// `HARNESS_MEMORY_CONFIG`: the server's config file instead of the default one (absolute, a
/// leading `~` expanded; the server refuses a relative value).
pub const CONFIG_VAR: &str = "HARNESS_MEMORY_CONFIG";

/// Harness Memory's config file, found as the server finds it: [`CONFIG_VAR`] when set, else under
/// `$XDG_CONFIG_HOME` (or `~/.config`) on Linux, `~/Library/Application Support` on macOS and
/// `%APPDATA%` on Windows. A relative variable does not count, as for the server. `None` while
/// [`CONFIG_VAR`] is relative (the server then refuses to start), and on Windows while `APPDATA` is
/// unset.
pub fn memory_config_path(env: &Env) -> Option<PathBuf> {
    if let Some(raw) = env.var(CONFIG_VAR) {
        let path = env.expand_home(raw);
        return path.has_root().then_some(path);
    }

    let rooted = |name: &str| env.var(name).map(PathBuf::from).filter(|dir| dir.has_root());

    let dir = match env.os {
        OsKind::Linux => rooted("XDG_CONFIG_HOME").unwrap_or_else(|| env.home_join(".config")),
        OsKind::MacOs => env.home_join("Library/Application Support"),
        OsKind::Windows => rooted("APPDATA")?,
    };

    Some(CONFIG_FILE.iter().fold(dir, |path, part| path.join(part)))
}

/// The team database settings in the config file. Reads the file only (see [`file`]).
/// `scriptsSupported` and `trustTeamScriptsSupported` follow `support` (see
/// [`ScriptsSupport::keeps_scripts`] and [`ScriptsSupport::asks_before_running`]), `hooksSupported`
/// [`HooksSupport::keeps_hooks`]; the saved values are reported either way.
pub fn memory_remote_get(env: &Env, support: impl Into<MemorySupport>) -> MemoryRemote {
    let support = support.into();

    let Some(path) = memory_config_path(env) else {
        let hint = match env.var(CONFIG_VAR) {
            Some(raw) => raw.to_owned(),
            None => WINDOWS_PATH_HINT.to_owned(),
        };

        return MemoryRemote {
            problem: Some(unknown_path(env)),
            ..remote_from(&ConfigFile::default(), hint, support)
        };
    };

    remote_from(&read_config(&path), tilde(env, &path), support)
}

/// Why [`memory_config_path`] found no file.
fn unknown_path(env: &Env) -> String {
    if env.var(CONFIG_VAR).is_some() {
        "HARNESS_MEMORY_CONFIG is not an absolute path, so Harness Memory refuses it".to_owned()
    } else {
        "APPDATA is not set, so the config folder is unknown".to_owned()
    }
}

fn remote_from(config: &ConfigFile, config_path: String, support: MemorySupport) -> MemoryRemote {
    let url = config.url.as_deref().filter(|url| !url.trim().is_empty());

    MemoryRemote {
        enabled: url.is_some(),
        kind: url.and_then(kind_of),
        url_redacted: url.map(redact_url),
        has_token: config.token.is_some(),
        repository_notes: config.repository_notes,
        global_notes: config.global_notes,
        scripts: config.scripts.unwrap_or_default(),
        scripts_supported: support.scripts.keeps_scripts(),
        trust_team_scripts: config.trust_team_scripts == Some(true),
        trust_team_scripts_supported: support.scripts.asks_before_running(),
        hooks_supported: support.hooks.keeps_hooks(),
        share_hooks: config.hooks == Some(MemoryNotesTarget::Remote),
        trust_team_hooks: config.trust_team_hooks == Some(true),
        config_path,
        problem: config
            .problem
            .clone()
            .or_else(|| url.filter(|url| kind_of(url).is_none()).map(|_| unsupported_scheme())),
    }
}

fn unsupported_scheme() -> String {
    "the saved URL does not start with a scheme Harness Memory supports".to_owned()
}

/// Applies `change` to the config file and returns the settings saved. Refuses (`InvalidInput`) a
/// file the app could not read or parse, one with keys Harness Memory does not know, and a
/// symbolic link; the URL and the token are validated as in [`validate_url`] and
/// [`validate_token`]. Turning the team database off removes the file. The `scripts` and
/// `trust_team_scripts` keys follow `support.scripts` (see [`ScriptsSupport`]), the `hooks` and
/// `trust_team_hooks` keys `support.hooks` (see [`hooks_key`]).
pub fn memory_remote_set(
    env: &Env,
    change: MemoryRemoteChange,
    support: impl Into<MemorySupport>,
) -> Result<MemoryRemote> {
    let support = support.into();
    let path = memory_config_path(env).ok_or_else(|| BusinessError::Unsupported(unknown_path(env)))?;
    let saved = read_config(&path);

    if let Some(reason) = &saved.blocked {
        return Err(BusinessError::InvalidInput(format!(
            "{} is not changed: {reason}",
            tilde(env, &path)
        )));
    }

    if !change.enabled {
        remove_config(&path)?;
        tracing::info!("team memory database removed from the Harness Memory config");

        return Ok(memory_remote_get(env, support));
    }

    let kind = change
        .kind
        .ok_or_else(|| BusinessError::InvalidInput("pick the kind of team database".to_owned()))?;

    let url = match change.url.as_deref() {
        Some(raw) => validate_url(raw, kind)?,
        None => kept_url(saved.url.as_deref(), kind)?,
    };

    let token = match (kind, change.token.as_deref()) {
        (MemoryRemoteKind::Libsql, Some(raw)) => validate_token(raw)?,
        (MemoryRemoteKind::Libsql, None) => saved.token,
        _ => None,
    };

    let hooks = hooks_key(change.share_hooks, saved.hooks, support.hooks);

    let routing = Routing {
        repository_notes: change.repository_notes,
        global_notes: change.global_notes,
        scripts: scripts_key(change.scripts, saved.scripts, support.scripts),
        trust_team_scripts: trust_key(
            change.trust_team_scripts,
            change.scripts,
            saved.trust_team_scripts,
            support.scripts,
        ),
        hooks,
        trust_team_hooks: trust_hooks_key(change.trust_team_hooks, hooks, saved.trust_team_hooks, support.hooks),
    };

    let text = render(&url, token.as_deref(), routing);
    write_config(&path, &text)?;
    tracing::info!(
        kind = kind_name(kind),
        "team memory database saved to the Harness Memory config"
    );

    Ok(memory_remote_get(env, support))
}

/// The `scripts` value to write (`None` for no key): `local` when scripts stay local on a server
/// that keeps them (unset means remote), nothing for a server that predates them, and the saved
/// value, untouched, when the server could not be asked.
fn scripts_key(
    wanted: MemoryNotesTarget,
    saved: Option<MemoryNotesTarget>,
    support: ScriptsSupport,
) -> Option<MemoryNotesTarget> {
    match support {
        ScriptsSupport::Approvals | ScriptsSupport::Supported => {
            (wanted == MemoryNotesTarget::Local).then_some(MemoryNotesTarget::Local)
        }
        ScriptsSupport::Unsupported => None,
        ScriptsSupport::Unknown => saved,
    }
}

/// The `trust_team_scripts` value to write (`None` for no key): `true` when wanted on a server that
/// asks before running a script while scripts go to the team database (unset means false; with
/// scripts kept local there are no team scripts to trust, so a later switch back to the team does
/// not trust them unasked), nothing for an older one, and the saved value, untouched, when the
/// server could not be asked.
fn trust_key(wanted: bool, scripts: MemoryNotesTarget, saved: Option<bool>, support: ScriptsSupport) -> Option<bool> {
    match support {
        ScriptsSupport::Approvals => (wanted && scripts == MemoryNotesTarget::Remote).then_some(true),
        ScriptsSupport::Supported | ScriptsSupport::Unsupported => None,
        ScriptsSupport::Unknown => saved,
    }
}

/// The `hooks` value to write (`None` for no key): `remote` while hooks are shared on a server
/// that keeps team hooks (unset means local), nothing for a server that predates them (it refuses
/// the key), and the saved value, untouched, when the server could not be asked. `wanted` `None`
/// keeps the saved choice.
fn hooks_key(
    wanted: Option<bool>,
    saved: Option<MemoryNotesTarget>,
    support: HooksSupport,
) -> Option<MemoryNotesTarget> {
    match support {
        HooksSupport::Supported(_) => {
            let share = wanted.unwrap_or(saved == Some(MemoryNotesTarget::Remote));
            share.then_some(MemoryNotesTarget::Remote)
        }
        HooksSupport::Unsupported => None,
        HooksSupport::NotInstalled | HooksSupport::Unknown => saved,
    }
}

/// The `trust_team_hooks` value to write (`None` for no key): `true` when wanted on a server that
/// keeps team hooks while they are shared (`hooks`; unset means false, and without sharing there
/// are no team hooks to trust), nothing for an older server, and the saved value, untouched, when
/// the server could not be asked. `wanted` `None` keeps the saved choice.
fn trust_hooks_key(
    wanted: Option<bool>,
    hooks: Option<MemoryNotesTarget>,
    saved: Option<bool>,
    support: HooksSupport,
) -> Option<bool> {
    match support {
        HooksSupport::Supported(_) => {
            let trust = wanted.unwrap_or(saved == Some(true));
            (trust && hooks == Some(MemoryNotesTarget::Remote)).then_some(true)
        }
        HooksSupport::Unsupported => None,
        HooksSupport::NotInstalled | HooksSupport::Unknown => saved,
    }
}

/// The saved URL, re-validated, when it is one of `kind`.
fn kept_url(saved: Option<&str>, kind: MemoryRemoteKind) -> Result<String> {
    let Some(saved) = saved.filter(|url| !url.trim().is_empty()) else {
        return Err(BusinessError::InvalidInput("enter the database URL".to_owned()));
    };

    if kind_of(saved) != Some(kind) {
        return Err(BusinessError::InvalidInput(format!(
            "the saved URL is not a {} URL; enter the new one",
            kind_name(kind)
        )));
    }

    validate_url(saved, kind).map_err(|error| match error {
        BusinessError::InvalidInput(reason) => {
            BusinessError::InvalidInput(format!("the saved URL cannot be kept ({reason}); enter it again"))
        }
        other => other,
    })
}
