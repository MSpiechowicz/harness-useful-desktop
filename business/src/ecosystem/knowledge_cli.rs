//! One run of a `harness-knowledge` command for the user (`docs …`, `repos`, `stats`, `held …`,
//! `search`), shared by [`knowledge_docs`](super::knowledge_docs), [`knowledge_held`](super::knowledge_held)
//! and [`knowledge_search`](super::knowledge_search).
//!
//! The run, its limits and its error lines are [`sidecar_cli`](super::sidecar_cli)'s, for
//! [`KNOWLEDGE_SIDECAR`]. A failed command also prints `{"error": "failed", "message"}` on stdout,
//! which this app does not read: the last `harness-knowledge:` line of stderr says the same. A
//! command on a database that lacks a capability exits 3 with `{"error": "missing", "code",
//! "message"}`: [`run_typed`] reads it into a [`KnowledgeProblem`] (the pages show it as their
//! availability), [`run_json`] as a `process` error with the server's sentence. A command that
//! needs the remote database while it cannot be reached (exit 1, "remote knowledge is unreachable
//! right now") is an `unreachable` error, so the UI says so in its own words.

use std::ffi::OsString;
use std::path::Path;

use serde_json::Value;

use super::catalog::KNOWLEDGE_SIDECAR;
use super::sidecar_cli::{CliAnswer, LIST_OUTPUT_BYTES, LIST_TIMEOUT, clean, run_sidecar_cli};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{KnowledgeAvailability, KnowledgeProblem};
use crate::process::ProcessRunner;

pub(crate) use super::sidecar_cli::CliCall;

/// A command with the limits of a list or a read (15 s, 1 MiB of output) and a closed stdin.
pub(crate) fn call(name: &str, args: Vec<OsString>) -> CliCall<'_> {
    CliCall {
        name,
        args,
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    }
}

/// `args` as the words of a command.
pub(crate) fn words(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

/// The exit code of a command on a database that lacks a capability.
const MISSING_EXIT: i32 = 3;

/// Why a command did not answer.
#[derive(Debug)]
pub(crate) enum CliFailure {
    /// It failed or could not run.
    Error(BusinessError),
    /// The database lacks a capability (exit 3): what, and the server's sentence (redacted).
    Missing(KnowledgeProblem, String),
}

impl From<BusinessError> for CliFailure {
    fn from(error: BusinessError) -> Self {
        Self::Error(error)
    }
}

impl From<CliFailure> for BusinessError {
    fn from(failure: CliFailure) -> Self {
        match failure {
            CliFailure::Error(error) => error,
            CliFailure::Missing(_, message) => Self::Process(message),
        }
    }
}

/// Runs `call` on the knowledge server at `server` and reads its stdout as JSON. `unsupported` for
/// a usage error (the server predates the command), `process` with the server's line when it
/// fails, `protocol` when the answer is not JSON, [`CliFailure::Missing`] for exit 3.
pub(crate) async fn run_typed(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
) -> std::result::Result<Value, CliFailure> {
    let name = call.name.to_owned();

    match run_sidecar_cli(&KNOWLEDGE_SIDECAR, env, runner, server, call).await? {
        CliAnswer::Done(stdout) => serde_json::from_str(&stdout).map_err(|_| {
            CliFailure::Error(BusinessError::Protocol(format!(
                "{} {name} answered output this app cannot read",
                KNOWLEDGE_SIDECAR.binary
            )))
        }),
        CliAnswer::Failed {
            status: Some(MISSING_EXIT),
            message,
            stdout,
        } => match missing(&stdout) {
            Some((problem, sentence)) => Err(CliFailure::Missing(problem, sentence.unwrap_or(message))),
            None => Err(CliFailure::Error(BusinessError::Process(message))),
        },
        CliAnswer::Failed {
            status: Some(super::sidecar_cli::USAGE_EXIT),
            ..
        } => Err(CliFailure::Error(BusinessError::Unsupported(format!(
            "this {} predates `{name}`, so update {}",
            KNOWLEDGE_SIDECAR.binary, KNOWLEDGE_SIDECAR.product
        )))),
        CliAnswer::Failed { message, stdout, .. } => match unreachable_in(&stdout, &message) {
            Some(sentence) => Err(CliFailure::Error(BusinessError::Unreachable(sentence))),
            None => Err(CliFailure::Error(BusinessError::Process(message))),
        },
    }
}

/// The end of the server's line when it cannot read a remote document or reach the remote database
/// (`harness-knowledge: remote knowledge is unreachable right now`).
const UNREACHABLE_LINE: &str = "remote knowledge is unreachable right now";

/// The sentence to show when a failed command could not reach the remote database: the server's
/// `{"error": "unreachable", "message"}` on stdout first (it survives a clipped stderr line), else
/// its stderr line (`message`) when that says so. `None` for any other failure.
pub(crate) fn unreachable_in(stdout: &str, message: &str) -> Option<String> {
    if let Some(data) = stdout_object(stdout)
        && data.get("error").and_then(Value::as_str) == Some("unreachable")
    {
        return Some(
            data.get("message")
                .and_then(Value::as_str)
                .map_or_else(|| message.to_owned(), clean),
        );
    }

    // The line ends with the words, so a long line that merely mentions them is not one.
    message
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace())
        .ends_with(UNREACHABLE_LINE)
        .then(|| message.to_owned())
}

