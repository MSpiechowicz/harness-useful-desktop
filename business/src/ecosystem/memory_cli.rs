//! One run of a `harness-memory` command for the user (`trust`, `notes`, `scripts list`,
//! `scripts delete --all`), shared by [`memory_trust`](super::memory_trust),
//! [`memory_notes`](super::memory_notes) and the scripts list of
//! [`memory_scripts`](super::memory_scripts), the page of a list ([`ListWindow`]), and deleting
//! every note or script ([`delete_all`]).
//!
//! The server runs in the home directory with closed stdin, `NO_COLOR=1`, a time limit and capped
//! output. When it fails, the line that says why is the last one it prefixes with
//! `harness-memory:` (it prints that one last, after any warning), else the last line that is not
//! empty, stderr before stdout; it comes back through [`redact_text`], at most 500 characters. A
//! usage error (exit 2) means the server does not know the command or an option: it predates it.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::catalog::MEMORY_BINARY;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{MemoryDeleteAll, MemoryScopeCounts, MemoryScriptsAvailability, ScriptScope, SortOrder};
use crate::process::{ExecSpec, ProcessRunner, launcher_search_dirs};
use crate::security::redact_text;

/// How long a notes or scripts command may take (15 s).
pub const LIST_TIMEOUT: Duration = Duration::from_secs(15);

/// The output kept of each stream of a notes or scripts command (1 MiB: a page of 200 entries,
/// or a note with its links, is far less).
pub(crate) const LIST_OUTPUT_BYTES: usize = 1024 * 1024;

/// How long deleting every note or script may take (60 s: one write per store, but a team
/// database far away holds thousands).
pub const DELETE_ALL_TIMEOUT: Duration = Duration::from_secs(60);

/// The entries of a page when the UI does not say.
pub const DEFAULT_PAGE_SIZE: u32 = 50;

/// The most entries on a page.
pub const MAX_PAGE_SIZE: u32 = 200;

/// The longest search, in characters (the server's own limit).
pub const MAX_QUERY_CHARS: usize = 256;

/// The `updated` sort, newest first unless asked otherwise; other sorts go A to Z.
const UPDATED_SORT: &str = "updated";

/// Which page of a list to read, and the search that filters it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListWindow {
    offset: u32,
    limit: u32,
    query: Option<String>,
    order: Option<SortOrder>,
}

impl ListWindow {
    /// From offset 0 and [`DEFAULT_PAGE_SIZE`] entries unless given; a blank `query` is none.
    /// `invalidInput` for a limit of 0 or above [`MAX_PAGE_SIZE`], or a query longer than
    /// [`MAX_QUERY_CHARS`].
    pub fn new(
        offset: Option<u32>,
        limit: Option<u32>,
        query: Option<String>,
        order: Option<SortOrder>,
    ) -> Result<Self> {
        let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);

        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(BusinessError::InvalidInput(format!(
                "a page holds 1 to {MAX_PAGE_SIZE} entries"
            )));
        }

        let query = query
            .map(|query| query.trim().to_owned())
            .filter(|query| !query.is_empty());

        if query
            .as_ref()
            .is_some_and(|query| query.chars().count() > MAX_QUERY_CHARS)
        {
            return Err(BusinessError::InvalidInput(format!(
                "a search is at most {MAX_QUERY_CHARS} characters"
            )));
        }

        Ok(Self {
            offset: offset.unwrap_or(0),
            limit,
            query,
            order,
        })
    }

    /// `list --json --offset N --limit N [--query Q] <filters> --sort S --order O
    /// [--project P]`: `filters` are the command's own options and values (`--kind fix`).
    pub(crate) fn list_args(&self, filters: &[(&str, &str)], sort: &str, project: Option<&Path>) -> Vec<OsString> {
        let order = match self.order {
            Some(SortOrder::Asc) => "asc",
            Some(SortOrder::Desc) => "desc",
            None if sort == UPDATED_SORT => "desc",
            None => "asc",
        };

        let mut args: Vec<OsString> = ["list", "--json", "--offset"].map(OsString::from).into();
        args.push(self.offset.to_string().into());
        args.push("--limit".into());
        args.push(self.limit.to_string().into());

        if let Some(query) = &self.query {
            args.push("--query".into());
            args.push(query.into());
        }

        for (option, value) in filters {
            args.push(option.into());
            args.push(value.into());
        }

        args.extend(["--sort", sort, "--order", order].map(OsString::from));
        args.extend(project_args(project));

        args
    }
}

/// `--project <root>` when there is a project.
pub(crate) fn project_args(project: Option<&Path>) -> Vec<OsString> {
    match project {
        Some(project) => vec!["--project".into(), project.as_os_str().to_owned()],
        None => Vec::new(),
    }
}

/// What the server's own lines start with.
const SERVER_PREFIX: &str = "harness-memory:";

/// The longest message returned, in characters.
const MAX_MESSAGE_CHARS: usize = 500;

/// The exit code of a usage error: the server does not know the command or one of its options.
pub(crate) const USAGE_EXIT: i32 = 2;

/// One command to run.
pub(crate) struct CliCall<'a> {
    /// The command's name in messages (`trust`, `notes list`).
    pub name: &'a str,
    /// The arguments after the binary.
    pub args: Vec<OsString>,
    /// How long it may take.
    pub timeout: Duration,
    /// The output kept of each stream.
    pub output_bytes: usize,
}

/// How a command that ran in time ended.
pub(crate) enum CliAnswer {
    /// It exited 0: its stdout.
    Done(String),
    /// It failed: its exit code (`None` when stopped by a signal) and the line that says why
    /// (redacted, clipped), or a sentence with the exit code when it printed nothing.
    Failed {
        /// The exit code.
        status: Option<i32>,
        /// What to show.
        message: String,
    },
}

