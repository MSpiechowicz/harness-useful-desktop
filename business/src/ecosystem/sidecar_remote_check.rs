//! `<binary> check` of a Harness family server: whether the installed server reaches a team
//! database ([`run_check`]). [`memory_remote`](super::memory_remote) decides what to test (the
//! saved or the typed database, the kinds the server can use) and passes it here.
//!
//! The server reads the URL and the token from `<prefix>_DATABASE_URL` and
//! `<prefix>_DATABASE_TOKEN` before its config file, so a URL or token typed in the UI is tested
//! through the environment (never the command line). The app's own inherited values of those
//! variables, `<prefix>_DATABASE_KIND` and `<prefix>_CONFIG`, are not passed on, so the test sees
//! the same file the settings show.

use std::path::Path;
use std::time::Duration;

use super::sidecar::Sidecar;
use super::sidecar_cli::clean;
use super::sidecar_remote_url::scrub;
use crate::env::Env;
use crate::model::MemoryRemoteTest;
use crate::process::{ExecOutput, ExecSpec, ProcessRunner, launcher_search_dirs};

/// How long `<binary> check` may take (20 s).
pub const CHECK_TIMEOUT: Duration = Duration::from_secs(20);

/// The output kept of each stream (16 KiB; the answer is one line).
const CHECK_OUTPUT_BYTES: usize = 16 * 1024;

/// What one check tests.
pub(crate) struct CheckRequest<'a> {
    /// The URL typed in, passed through the environment; `None` tests the saved one.
    pub url: Option<&'a str>,
    /// The `database_kind` the environment tells the server (`None`: the server picks).
    pub database_kind: Option<&'static str>,
    /// `Some(None)` tests without a token, `Some(Some(token))` with that one, `None` with the
    /// saved one.
    pub token: Option<Option<&'a str>>,
    /// Every secret the server may quote (the typed and the saved ones), masked out of its
    /// answer, longest first.
    pub secrets: &'a [String],
    /// Run `check --json` (the server answers one JSON object) instead of `check`.
    pub json: bool,
}

/// Runs `<binary> check` of `sidecar` at `server`. `ok` is the server's exit status; `message` its
/// first line (standard output on success, standard error on failure) with `request.secrets`
/// masked and passed through [`redact_text`](crate::security::redact_text), or why it could not run.
pub(crate) async fn run_check(
    sidecar: &Sidecar,
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    request: CheckRequest<'_>,
) -> MemoryRemoteTest {
    let secrets = request.secrets;
    let output = match run_check_raw(sidecar, env, runner, server, request).await {
        Ok(output) => output,
        Err(message) => return failed(message),
    };

    let ok = output.success();
    let (first, second) = if ok {
        (&output.stdout, &output.stderr)
    } else {
        (&output.stderr, &output.stdout)
    };

    let message = match first_line(first).or_else(|| first_line(second)) {
        Some(line) => clean(&scrub(line, secrets)),
        None => silent_answer(sidecar, ok, output.status),
    };

    MemoryRemoteTest { ok, message }
}

/// Runs `<binary> check` of `sidecar` at `server` and returns what it printed, or why it could not
/// run (it did not start, or did not answer within [`CHECK_TIMEOUT`]).
pub(crate) async fn run_check_raw(
    sidecar: &Sidecar,
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    request: CheckRequest<'_>,
) -> std::result::Result<ExecOutput, String> {
    let url_var = sidecar.var("DATABASE_URL");
    let token_var = sidecar.var("DATABASE_TOKEN");
    let kind_var = sidecar.var("DATABASE_KIND");

    let mut spec = ExecSpec::new(server)
        .args(if request.json {
            vec!["check", "--json"]
        } else {
            vec!["check"]
        })
        .env_remove_prefix(&url_var)
        .env_remove_prefix(&token_var)
        .env_remove_prefix(&kind_var)
        .env_remove_prefix(sidecar.var("CONFIG"))
        .search_dirs(launcher_search_dirs(env, server))
        .timeout(CHECK_TIMEOUT)
        .max_output_bytes(CHECK_OUTPUT_BYTES);

    if let Some(url) = request.url {
        spec = spec.env(&url_var, url);

        if let Some(name) = request.database_kind {
            spec = spec.env(&kind_var, name);
        }
    }

    // The server treats an empty variable as unset (and would read the saved token), but trims a
    // set one: a single space tests without a token.
    if let Some(token) = request.token {
        spec = spec.env(&token_var, token.unwrap_or(" "));
    }

    let output = runner.run(&spec).await.map_err(|error| {
        tracing::warn!(%error, "{} check could not be started", sidecar.binary);
        format!("{} could not be started", sidecar.product)
    })?;

    if output.timed_out {
        return Err(format!(
            "{} did not answer within {} seconds",
            sidecar.product,
            CHECK_TIMEOUT.as_secs()
        ));
    }

    Ok(output)
}

/// A test that did not run, with `message` why.
pub(crate) fn failed(message: String) -> MemoryRemoteTest {
    MemoryRemoteTest { ok: false, message }
}

/// The message when the server printed nothing.
fn silent_answer(sidecar: &Sidecar, ok: bool, status: Option<i32>) -> String {
    match (ok, status) {
        (true, _) => format!("{} reached the database", sidecar.product),
        (false, Some(code)) => format!("{} check failed (exit code {code})", sidecar.binary),
        (false, None) => format!("{} check was stopped", sidecar.binary),
    }
}

fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}
