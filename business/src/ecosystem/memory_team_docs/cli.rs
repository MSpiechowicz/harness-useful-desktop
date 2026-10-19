//! `harness-memory <plural> list|get|put|delete --json` for a kind of team documents (`hooks`).
//!
//! - `<plural> list --json [--project <root>] --offset N --limit N` answers `{"total",
//!   "nextOffset", "project": {"key", "shareable"} | null, "<plural>": [row…]}`: the global team
//!   documents and those of the repository of `<root>`. [`list_team_docs`] reads every page.
//! - `<plural> put --json [--project <root>] [--expect-sha <sha>]` reads the body on stdin and
//!   answers the stored row. Without `--expect-sha` it only inserts; with it, it updates only
//!   while the stored hash is that one.
//! - `<plural> get <id> --json` answers the row of one team document (`notFound` without one).
//! - `<plural> delete <id> [--expect-sha <sha>] --json` answers `{"deleted": bool}`.
//! - `<plural> delete --mine --json` (routines) answers `{"deleted": n, "ids": [id…]}`: every team
//!   document whose author is the user name the server records for this user.
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
use crate::process::ProcessRunner;
use crate::signed_docs::fields::MAX_REPO_KEY_BYTES;
use crate::signed_docs::team::{ProjectKey, ShareRequest, TeamRow};
use crate::signed_docs::{DocKind, check_id, is_doc_id, is_sha256};

/// How long one command may take (15 s: one query against a team database).
pub const DOCS_TIMEOUT: Duration = Duration::from_secs(15);

/// The largest body a kind may have (checked for each kind in [`list_team_docs`]).
const MAX_BODY_BYTES: usize = 16 * 1024;

/// The largest JSON one row can take: a 16 KiB body and a 512-byte repository key, each byte
/// escaped as `\u00XX` (6 bytes), and about 1 KiB for the other fields.
const MAX_ROW_JSON_BYTES: usize = MAX_BODY_BYTES * 6 + MAX_REPO_KEY_BYTES * 6 + 1024;

/// Rows asked for per page: a page of worst-case rows still fits the output kept.
const PAGE_ROWS: usize = 16;

/// The most pages read per list (the server keeps at most 64 documents per scope, so a project
/// list holds at most 128; this leaves room for rows it skips). A list longer than that is
/// incomplete.
const MAX_PAGES: usize = 16;

/// The output kept of each stream of a command (2 MiB, so a whole list stays within
/// [`MAX_PAGES`] × 2 MiB).
const DOCS_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

const _: () = assert!(PAGE_ROWS * MAX_ROW_JSON_BYTES + 64 * 1024 <= DOCS_OUTPUT_BYTES);

/// What one `<plural> list` returned, all pages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TeamList {
    /// The identity of the project root asked for, `None` without one.
    pub project: Option<ProjectKey>,
    /// The global team documents and those of that repository.
    pub rows: Vec<TeamRow>,
    /// False when the list had more than [`MAX_PAGES`] pages: rows may be missing, so nothing may
    /// be removed for being unlisted.
    pub complete: bool,
}

/// Every team document of kind `K` a chat in `project` (a project root; `None`: global only)
/// sees, all pages. Pages follow the answer's `nextOffset` (the offset to ask for next, rows the
/// server skipped included; `null` at the end); an answer without it (an older server) advances by
/// the page size until `total`. Rows the app cannot read are left out (logged by count).
/// `unsupported` for a server that predates the kind's team documents, `protocol` for a page that
/// cannot be read (cut short at the output cap included), the failure's error otherwise.
pub async fn list_team_docs<K: DocKind>(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Option<&Path>,
) -> Result<TeamList> {
    const { assert!(K::MAX_FILE_BYTES <= MAX_BODY_BYTES) };

    let name = format!("{} list", K::PLURAL);
    let mut list = TeamList::default();
    let mut offset = 0;
    let mut skipped = 0;

    for page in 0..MAX_PAGES {
        let data = list_page::<K>(env, runner, server, project, offset, PAGE_ROWS).await?;

        if page == 0 {
            list.project = data
                .get("project")
                .filter(|project| !project.is_null())
                .map(|project| serde_json::from_value(project.clone()))
                .transpose()
                .map_err(|_| unreadable(&name))?;
        }

        let rows = data
            .get(K::PLURAL)
            .and_then(Value::as_array)
            .ok_or_else(|| unreadable(&name))?;

        for row in rows {
            match serde_json::from_value::<TeamRow>(row.clone()) {
                Ok(row) if is_doc_id(&row.id) => list.rows.push(row),
                _ => skipped += 1,
            }
        }

        match next_offset(&data, offset, &name)? {
            Some(next) => offset = next,
            None => {
                list.complete = true;
                break;
            }
        }
    }

    if skipped > 0 {
        tracing::warn!(skipped, "harness-memory listed team {} this app cannot read", K::PLURAL);
    }

    if !list.complete {
        tracing::warn!(
            pages = MAX_PAGES,
            "harness-memory listed more remote {} pages than this app reads",
            K::NOUN
        );
    }

    Ok(list)
}