/// Runs `call` on the server at `server`. `process` when it does not start or does not answer in
/// time.
pub(crate) async fn run_memory_cli(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
) -> Result<CliAnswer> {
    let spec = ExecSpec::new(server)
        .args(call.args)
        .cwd(&env.home)
        .env("NO_COLOR", "1")
        .search_dirs(launcher_search_dirs(env, server))
        .timeout(call.timeout)
        .max_output_bytes(call.output_bytes);

    let output = runner.run(&spec).await.map_err(|error| {
        tracing::warn!(%error, command = call.name, "harness-memory could not be started");
        BusinessError::Process("Harness Memory could not be started".to_owned())
    })?;

    if output.timed_out {
        return Err(BusinessError::Process(format!(
            "Harness Memory did not answer within {} seconds",
            call.timeout.as_secs()
        )));
    }

    if output.success() {
        return Ok(CliAnswer::Done(output.stdout));
    }

    let decisive = decisive_line(&output.stderr).or_else(|| decisive_line(&output.stdout));

    let message = match decisive {
        Some(line) => clean(line),
        None => match output.status {
            Some(code) => format!("harness-memory {} failed (exit code {code})", call.name),
            None => format!("harness-memory {} was stopped", call.name),
        },
    };

    Ok(CliAnswer::Failed {
        status: output.status,
        message,
    })
}

/// The line of `text` that says why the server failed (see the module docs).
fn decisive_line(text: &str) -> Option<&str> {
    let lines = || text.lines().map(str::trim).filter(|line| !line.is_empty());

    lines()
        .rev()
        .find(|line| line.starts_with(SERVER_PREFIX))
        .or_else(|| lines().next_back())
}

/// `line` through [`redact_text`], at most [`MAX_MESSAGE_CHARS`] long.
fn clean(line: &str) -> String {
    let redacted = redact_text(line);

    match redacted.char_indices().nth(MAX_MESSAGE_CHARS) {
        Some((end, _)) => format!("{}…", &redacted[..end]),
        None => redacted,
    }
}

/// Runs `call` and returns its stdout. `unsupported` for a usage error (the server predates the
/// command), `process` with the server's line when it fails otherwise, does not start or does not
/// answer in time.
pub(crate) async fn run_text(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    call: CliCall<'_>,
) -> Result<String> {
    let name = call.name.to_owned();

    match run_memory_cli(env, runner, server, call).await? {
        CliAnswer::Done(stdout) => Ok(stdout),
        CliAnswer::Failed {
            status: Some(USAGE_EXIT),
            ..
        } => Err(BusinessError::Unsupported(format!(
            "this {MEMORY_BINARY} predates `{name}`; update Harness Memory"
        ))),
        CliAnswer::Failed { message, .. } => Err(BusinessError::Process(message)),
    }
}

/// [`run_text`], its stdout read as JSON. `protocol` when it is not JSON (the parser's message is
/// dropped: it can quote the data).
pub(crate) async fn run_json(env: &Env, runner: &dyn ProcessRunner, server: &Path, call: CliCall<'_>) -> Result<Value> {
    let name = call.name.to_owned();
    let stdout = run_text(env, runner, server, call).await?;

    serde_json::from_str(&stdout)
        .map_err(|_| BusinessError::Protocol(format!("harness-memory {name} answered output this app cannot read")))
}

/// A list command's JSON answer, and whether the server combines filters.
pub(crate) struct ListAnswer {
    /// The answer.
    pub data: Value,
    /// True when the server took the filters only a server that combines them knows, or answered
    /// without them as such a server does (with `scopes`).
    pub combined_filters: bool,
}

/// Runs list command `name` with `args`. `fallback` is the same command without the filters only
/// a server that combines them knows (`--scope`, `--review`), when `args` has any: a usage error
/// then means an older server, which is asked once more with `fallback` and answers without
/// combined filters. Fails as [`run_json`].
pub(crate) async fn run_list(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    name: &str,
    args: Vec<OsString>,
    fallback: Option<Vec<OsString>>,
) -> Result<ListAnswer> {
    let call = |args| CliCall {
        name,
        args,
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
    };

    match (run_json(env, runner, server, call(args)).await, fallback) {
        (Err(BusinessError::Unsupported(_)), Some(fallback)) => {
            tracing::info!(
                command = name,
                "harness-memory predates combined filters; listing without them"
            );

            let data = run_json(env, runner, server, call(fallback)).await?;

            Ok(ListAnswer {
                data,
                combined_filters: false,
            })
        }
        (answer, _) => {
            let data = answer?;
            let combined_filters = data.get("scopes").is_some_and(Value::is_object);

            Ok(ListAnswer { data, combined_filters })
        }
    }
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
    let scopes = data.get("scopes").filter(|scopes| scopes.is_object())?;

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
/// which answers `{"deleted": N, "team": M}`. `unsupported` when the server predates it,
/// `process` with its line when it fails, `protocol` when it answers without `deleted`.
pub(crate) async fn delete_all(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    command: &str,
    project: Option<&Path>,
) -> Result<MemoryDeleteAll> {
    let name = format!("{command} delete --all");

    let mut args: Vec<OsString> = [command, "delete", "--all", "--yes", "--json"]
        .map(OsString::from)
        .into();
    args.extend(project_args(project));

    let call = CliCall {
        name: &name,
        args,
        timeout: DELETE_ALL_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
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
    };

    tracing::info!(command, deleted, team = ?outcome.team, "memory entries deleted");
    Ok(outcome)
}
