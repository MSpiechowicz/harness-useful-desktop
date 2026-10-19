//! `harness-memory check`: whether the installed server reaches a team database.
//!
//! The server reads the URL and the token from `HARNESS_MEMORY_DATABASE_URL` and
//! `HARNESS_MEMORY_DATABASE_TOKEN` before its config file, so a URL or token typed in the UI is
//! tested through the environment (never the command line); without one the saved value is
//! used. The app's own inherited values of those variables, `HARNESS_MEMORY_DATABASE_KIND` and
//! `HARNESS_MEMORY_CONFIG`, are not passed on, so the test sees the same file the settings show.
//!
//! The server takes the kind of a URL from the environment from `HARNESS_MEMORY_DATABASE_KIND` only
//! (unset picks Amazon DocumentDB by the host), so an Amazon DocumentDB URL is tested with
//! `HARNESS_MEMORY_DATABASE_KIND=documentdb`, a MongoDB one at a DocumentDB-looking host with
//! `mongodb`, and any other without it. The saved URL is tested with the saved `database_kind`, so
//! it must be of the kind asked for, except that a saved MongoDB URL is tested as Amazon DocumentDB
//! (and back) through the environment, unless the server would refuse the file (then it is left to
//! the file, and so needs retyping).

use std::time::Duration;

use super::file::{ConfigFile, read_config};
use super::gates::documentdb_unavailable;
use super::memory_config_path;
use super::url::{a_kind, kind_key, saved_kind, scrub, secrets_of, validate_token, validate_url};
use crate::ecosystem::memory_binary::installed_memory;
use crate::ecosystem::memory_kinds::KindsSupport;
use crate::env::Env;
use crate::error::BusinessError;
use crate::model::{MemoryRemoteKind, MemoryRemoteTest};
use crate::process::{ExecSpec, ProcessRunner, launcher_search_dirs};
use crate::security::redact_text;

/// How long `harness-memory check` may take (20 s).
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(20);

/// The output kept of each stream (16 KiB; the answer is one line).
const CHECK_OUTPUT_BYTES: usize = 16 * 1024;

/// The longest message returned, in characters.
const MAX_MESSAGE_CHARS: usize = 500;

const URL_VAR: &str = "HARNESS_MEMORY_DATABASE_URL";
const TOKEN_VAR: &str = "HARNESS_MEMORY_DATABASE_TOKEN";
const KIND_VAR: &str = "HARNESS_MEMORY_DATABASE_KIND";

/// Runs `harness-memory check` against a database of `kind`: `url` and `token` when given (an empty
/// `token` tests without one), else the saved ones. `ok` is the server's exit status; `message` its
/// first line (standard output on success, standard error on failure) with the typed and the saved
/// secrets masked and passed through [`redact_text`], or why it could not run. Amazon DocumentDB is
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
        && let Some(problem) = saved_url_problem(&saved, kind)
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

    let mut spec = ExecSpec::new(&server.path)
        .args(["check"])
        .env_remove_prefix(URL_VAR)
        .env_remove_prefix(TOKEN_VAR)
        .env_remove_prefix(KIND_VAR)
        .env_remove_prefix("HARNESS_MEMORY_CONFIG")
        .search_dirs(launcher_search_dirs(env, &server.path))
        .timeout(CHECK_TIMEOUT)
        .max_output_bytes(CHECK_OUTPUT_BYTES);

    if let Some(url) = &url {
        spec = spec.env(URL_VAR, url);

        if let Some(name) = kind_key(kind, url) {
            spec = spec.env(KIND_VAR, name);
        }
    }

    // The server treats an empty variable as unset (and would read the saved token), but trims a
    // set one: a single space tests without a token.
    if let Some(token) = &token {
        spec = spec.env(TOKEN_VAR, token.as_deref().unwrap_or(" "));
    }

    // The server may quote the saved values as well as the typed ones.
    let mut secrets = secrets_of(url.as_deref(), token.flatten().as_deref());
    secrets.extend(secrets_of(saved.url.as_deref(), saved.token.as_deref()));
    secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));

    let output = match runner.run(&spec).await {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!(%error, "harness-memory check could not be started");
            return failed("Harness Memory could not be started".to_owned());
        }
    };

    if output.timed_out {
        return failed(format!(
            "Harness Memory did not answer within {} seconds",
            CHECK_TIMEOUT.as_secs()
        ));
    }

    let ok = output.success();
    let (first, second) = if ok {
        (&output.stdout, &output.stderr)
    } else {
        (&output.stderr, &output.stdout)
    };

    let message = match first_line(first).or_else(|| first_line(second)) {
        Some(line) => clean(line, &secrets),
        None => silent_answer(ok, output.status),
    };

    MemoryRemoteTest { ok, message }
}

/// The saved URL, validated for `kind`, when it is a MongoDB one saved as the other of MongoDB and
/// Amazon DocumentDB in a file Harness Memory would use.
fn switched_url(saved: &ConfigFile, kind: MemoryRemoteKind) -> Option<String> {
    if saved.refused {
        return None;
    }

    let url = saved.url.as_deref()?;

    if saved_kind(url, saved.database_kind.as_deref()) == Some(kind) {
        return None;
    }

    validate_url(url, kind).ok()
}

/// Why the saved URL cannot be tested as `kind`, if it cannot.
fn saved_url_problem(saved: &ConfigFile, kind: MemoryRemoteKind) -> Option<String> {
    match saved.url.as_deref().filter(|url| !url.trim().is_empty()) {
        None => Some("no database URL is saved; enter one to test it".to_owned()),
        Some(url) if saved_kind(url, saved.database_kind.as_deref()) != Some(kind) => Some(format!(
            "the saved URL is not {} URL, so enter one to test it",
            a_kind(kind)
        )),
        Some(_) => None,
    }
}

/// The message when the server printed nothing.
fn silent_answer(ok: bool, status: Option<i32>) -> String {
    match (ok, status) {
        (true, _) => "Harness Memory reached the database".to_owned(),
        (false, Some(code)) => format!("harness-memory check failed (exit code {code})"),
        (false, None) => "harness-memory check was stopped".to_owned(),
    }
}

fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

/// `line` with `secrets` masked, through [`redact_text`], at most [`MAX_MESSAGE_CHARS`] long.
fn clean(line: &str, secrets: &[String]) -> String {
    let redacted = redact_text(&scrub(line, secrets));

    match redacted.char_indices().nth(MAX_MESSAGE_CHARS) {
        Some((end, _)) => format!("{}…", &redacted[..end]),
        None => redacted,
    }
}

fn reason(error: BusinessError) -> String {
    match error {
        BusinessError::InvalidInput(reason) => reason,
        other => other.to_string(),
    }
}

fn failed(message: String) -> MemoryRemoteTest {
    MemoryRemoteTest { ok: false, message }
}