/// The offset of the page after the one at `offset`, `None` at the end. `nextOffset` when the
/// answer has it (it must move forward), else `offset` plus the page size while below `total`.
fn next_offset(data: &Value, offset: usize, name: &str) -> Result<Option<usize>> {
    match data.get("nextOffset") {
        Some(Value::Null) => Ok(None),
        Some(next) => {
            let next = next
                .as_u64()
                .and_then(|next| usize::try_from(next).ok())
                .filter(|next| *next > offset)
                .ok_or_else(|| unreadable(name))?;

            Ok(Some(next))
        }
        None => {
            let total = data.get("total").and_then(Value::as_u64).unwrap_or(0);
            let next = offset + PAGE_ROWS;

            Ok((next as u64) < total).map(|more| more.then_some(next))
        }
    }
}

/// The identity the server gives the project root `project` (`<plural> list --project`, one
/// row).
pub async fn project_key<K: DocKind>(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: &Path,
) -> Result<ProjectKey> {
    let data = list_page::<K>(env, runner, server, Some(project), 0, 1).await?;

    data.get("project")
        .filter(|project| !project.is_null())
        .and_then(|project| serde_json::from_value(project.clone()).ok())
        .ok_or_else(|| unreadable(&format!("{} list", K::PLURAL)))
}

/// Team document `id` as the team database holds it now (`<plural> get <id> --json`).
/// `notFound` when the team has no such document, `unsupported` while sharing is off, the
/// failure's error otherwise.
pub async fn get_team_doc<K: DocKind>(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &str,
) -> Result<TeamRow> {
    check_id::<K>(id)?;

    let name = format!("{} get", K::PLURAL);
    let args: Vec<OsString> = [K::PLURAL, "get", id, "--json"].map(OsString::from).into();

    let data = run::<K>(env, runner, server, &name, args, None).await?;
    let row: TeamRow = serde_json::from_value(data).map_err(|_| unreadable(&name))?;

    if row.id != id {
        return Err(unreadable(&name));
    }

    Ok(row)
}

/// Stores `body` as a team document: a project document with `project` (its root), an update of
/// the stored version `expect_sha` when given, an insert otherwise. Returns the stored row.
/// `conflict` when the team copy changed (or exists, for an insert), `notFound` when it is gone,
/// `forbidden` when the server refuses it (a Harness family hook, a `path:` project),
/// `invalidInput` for a body it calls invalid or a scope at its limit, `unsupported` while sharing
/// is off.
pub async fn put_team_doc<K: DocKind>(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    body: &[u8],
    project: Option<&Path>,
    expect_sha: Option<&str>,
) -> Result<TeamRow> {
    if body.len() > K::MAX_FILE_BYTES {
        return Err(BusinessError::InvalidInput(format!(
            "the {} is larger than {} KiB",
            K::NOUN,
            K::MAX_FILE_BYTES / 1024
        )));
    }

    let name = format!("{} put", K::PLURAL);
    let mut args: Vec<OsString> = [K::PLURAL, "put", "--json"].map(OsString::from).into();
    args.extend(project_args(project));
    args.extend(expect_args::<K>(expect_sha)?);

    let data = run::<K>(env, runner, server, &name, args, Some(body.to_vec())).await?;

    serde_json::from_value(data).map_err(|_| unreadable(&name))
}

/// A local document stored as its team copy ([`put_shared`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedDoc {
    /// The row the server stored.
    pub row: TeamRow,
    /// The project root of a project document, `None` for a global one.
    pub root: Option<String>,
    /// The repository key the server gives that root.
    pub key: Option<ProjectKey>,
    /// The SHA-256 of the local file shared.
    pub local_sha256: String,
    /// True when it updated the team copy, false when it added one.
    pub update: bool,
}

