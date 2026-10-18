//! `harness-memory hooks list|put|delete --json`.
//!
//! - `hooks list --json [--project <root>] --offset N --limit N` answers `{"total", "nextOffset",
//!   "project": {"key", "shareable"} | null, "hooks": [row…]}`: the global team hooks and those of
//!   the repository of `<root>`. [`list_team_hooks`] reads every page.
//! - `hooks put --json [--project <root>] [--expect-sha <sha>]` reads the body on stdin and answers
//!   the stored row. Without `--expect-sha` it only inserts; with it, it updates only while the
//!   stored hash is that one.
//! - `hooks delete <id> [--expect-sha <sha>] --json` answers `{"deleted": bool}`.
//! - A failure exits 1 with `{"error": "conflict"|"notFound"|"invalid"|"refused"|"limit"|"local",
//!   "message"}` on stdout, which becomes the matching error (the message redacted and clipped,
//!   never logged). A usage error (exit 2) is a server that predates the command.
//!
//! Ids and hashes are checked before they reach the command line.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use crate::ecosystem::catalog::MEMORY_BINARY;
use crate::ecosystem::memory_cli::{CliAnswer, CliCall, USAGE_EXIT, clean, project_args, run_memory_cli};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::hooks::team::{ProjectKey, TeamRow};
use crate::hooks::{MAX_HOOK_FILE_BYTES, is_hook_id};
use crate::process::ProcessRunner;

/// How long one hooks command may take (15 s: one query against a team database).
pub const HOOKS_TIMEOUT: Duration = Duration::from_secs(15);

/// The largest JSON one row can take: a 16 KiB body and a 512-byte repository key, each byte
/// escaped as `\u00XX` (6 bytes), and about 1 KiB for the other fields.
const MAX_ROW_JSON_BYTES: usize = MAX_HOOK_FILE_BYTES * 6 + 512 * 6 + 1024;

/// Rows asked for per page: a page of worst-case rows still fits the output kept.
const PAGE_ROWS: usize = 16;

/// The most pages read per list (the server keeps at most 64 hooks per scope, so a project list
/// holds at most 128; this leaves room for rows it skips). A list longer than that is incomplete.
const MAX_PAGES: usize = 16;

/// The output kept of each stream of a hooks command (2 MiB, so a whole list stays within
/// [`MAX_PAGES`] × 2 MiB).
const HOOKS_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

const _: () = assert!(PAGE_ROWS * MAX_ROW_JSON_BYTES + 64 * 1024 <= HOOKS_OUTPUT_BYTES);

/// What one `hooks list` returned, all pages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TeamList {
    /// The identity of the project root asked for, `None` without one.
    pub project: Option<ProjectKey>,
    /// The global team hooks and those of that repository.
    pub rows: Vec<TeamRow>,
    /// False when the list had more than [`MAX_PAGES`] pages: rows may be missing, so nothing may
    /// be removed for being unlisted.
    pub complete: bool,
}

/// Every team hook a chat in `project` (a project root; `None`: global only) sees, all pages.
/// Pages follow the answer's `nextOffset` (the offset to ask for next, rows the server skipped
/// included; `null` at the end); an answer without it (an older server) advances by the page size
/// until `total`. Rows the app cannot read are left out (logged by count). `unsupported` for a
/// server that predates team hooks, `protocol` for a page that cannot be read (cut short at the
/// output cap included), the failure's error otherwise.
pub async fn list_team_hooks(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Option<&Path>,
) -> Result<TeamList> {
    let mut list = TeamList::default();
    let mut offset = 0;
    let mut skipped = 0;

    for page in 0..MAX_PAGES {
        let data = list_page(env, runner, server, project, offset, PAGE_ROWS).await?;

        if page == 0 {
            list.project = data
                .get("project")
                .filter(|project| !project.is_null())
                .map(|project| serde_json::from_value(project.clone()))
                .transpose()
                .map_err(|_| unreadable("hooks list"))?;
        }

        let rows = data
            .get("hooks")
            .and_then(Value::as_array)
            .ok_or_else(|| unreadable("hooks list"))?;

        for row in rows {
            match serde_json::from_value::<TeamRow>(row.clone()) {
                Ok(row) if is_hook_id(&row.id) => list.rows.push(row),
                _ => skipped += 1,
            }
        }

        match next_offset(&data, offset)? {
            Some(next) => offset = next,
            None => {
                list.complete = true;
                break;
            }
        }
    }

    if skipped > 0 {
        tracing::warn!(skipped, "harness-memory listed team hooks this app cannot read");
    }

    if !list.complete {
        tracing::warn!(
            pages = MAX_PAGES,
            "harness-memory listed more team hook pages than this app reads"
        );
    }

    Ok(list)
}

/// The offset of the page after the one at `offset`, `None` at the end. `nextOffset` when the
/// answer has it (it must move forward), else `offset` plus the page size while below `total`.
fn next_offset(data: &Value, offset: usize) -> Result<Option<usize>> {
    match data.get("nextOffset") {
        Some(Value::Null) => Ok(None),
        Some(next) => {
            let next = next
                .as_u64()
                .and_then(|next| usize::try_from(next).ok())
                .filter(|next| *next > offset)
                .ok_or_else(|| unreadable("hooks list"))?;

            Ok(Some(next))
        }
        None => {
            let total = data.get("total").and_then(Value::as_u64).unwrap_or(0);
            let next = offset + PAGE_ROWS;

            Ok((next as u64) < total).map(|more| more.then_some(next))
        }
    }
}

