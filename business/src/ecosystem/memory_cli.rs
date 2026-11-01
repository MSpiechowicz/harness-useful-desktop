//! One run of a `harness-memory` command for the user (`trust`, `notes`, `scripts list`,
//! `scripts delete --all`, `hooks`), shared by [`memory_trust`](super::memory_trust),
//! [`memory_notes`](super::memory_notes), the scripts list of
//! [`memory_scripts`](super::memory_scripts) and [`memory_team_docs`](super::memory_team_docs),
//! the page of a list ([`ListWindow`]; the filters it asks with are
//! [`memory_list`](super::memory_list)'s), and deleting every note or script ([`delete_all`]).
//!
//! The run, its limits and its error lines are [`sidecar_cli`](super::sidecar_cli)'s, for
//! [`MEMORY_SIDECAR`]; this file keeps the memory server's own answers (scope counts, what a
//! failed list says, deleting everything).

use std::ffi::OsString;
use std::path::Path;

use serde_json::Value;

use super::catalog::MEMORY_SIDECAR;
use super::sidecar_cli::{run_sidecar_cli, run_sidecar_json, run_sidecar_text};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{MemoryDeleteAll, MemoryScopeCounts, MemoryScriptsAvailability, ScriptScope};
use crate::process::ProcessRunner;

pub(crate) use super::sidecar_cli::{
    CliAnswer, CliCall, LIST_OUTPUT_BYTES, USAGE_EXIT, clean, is_repo_key, project_args, repos_args,
};
pub use super::sidecar_cli::{DEFAULT_PAGE_SIZE, DELETE_ALL_TIMEOUT, LIST_TIMEOUT, MAX_PAGE_SIZE, MAX_QUERY_CHARS};
pub use super::sidecar_cli::{ListWindow, Repos};

/// Runs `call` on the memory server at `server` (see [`run_sidecar_cli`]).
pub(crate) async fn run_memory_cli(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
) -> Result<CliAnswer> {
    run_sidecar_cli(&MEMORY_SIDECAR, env, runner, server, call).await
}

/// Runs `call` and returns its stdout (see [`run_sidecar_text`]).
pub(crate) async fn run_text(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
) -> Result<String> {
    run_sidecar_text(&MEMORY_SIDECAR, env, runner, server, call).await
}

/// [`run_text`], its stdout read as JSON (see [`run_sidecar_json`]).
pub(crate) async fn run_json(env: &Env, runner: &dyn ProcessRunner, server: &Path, call: CliCall<'_>) -> Result<Value> {
    run_sidecar_json(&MEMORY_SIDECAR, env, runner, server, call).await
}

/// What a failed list says on its page: a server that predates the command is `outdated`, a
/// missing one `notInstalled`, anything else `failed` with its message.
pub(crate) fn list_availability(error: BusinessError) -> MemoryScriptsAvailability {
    match error {
        BusinessError::Unsupported(_) => MemoryScriptsAvailability::Outdated,
        BusinessError::NotFound(_) => MemoryScriptsAvailability::NotInstalled,
        BusinessError::Process(message) | BusinessError::Protocol(message) => {
            MemoryScriptsAvailability::Failed { message }
        }
        other => MemoryScriptsAvailability::Failed {
            message: other.to_string(),
        },
    }
}

/// `count` as the `u32` of a DTO.
pub(crate) fn count_of(value: Option<&Value>) -> Option<u32> {
    value
        .and_then(Value::as_u64)
        .map(|count| u32::try_from(count).unwrap_or(u32::MAX))
}

/// The `scopes` counts of a list answer; `None` when the server does not send them.
pub(crate) fn scope_counts(data: &Value) -> Option<MemoryScopeCounts> {
    counts_by_scope(data, "scopes")
}

/// The counts of each scope in object `field` of a list answer (`{"repo": N, "global": N}`);
/// `None` when the server does not send it.
pub(crate) fn counts_by_scope(data: &Value, field: &str) -> Option<MemoryScopeCounts> {
    let scopes = data.get(field).filter(|scopes| scopes.is_object())?;

    Some(MemoryScopeCounts {
        repo: count_of(scopes.get("repo")).unwrap_or(0),
        global: count_of(scopes.get("global")).unwrap_or(0),
    })
}

/// The server's name of `scope`.
pub(crate) fn scope_name(scope: ScriptScope) -> &'static str {
    match scope {
        ScriptScope::Repo => "repo",
        ScriptScope::Global => "global",
    }
}

/// Deletes every entry `harness-memory <command> list` shows for `project` without filters, with
/// `<command> delete --all --yes --json [--project <root>]` (`command` is `notes` or `scripts`),
/// which answers `{"deleted": N, "team": M}` (notes in a scope routed `remote+local` add
/// `"kept": K`, the team notes left in place). `unsupported` when the server predates it,
/// `process` with its line when it fails, `protocol` when it answers without `deleted`.
pub(crate) async fn delete_all(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    command: &str,
    repos: Repos<'_>,
    scope: Option<ScriptScope>,
) -> Result<MemoryDeleteAll> {
    if repos == Repos::All {
        return Err(BusinessError::InvalidInput(
            "every repository cannot be deleted at once; pick one".to_owned(),
        ));
    }

    let name = format!("{command} delete --all");

    let mut args: Vec<OsString> = [command, "delete", "--all", "--yes", "--json"]
        .map(OsString::from)
        .into();
    args.extend(repos_args(repos));
    args.extend(
        scope
            .into_iter()
            .flat_map(|scope| [OsString::from("--scope"), scope_name(scope).into()]),
    );

    let call = CliCall {
        name: &name,
        args,
        timeout: DELETE_ALL_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    };

    let data = run_json(env, runner, server, call).await?;

    let Some(deleted) = count_of(data.get("deleted")) else {
        return Err(BusinessError::Protocol(format!(
            "harness-memory {name} answered without `deleted`"
        )));
    };

    let outcome = MemoryDeleteAll {
        deleted,
        team: count_of(data.get("team")),
        kept: count_of(data.get("kept")),
    };

    tracing::info!(command, deleted, team = ?outcome.team, kept = ?outcome.kept, "memory entries deleted");
    Ok(outcome)
}