impl SharedDoc {
    /// The project root and its repository key, for a project document.
    pub fn project(&self) -> Option<(&str, &ProjectKey)> {
        self.root.as_deref().zip(self.key.as_ref())
    }
}

/// Stores the local document of `request` as its team copy (see [`put_team_doc`]): a project
/// document under the repository key the server gives its root (asked now), an update of the
/// team copy it names, an insert otherwise.
pub async fn put_shared<K: DocKind>(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    request: ShareRequest<K>,
) -> Result<SharedDoc> {
    let key = match &request.project {
        Some(root) => Some(project_key::<K>(env, runner, server, Path::new(root)).await?),
        None => None,
    };

    let body = request.body(key.as_ref())?;
    let project = request.project.as_deref().map(Path::new);
    let row = put_team_doc::<K>(env, runner, server, &body, project, request.team_sha256.as_deref()).await?;

    Ok(SharedDoc {
        row,
        update: request.team_sha256.is_some(),
        root: request.project,
        key,
        local_sha256: request.local_sha256,
    })
}

/// Removes team document `id` while its stored version is `expect_sha` (when given). Returns
/// whether there was one. `conflict` when it changed since.
pub async fn delete_team_doc<K: DocKind>(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    id: &str,
    expect_sha: Option<&str>,
) -> Result<bool> {
    check_id::<K>(id)?;

    let name = format!("{} delete", K::PLURAL);
    let mut args: Vec<OsString> = [K::PLURAL, "delete", id].map(OsString::from).into();
    args.extend(expect_args::<K>(expect_sha)?);
    args.push("--json".into());

    let data = run::<K>(env, runner, server, &name, args, None).await?;

    data.get("deleted")
        .and_then(Value::as_bool)
        .ok_or_else(|| unreadable(&name))
}

/// What `<plural> delete --mine` removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TeamDocsRemoved {
    /// How many team documents the server deleted.
    pub deleted: usize,
    /// Their ids (ids this app cannot read are left out, so this may be shorter than `deleted`).
    pub ids: Vec<String>,
}

/// Removes every team document of kind `K` whose author is the name the server records for this
/// user (`<plural> delete --mine --json`; servers offer it for routines). The author is a free-text
/// label, so this is a convenience, not a permission. `unsupported` for a server without it or
/// while sharing is off, the failure's error otherwise.
pub async fn delete_mine_team_docs<K: DocKind>(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
) -> Result<TeamDocsRemoved> {
    let name = format!("{} delete --mine", K::PLURAL);
    let args: Vec<OsString> = [K::PLURAL, "delete", "--mine", "--json"].map(OsString::from).into();

    let data = run::<K>(env, runner, server, &name, args, None).await?;

    let deleted = data
        .get("deleted")
        .and_then(Value::as_u64)
        .and_then(|deleted| usize::try_from(deleted).ok())
        .ok_or_else(|| unreadable(&name))?;

    let ids = data
        .get("ids")
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .filter(|id| is_doc_id(id))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    Ok(TeamDocsRemoved { deleted, ids })
}

/// One page of `<plural> list`.
async fn list_page<K: DocKind>(
    env: &Env,
    runner: &dyn ProcessRunner,
    server: &Path,
    project: Option<&Path>,
    offset: usize,
    limit: usize,
) -> Result<Value> {
    let mut args: Vec<OsString> = [K::PLURAL, "list", "--json", "--offset"].map(OsString::from).into();
    args.push(offset.to_string().into());
    args.push("--limit".into());
    args.push(limit.to_string().into());
    args.extend(project_args(project));

    run::<K>(env, runner, server, &format!("{} list", K::PLURAL), args, None).await
}

/// `--expect-sha <sha>` when there is one (a lowercase SHA-256, `invalidInput` otherwise).
fn expect_args<K: DocKind>(expect_sha: Option<&str>) -> Result<Vec<OsString>> {
    let Some(sha) = expect_sha else {
        return Ok(Vec::new());
    };

    if !is_sha256(sha) {
        return Err(BusinessError::InvalidInput(format!(
            "a {} hash is 64 lowercase hex characters",
            K::NOUN
        )));
    }

    Ok(vec!["--expect-sha".into(), sha.into()])
}

/// Runs command `name` and reads its JSON answer (see the module docs for failures).
async fn run<K: DocKind>(
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
        timeout: DOCS_TIMEOUT,
        output_bytes: DOCS_OUTPUT_BYTES,
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
            tracing::warn!(command = name, code = ?error.code(), "harness-memory {} command failed", K::PLURAL);

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
