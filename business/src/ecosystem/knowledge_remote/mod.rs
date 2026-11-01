//! The shared team database of Harness Knowledge, set in the server's own config file
//! (`<config dir>/harness-knowledge/config.toml`, see [`knowledge_config_path`]).
//!
//! - [`knowledge_remote_get`]: what the file holds, with the URL redacted and the token reduced to
//!   whether one is saved.
//! - [`knowledge_remote_set`]: saves or removes the `[database]` table with the URL and, for libSQL,
//!   the token.
//! - [`knowledge_remote_test`]: runs `harness-knowledge check --json` against the saved or the
//!   given database and says what it lacks ([`check`]).
//!
//! The kinds are PostgreSQL, MongoDB and libSQL: Amazon DocumentDB is not supported (the server
//! refuses its hosts), so it is never saved or tested, and a saved URL at a DocumentDB host is
//! reported as the problem it is. The app itself never connects to the database. The URL and the
//! token are secrets: they are never logged, never returned to the webview, and reach the server
//! only through its environment.

mod check;
mod file;

use std::path::PathBuf;

pub use check::{knowledge_remote_test, read_check};

use self::file::{ConfigFile, Routes, parse_routing, read_config, remove_config, render, write_config};
use super::catalog::KNOWLEDGE_SIDECAR;
use super::knowledge_features::KnowledgeSupport;
use super::memory_remote::is_documentdb_url;
use super::sidecar_config::{config_path, path_hint, refuse_blocked, unknown_path};
use super::sidecar_remote_url::{kind_name, kind_of, redact_url, url_and_token};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{KnowledgeRemote, MemoryNotesRouting, MemoryRemoteKind};
use crate::pi_family::tilde;

/// A change of the team database settings (`knowledge_remote_set`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnowledgeRemoteChange {
    /// False removes the team database settings (the file, since it holds nothing else).
    pub enabled: bool,
    /// The kind of database; required when `enabled`. Amazon DocumentDB is refused.
    pub kind: Option<MemoryRemoteKind>,
    /// The URL; `None` keeps the saved one (only when its scheme is of `kind`).
    pub url: Option<String>,
    /// The libSQL token; `None` keeps the saved one, an empty string removes it. Not saved for
    /// PostgreSQL and MongoDB, which take their credentials in the URL.
    pub token: Option<String>,
    /// Where project documents go (`[routing] repository_documents`); `None` keeps the saved one.
    pub repository_documents: Option<MemoryNotesRouting>,
    /// Where global documents go (`[routing] global_documents`); `None` keeps the saved one.
    pub global_documents: Option<MemoryNotesRouting>,
}

/// The variable that sets where project documents go, over the config file.
const REPOSITORY_VAR: &str = "HARNESS_KNOWLEDGE_REPOSITORY_DOCUMENTS";

/// The variable that sets where global documents go, over the config file.
const GLOBAL_VAR: &str = "HARNESS_KNOWLEDGE_GLOBAL_DOCUMENTS";

/// Harness Knowledge's config file, found as the server finds it: `HARNESS_KNOWLEDGE_CONFIG` when
/// set, else under `$XDG_CONFIG_HOME` (or `~/.config`) on Linux, `~/Library/Application Support` on
/// macOS and `%APPDATA%` on Windows. A relative variable does not count, as for the server.
pub fn knowledge_config_path(env: &Env) -> Option<PathBuf> {
    config_path(&KNOWLEDGE_SIDECAR, env)
}

/// Why `kind` cannot be the team database, if it cannot (Amazon DocumentDB).
pub(crate) fn unsupported_kind(kind: MemoryRemoteKind) -> Option<String> {
    (kind == MemoryRemoteKind::Documentdb).then(|| {
        format!(
            "{} does not support Amazon DocumentDB yet, so use MongoDB (Atlas, or 8.2 with mongot), PostgreSQL with \
             pgvector or libSQL",
            KNOWLEDGE_SIDECAR.product
        )
    })
}

/// The team database settings in the config file. Reads the file only (see [`file`]).
/// `embedderSupported` follows `support` ([`KnowledgeSupport::embedder`]); the saved values are
/// reported either way.
pub fn knowledge_remote_get(env: &Env, support: KnowledgeSupport) -> KnowledgeRemote {
    let Some(path) = knowledge_config_path(env) else {
        return KnowledgeRemote {
            problem: Some(unknown_path(&KNOWLEDGE_SIDECAR, env)),
            ..remote_from(env, &ConfigFile::default(), path_hint(&KNOWLEDGE_SIDECAR, env), support)
        };
    };

    remote_from(env, &read_config(&path), tilde(env, &path), support)
}

/// Where documents go for the variable `var` when it is set (its value as the server reads it, the
/// file's `saved` when the value is not one), and whether it is set.
fn routing_from_env(env: &Env, var: &str, saved: MemoryNotesRouting) -> (MemoryNotesRouting, bool) {
    match env.var(var) {
        Some(raw) => (parse_routing(raw).unwrap_or(saved), true),
        None => (saved, false),
    }
}