/// The identity the server gives the project root `project` (`hooks list --project`, one row).
pub async fn project_key(env: &Env, runner: &dyn ProcessRunner, server: &Path, project: &Path) -> Result<ProjectKey> {
    let data = list_page(env, runner, server, Some(project), 0, 1).await?;

    data.get("project")
        .filter(|project| !project.is_null())
        .and_then(|project| serde_json::from_value(project.clone()).ok())
        .ok_or_else(|| unreadable("hooks list"))
}

/// Stores `body` as a team hook: a project hook with `project` (its root), an update of the
/// stored version `expect_sha` when given, an insert otherwise. Returns the stored row.
/// `conflict` when the team copy changed (or exists, for an insert), `notFound` when it is gone,
/// `forbidden` when the server refuses it (a Harness family hook, a `path:` project), `invalidInput`
/// for a body it calls invalid or a scope at its limit, `unsupported` while sharing is off.
pub async fn put_team_hook(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    body: &[u8],
    project: Option<&Path>,
    expect_sha: Option<&str>,
) -> Result<TeamRow> {
    if body.len() > MAX_HOOK_FILE_BYTES {
        return Err(BusinessError::InvalidInput("the hook is larger than 16 KiB".to_owned()));
    }

    let mut args: Vec<OsString> = ["hooks", "put", "--json"].map(OsString::from).into();
    args.extend(project_args(project));
    args.extend(expect_args(expect_sha)?);

    let data = run(env, runner, server, "hooks put", args, Some(body.to_vec())).await?;

    serde_json::from_value(data).map_err(|_| unreadable("hooks put"))
}

/// Removes team hook `id` while its stored version is `expect_sha` (when given). Returns whether
/// there was one. `conflict` when it changed since.
pub async fn delete_team_hook(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &str,
    expect_sha: Option<&str>,
) -> Result<bool> {
    if !is_hook_id(id) {
        return Err(BusinessError::InvalidInput(
            "a hook id is 32 lowercase hex characters".to_owned(),
        ));
    }

    let mut args: Vec<OsString> = ["hooks", "delete", id].map(OsString::from).into();
    args.extend(expect_args(expect_sha)?);
    args.push("--json".into());

    let data = run(env, runner, server, "hooks delete", args, None).await?;

    data.get("deleted")
        .and_then(Value::as_bool)
        .ok_or_else(|| unreadable("hooks delete"))
}

/// One page of `hooks list`.
async fn list_page(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Option<&Path>,
    offset: usize,
    limit: usize,
) -> Result<Value> {
    let mut args: Vec<OsString> = ["hooks", "list", "--json", "--offset"].map(OsString::from).into();
    args.push(offset.to_string().into());
    args.push("--limit".into());
    args.push(limit.to_string().into());
    args.extend(project_args(project));

    run(env, runner, server, "hooks list", args, None).await
}

/// `--expect-sha <sha>` when there is one (a lowercase SHA-256, `invalidInput` otherwise).
fn expect_args(expect_sha: Option<&str>) -> Result<Vec<OsString>> {
    let Some(sha) = expect_sha else {
        return Ok(Vec::new());
    };

    let valid = sha.len() == 64
        && sha
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));

    if !valid {
        return Err(BusinessError::InvalidInput(
            "a hook hash is 64 lowercase hex characters".to_owned(),
        ));
    }

    Ok(vec!["--expect-sha".into(), sha.into()])
}

/// Runs hooks command `name` and reads its JSON answer (see the module docs for failures).
async fn run(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    name: &str,
    args: Vec<OsString>,
    stdin: Option<Vec<u8>>,
) -> Result<Value> {
    let call = CliCall {
        name,
        args,
        timeout: HOOKS_TIMEOUT,
        output_bytes: HOOKS_OUTPUT_BYTES,
        stdin,
    };

    match run_memory_cli(env, runner, server, call).await? {
        CliAnswer::Done(stdout) => serde_json::from_str(&stdout).map_err(|_| unreadable(name)),
        CliAnswer::Failed {
            status: Some(USAGE_EXIT),
            ..
        } => Err(BusinessError::Unsupported(format!(
            "this {MEMORY_BINARY} predates `{name}`. Update Harness Memory."
        ))),
        CliAnswer::Failed { message, stdout, .. } => {
            let error = failure(&stdout, message);
            tracing::warn!(command = name, code = ?error.code(), "harness-memory hooks command failed");

            Err(error)
        }
    }
}

/// The error a failure answer stands for; `message` (the server's line) when it printed none.
fn failure(stdout: &str, message: String) -> BusinessError {
    let Ok(data) = serde_json::from_str::<Value>(stdout.trim()) else {
        return BusinessError::Process(message);
    };

    let Some(code) = data.get("error").and_then(Value::as_str) else {
        return BusinessError::Process(message);
    };

    let text = data
        .get("message")
        .and_then(Value::as_str)
        .map(clean)
        .filter(|text| !text.is_empty())
        .unwrap_or(message);

    match code {
        "conflict" => BusinessError::Conflict(text),
        "notFound" => BusinessError::NotFound(text),
        "invalid" | "limit" => BusinessError::InvalidInput(text),
        "refused" => BusinessError::Forbidden(text),
        "local" => BusinessError::Unsupported(text),
        _ => BusinessError::Process(text),
    }
}

fn unreadable(name: &str) -> BusinessError {
    BusinessError::Protocol(format!("harness-memory {name} answered output this app cannot read"))
}
