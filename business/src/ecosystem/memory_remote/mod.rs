//! The shared team database of Harness Memory, set in the server's own config file
//! (`<config dir>/harness-memory/config.toml`, see [`memory_config_path`]).
//!
//! - [`memory_remote_get`]: what the file holds, with the URL redacted and the token reduced to
//!   whether one is saved.
//! - [`memory_remote_set`]: saves or removes the URL, the token and the note routing.
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

use self::file::{ConfigFile, read_config, remove_config, render, write_config};
use self::url::kind_name;
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
}

/// Harness Memory's config file: under `$XDG_CONFIG_HOME` (or `~/.config`) on Linux,
/// `~/Library/Application Support` on macOS and `%APPDATA%` on Windows. A relative variable does
/// not count, as for the server. `None` on Windows while `APPDATA` is unset.
pub fn memory_config_path(env: &Env) -> Option<PathBuf> {
    let rooted = |name: &str| env.var(name).map(PathBuf::from).filter(|dir| dir.has_root());

    let dir = match env.os {
        OsKind::Linux => rooted("XDG_CONFIG_HOME").unwrap_or_else(|| env.home_join(".config")),
        OsKind::MacOs => env.home_join("Library/Application Support"),
        OsKind::Windows => rooted("APPDATA")?,
    };

    Some(CONFIG_FILE.iter().fold(dir, |path, part| path.join(part)))
}

/// The team database settings in the config file. Reads the file only (see [`file`]).
pub fn memory_remote_get(env: &Env) -> MemoryRemote {
    let Some(path) = memory_config_path(env) else {
        return MemoryRemote {
            problem: Some("APPDATA is not set, so the config folder is unknown".to_owned()),
            ..remote_from(&ConfigFile::default(), WINDOWS_PATH_HINT.to_owned())
        };
    };

    remote_from(&read_config(&path), tilde(env, &path))
}

fn remote_from(config: &ConfigFile, config_path: String) -> MemoryRemote {
    let url = config.url.as_deref().filter(|url| !url.trim().is_empty());

    MemoryRemote {
        enabled: url.is_some(),
        kind: url.and_then(kind_of),
        url_redacted: url.map(redact_url),
        has_token: config.token.is_some(),
        repository_notes: config.repository_notes,
        global_notes: config.global_notes,
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
/// [`validate_token`]. Turning the team database off removes the file.
pub fn memory_remote_set(env: &Env, change: MemoryRemoteChange) -> Result<MemoryRemote> {
    let path = memory_config_path(env)
        .ok_or_else(|| BusinessError::Unsupported("APPDATA is not set, so the config folder is unknown".to_owned()))?;
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

        return Ok(memory_remote_get(env));
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

    let text = render(&url, token.as_deref(), change.repository_notes, change.global_notes);
    write_config(&path, &text)?;
    tracing::info!(
        kind = kind_name(kind),
        "team memory database saved to the Harness Memory config"
    );

    Ok(memory_remote_get(env))
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
