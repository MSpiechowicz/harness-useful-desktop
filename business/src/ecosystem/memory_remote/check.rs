//! `harness-memory check`: whether the installed server reaches a team database.
//!
//! Running it, and what it is given through the environment, is
//! [`sidecar_remote_check`](crate::ecosystem::sidecar_remote_check)'s. This file decides what is
//! tested, as the memory server does: the server takes the kind of a URL from the environment
//! from `HARNESS_MEMORY_DATABASE_KIND` only (unset picks Amazon DocumentDB by the host), so an
//! Amazon DocumentDB URL is tested with `HARNESS_MEMORY_DATABASE_KIND=documentdb`, a MongoDB one at
//! a DocumentDB-looking host with `mongodb`, and any other without it. The saved URL is tested with
//! the saved `database_kind`, so it must be of the kind asked for, except that a saved MongoDB URL
//! is tested as Amazon DocumentDB (and back) through the environment, unless the server would
//! refuse the file (then it is left to the file, and so needs retyping).

use super::file::{ConfigFile, read_config};
use super::gates::documentdb_unavailable;
use super::memory_config_path;
use super::url::{kind_key, saved_kind};
use crate::ecosystem::catalog::MEMORY_SIDECAR;
use crate::ecosystem::memory_binary::installed_memory;
use crate::ecosystem::memory_kinds::KindsSupport;
use crate::ecosystem::sidecar_remote_check::{CheckRequest, failed, run_check};
use crate::ecosystem::sidecar_remote_url::{check_secrets, reason, saved_url_problem, validate_token, validate_url};
use crate::env::Env;
use crate::model::{MemoryRemoteKind, MemoryRemoteTest};
use crate::process::ProcessRunner;

pub use crate::ecosystem::sidecar_remote_check::CHECK_TIMEOUT;

/// Runs `harness-memory check` against a database of `kind`: `url` and `token` when given (an empty
/// `token` tests without one), else the saved ones. `ok` is the server's exit status; `message` its
/// first line (standard output on success, standard error on failure) with the typed and the saved
/// secrets masked and passed through [`redact_text`](crate::security::redact_text), or why it could not run. Amazon DocumentDB is
/// not tested unless `kinds` lists it (`kinds` is not read for other kinds).
pub async fn memory_remote_test(
    env: &Env,
    runner: &dyn ProcessRunner,
    kind: MemoryRemoteKind,
    kinds: KindsSupport,
    url: Option<String>,
    token: Option<String>,
) -> MemoryRemoteTest {
    if kind == MemoryRemoteKind::Documentdb
        && let Some(reason) = documentdb_unavailable(kinds)
    {
        return failed(reason);
    }

    let url = match url.as_deref().map(|raw| validate_url(raw, kind)).transpose() {
        Ok(url) => url,
        Err(error) => return failed(reason(error)),
    };

    let saved = memory_config_path(env)
        .map(|path| read_config(&path))
        .unwrap_or_default();

    // MongoDB and Amazon DocumentDB share the URL schemes: a saved URL of the other of the two is
    // tested as the kind asked for (through the environment), so switching needs no retyping. Not
    // from a file the server refuses: that would test settings it will not use.
    let url = url.or_else(|| switched_url(&saved, kind));

    if url.is_none()
        && let Some(problem) = saved_url_problem(
            saved.url.as_deref(),
            |url| saved_kind(url, saved.database_kind.as_deref()) == Some(kind),
            kind,
        )
    {
        return failed(problem);
    }

    // `Some(None)`: test without a token; `None`: use the saved one (and never for other kinds).
    let token = match (kind, token.as_deref()) {
        (MemoryRemoteKind::Libsql, Some(raw)) => match validate_token(raw) {
            Ok(token) => Some(token),
            Err(error) => return failed(reason(error)),
        },
        _ => None,
    };

    let Some(server) = installed_memory(env, runner).await else {
        return failed("Harness Memory is not installed".to_owned());
    };

    // The server may quote the saved values as well as the typed ones.
    let secrets = check_secrets(
        url.as_deref(),
        token.as_ref(),
        saved.url.as_deref(),
        saved.token.as_deref(),
    );

    let request = CheckRequest {
        url: url.as_deref(),
        database_kind: url.as_deref().and_then(|url| kind_key(kind, url)),
        token: token.as_ref().map(Option::as_deref),
        secrets: &secrets,
        json: false,
    };

    run_check(&MEMORY_SIDECAR, env, runner, &server.path, request).await
}

/// The saved URL, validated for `kind`, when it is a MongoDB one saved as the other of MongoDB and
/// Amazon DocumentDB in a file Harness Memory would use.
fn switched_url(saved: &ConfigFile, kind: MemoryRemoteKind) -> Option<String> {
    if saved.status.refused {
        return None;
    }

    let url = saved.url.as_deref()?;

    if saved_kind(url, saved.database_kind.as_deref()) == Some(kind) {
        return None;
    }

    validate_url(url, kind).ok()
}