fn remote_from(env: &Env, config: &ConfigFile, config_path: String, support: KnowledgeSupport) -> KnowledgeRemote {
    let (repository_documents, repository_documents_env) =
        routing_from_env(env, REPOSITORY_VAR, config.repository_documents);
    let (global_documents, global_documents_env) = routing_from_env(env, GLOBAL_VAR, config.global_documents);

    let url = config.url.as_deref().filter(|url| !url.trim().is_empty());
    let kind = url.and_then(kind_of);
    let documentdb = url.is_some_and(|url| kind == Some(MemoryRemoteKind::Mongodb) && is_documentdb_url(url));

    let bad_variable = [REPOSITORY_VAR, GLOBAL_VAR]
        .into_iter()
        .find(|var| env.var(var).is_some_and(|raw| parse_routing(raw).is_none()));

    let problem = config.status.problem.clone().or_else(|| match url {
        _ if documentdb => unsupported_kind(MemoryRemoteKind::Documentdb),
        _ if bad_variable.is_some() => Some(format!(
            "{} is not \"remote\", \"local\" or \"remote+local\", so {} refuses to start",
            bad_variable.unwrap_or_default(),
            KNOWLEDGE_SIDECAR.product
        )),
        Some(_) if kind.is_none() => Some(format!(
            "the saved URL does not start with a scheme {} supports",
            KNOWLEDGE_SIDECAR.product
        )),
        _ => None,
    });

    KnowledgeRemote {
        enabled: url.is_some(),
        kind,
        url_redacted: url.map(redact_url),
        has_token: config.token.is_some(),
        embedder_supported: support.embedder(),
        repository_documents,
        global_documents,
        repository_documents_env,
        global_documents_env,
        routing_supported: support.routing(),
        config_path,
        problem,
    }
}

/// Applies `change` to the config file and returns the settings saved. Refuses (`InvalidInput`) a
/// file the app could not read or parse, one with keys Harness Knowledge does not know, and a
/// symbolic link; the URL and the token are validated as in [`validate_url`] and
/// [`validate_token`]. Turning the team database off removes the file. Amazon DocumentDB, and
/// MongoDB at a DocumentDB host, are refused (`Unsupported`).
pub fn knowledge_remote_set(
    env: &Env,
    change: KnowledgeRemoteChange,
    support: KnowledgeSupport,
) -> Result<KnowledgeRemote> {
    let path =
        knowledge_config_path(env).ok_or_else(|| BusinessError::Unsupported(unknown_path(&KNOWLEDGE_SIDECAR, env)))?;
    let saved = read_config(&path);

    refuse_blocked(env, &path, &saved.status)?;

    if !change.enabled {
        remove_config(&path)?;
        tracing::info!("team knowledge database removed from the Harness Knowledge config");

        return Ok(knowledge_remote_get(env, support));
    }

    let kind = change
        .kind
        .ok_or_else(|| BusinessError::InvalidInput("pick the kind of remote database".to_owned()))?;

    if let Some(reason) = unsupported_kind(kind) {
        return Err(BusinessError::Unsupported(reason));
    }

    let (url, token) = url_and_token(
        kind,
        change.url.as_deref(),
        saved.url.as_deref(),
        change.token.as_deref(),
        saved.token,
    )?;

    if kind == MemoryRemoteKind::Mongodb && is_documentdb_url(&url) {
        return Err(BusinessError::Unsupported(
            unsupported_kind(MemoryRemoteKind::Documentdb).unwrap_or_default(),
        ));
    }

    // A variable wins over the file, so a value chosen for its place is not written.
    let kept = |var: &str, chosen: Option<MemoryNotesRouting>, saved: MemoryNotesRouting| {
        if env.var(var).is_some() {
            saved
        } else {
            chosen.unwrap_or(saved)
        }
    };

    let routes = Routes {
        repository: kept(REPOSITORY_VAR, change.repository_documents, saved.repository_documents),
        global: kept(GLOBAL_VAR, change.global_documents, saved.global_documents),
    };

    // A server that predates routing refuses the table, so the app never writes one for it.
    if !support.routing()
        && (routes.repository != saved.repository_documents || routes.global != saved.global_documents)
    {
        return Err(BusinessError::Unsupported(format!(
            "this {} cannot route documents yet, so update {}",
            KNOWLEDGE_SIDECAR.binary, KNOWLEDGE_SIDECAR.product
        )));
    }

    write_config(&path, &render(&url, token.as_deref(), routes))?;
    tracing::info!(
        kind = kind_name(kind),
        "team knowledge database saved to the Harness Knowledge config"
    );

    Ok(knowledge_remote_get(env, support))
}