/// The first JSON object line of `stdout`.
pub(crate) fn stdout_object(stdout: &str) -> Option<Value> {
    stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with('{'))
        .and_then(|line| serde_json::from_str(line).ok())
}

/// [`run_typed`] with a missing capability as a `process` error carrying the server's sentence.
pub(crate) async fn run_json(env: &Env, runner: &dyn ProcessRunner, server: &Path, call: CliCall<'_>) -> Result<Value> {
    run_typed(env, runner, server, call).await.map_err(Into::into)
}

/// The problem and sentence in `{"error": "missing", "code", "message"}`; `None` for any other
/// output.
fn missing(stdout: &str) -> Option<(KnowledgeProblem, Option<String>)> {
    let data = stdout_object(stdout)?;

    if data.get("error")?.as_str()? != "missing" {
        return None;
    }

    let problem = problem_of(data.get("code")?.as_str()?);
    let message = data.get("message").and_then(Value::as_str).map(clean);

    Some((problem, message))
}

/// The problem a `check` or `missing` code names (`Other` for one this app does not know).
pub(crate) fn problem_of(code: &str) -> KnowledgeProblem {
    match code {
        "no-mongot" => KnowledgeProblem::NoMongot,
        "no-pgvector" => KnowledgeProblem::NoPgvector,
        "pgvector-no-privilege" => KnowledgeProblem::PgvectorNoPrivilege,
        "pgvector-too-old" => KnowledgeProblem::PgvectorTooOld,
        "libsql-no-vector" => KnowledgeProblem::LibsqlNoVector,
        "no-fts5" => KnowledgeProblem::NoFts5,
        "index-building" => KnowledgeProblem::IndexBuilding,
        "unreachable" => KnowledgeProblem::Unreachable,
        "auth" => KnowledgeProblem::Auth,
        _ => KnowledgeProblem::Other,
    }
}

/// What a failed read says on its page: [`availability_of`], or the missing capability.
pub(crate) fn availability_of_failure(failure: CliFailure) -> KnowledgeAvailability {
    match failure {
        CliFailure::Missing(problem, message) => KnowledgeAvailability::Missing { problem, message },
        CliFailure::Error(error) => availability_of(error),
    }
}

/// What a failed read says on its page: a server that predates the command is `outdated`, a
/// missing one `notInstalled`, anything else `failed` with its message.
pub(crate) fn availability_of(error: BusinessError) -> KnowledgeAvailability {
    match error {
        BusinessError::Unsupported(_) => KnowledgeAvailability::Outdated,
        BusinessError::NotFound(_) => KnowledgeAvailability::NotInstalled,
        BusinessError::Unreachable(_) => KnowledgeAvailability::Unreachable,
        BusinessError::Process(message) | BusinessError::Protocol(message) if is_upgrading(&message) => {
            KnowledgeAvailability::Upgrading
        }
        BusinessError::Process(message) | BusinessError::Protocol(message) => KnowledgeAvailability::Failed { message },
        other => KnowledgeAvailability::Failed {
            message: other.to_string(),
        },
    }
}

/// Whether the server's line says it is upgrading its database (`upgrading the knowledge
/// database`). The server has no code for it yet, so the words are matched.
fn is_upgrading(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains("upgrading the knowledge database")
}

/// `value` as the `u32` of a DTO (a count above `u32::MAX` is `u32::MAX`), 0 when it is no count.
pub(crate) fn count_of(value: Option<&Value>) -> u32 {
    value
        .and_then(Value::as_u64)
        .map_or(0, |count| u32::try_from(count).unwrap_or(u32::MAX))
}

/// `value`'s field `name` read as `T`; `protocol` (naming `command`) when it is missing or does not
/// fit, so a server that answers another shape fails loudly instead of showing nothing.
pub(crate) fn field<T: serde::de::DeserializeOwned>(value: &Value, name: &str, command: &str) -> Result<T> {
    value
        .get(name)
        .and_then(|found| serde_json::from_value(found.clone()).ok())
        .ok_or_else(|| {
            BusinessError::Protocol(format!(
                "{} {command} answered without a usable `{name}`",
                KNOWLEDGE_SIDECAR.binary
            ))
        })
}
