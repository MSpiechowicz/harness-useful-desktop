//! Hiding a team note or script on this machine (`memory_note_hide`, `memory_script_hide`) and
//! discarding the local copies the server no longer reads (`memory_notes_discard_local`,
//! `memory_scripts_discard_local`), and the by-id calls the notes and scripts commands share.
//!
//! Each runs one `harness-memory notes|scripts` command as [`memory_cli`](super::memory_cli) runs
//! the server's commands ([`MemoryCopyKind`] picks which):
//!
//! - `<kind> hide|unhide <id> --json [--project P]` hides a team entry on this machine or shows
//!   it again, answering `{"hidden": <id>}` or `{"unhidden": <id>}`; the team database is not
//!   changed. One saved on this machine is refused (`… is saved on this computer, delete it
//!   instead`, exit 1), and so is a call without a team database (see [`hide_entry`]);
//! - `<kind> discard-local (--all | -) [--scope repo|global] --yes --json` deletes entries from
//!   the local file while they are routed `remote` (ids on stdin with `-`) and answers
//!   `{"deleted": N}` (see [`discard_local`]).
//!
//! An entry the server cannot see from the project is `harness-memory: note <id> not found` (or
//! `script`, exit 1), which comes back as `notFound`.

use std::ffi::OsString;
use std::path::PathBuf;

use serde_json::Value;

use super::memory_cli::{
    CliCall, DELETE_ALL_TIMEOUT, LIST_OUTPUT_BYTES, LIST_TIMEOUT, Repos, count_of, repos_args, run_json, scope_name,
};
use super::memory_scripts::{MemoryScripts, unavailable_error};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{LocalNoteIds, MemoryCopyKind, NoteIdsWord, ScriptScope};
use crate::process::ProcessRunner;

/// Lists or reads the team entries hidden on this machine.
pub(super) const HIDDEN_FLAG: &str = "--hidden";

/// The most local ids one command takes on stdin.
pub const MAX_LOCAL_IDS: usize = 100_000;

/// Hides team entry `id` of `kind` on this machine (`hidden`), or shows it again; the team
/// database is not changed. `notFound` when the server cannot see it from `project` or is not
/// installed, `unsupported` when the server predates hiding, `process` with its (redacted) line
/// when it refuses (an entry saved on this machine, no team database), `protocol` when it answers
/// another id.
pub async fn hide_entry(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    kind: MemoryCopyKind,
    id: i64,
    repos: Repos<'_>,
    hidden: bool,
) -> Result<()> {
    let server = entry_server(env, probe, kind).await?;
    let (command, field) = if hidden {
        ("hide", "hidden")
    } else {
        ("unhide", "unhidden")
    };
    let name = format!("{} {command}", kind_command(kind));
    let args = by_id(kind, command, id, &["--json"], repos);

    let data = run_json(env, runner, &server, call(&name, args))
        .await
        .map_err(|error| not_found(kind, error, id))?;

    if data.get(field).and_then(Value::as_i64) != Some(id) {
        return Err(BusinessError::Protocol(format!(
            "harness-memory {name} answered without `{field}: {id}`"
        )));
    }

    tracing::info!(
        id,
        hidden,
        kind = kind_command(kind),
        "memory entry hidden on this machine changed"
    );
    Ok(())
}

/// Deletes entries of `kind` from the local file while they are routed `remote` (where the server
/// no longer reads them): every one, or those of `ids` (local-file ids, as a copy to the team
/// database lists them), of `scope` only when given. How many were deleted. `invalidInput` for
/// none (`none`, no ids) or an id that is not positive, `notFound` when the server is not
/// installed, `unsupported` when it predates discarding, `process` with its (redacted) line when
/// it refuses (not routed `remote`).
pub async fn discard_local(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    kind: MemoryCopyKind,
    ids: &LocalNoteIds,
    scope: Option<ScriptScope>,
) -> Result<u32> {
    let nothing_picked = || BusinessError::InvalidInput(format!("pick the {} to delete", kind_command(kind)));

    let (which, stdin) = match ids {
        LocalNoteIds::Word(NoteIdsWord::All) => ("--all", None),
        LocalNoteIds::Word(NoteIdsWord::None) => return Err(nothing_picked()),
        LocalNoteIds::Ids(ids) if ids.is_empty() => return Err(nothing_picked()),
        LocalNoteIds::Ids(ids) => ("-", Some(ids_on_stdin(ids)?)),
    };

    let server = entry_server(env, probe, kind).await?;
    let name = format!("{} discard-local", kind_command(kind));

    let mut args = kind_args(kind, vec!["discard-local".into(), which.into()]);
    args.extend(scope_args(scope));
    args.extend(["--yes", "--json"].map(OsString::from));

    let call = CliCall {
        name: &name,
        args,
        timeout: DELETE_ALL_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin,
    };

    let data = run_json(env, runner, &server, call).await?;

    let Some(deleted) = count_of(data.get("deleted")) else {
        return Err(BusinessError::Protocol(format!(
            "harness-memory {name} answered without `deleted`"
        )));
    };

    tracing::info!(
        deleted,
        kind = kind_command(kind),
        "memory entries discarded from this machine"
    );
    Ok(deleted)
}

/// `ids` as the server reads them from stdin (comma-separated). `invalidInput` for an id that is
/// not positive or more than [`MAX_LOCAL_IDS`].
pub(super) fn ids_on_stdin(ids: &[i64]) -> Result<Vec<u8>> {
    if ids.len() > MAX_LOCAL_IDS {
        return Err(BusinessError::InvalidInput(format!(
            "pick at most {MAX_LOCAL_IDS} at a time"
        )));
    }

    if ids.iter().any(|id| *id <= 0) {
        return Err(BusinessError::InvalidInput(
            "a local id is a positive number".to_owned(),
        ));
    }

    let text: Vec<String> = ids.iter().map(i64::to_string).collect();

    Ok(format!("{}\n", text.join(",")).into_bytes())
}

/// `--scope <scope>` when there is one.
pub(super) fn scope_args(scope: Option<ScriptScope>) -> Vec<OsString> {
    match scope {
        Some(scope) => vec!["--scope".into(), scope_name(scope).into()],
        None => Vec::new(),
    }
}

/// The installed server when it has the notes view (the generation whose `notes` and `scripts`
/// commands take pages), else the error that says why not.
pub(super) async fn entry_server(env: &Env, probe: &MemoryScripts, kind: MemoryCopyKind) -> Result<PathBuf> {
    let what = match kind {
        MemoryCopyKind::Notes => "the notes view",
        MemoryCopyKind::Scripts => "scripts",
    };

    probe
        .cli_server(env)
        .await
        .map_err(|availability| unavailable_error(availability, what))
}

/// A call of `name` with `args`, within [`LIST_TIMEOUT`].
pub(super) fn call(name: &str, args: Vec<OsString>) -> CliCall<'_> {
    CliCall {
        name,
        args,
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
        stdin: None,
    }
}

/// `notes` or `scripts`, then `rest`.
pub(super) fn kind_args(kind: MemoryCopyKind, rest: Vec<OsString>) -> Vec<OsString> {
    let mut args = vec![OsString::from(kind_command(kind))];
    args.extend(rest);

    args
}

/// `<kind> <command> <id> [<flags>] [--project <root>]`.
pub(super) fn by_id(kind: MemoryCopyKind, command: &str, id: i64, flags: &[&str], repos: Repos<'_>) -> Vec<OsString> {
    let mut rest: Vec<OsString> = vec![command.into(), id.to_string().into()];
    rest.extend(flags.iter().map(OsString::from));
    rest.extend(repos_args(repos));

    kind_args(kind, rest)
}

/// `error` as `notFound` when it is the server's line for an entry of `kind` it cannot see.
pub(super) fn not_found(kind: MemoryCopyKind, error: BusinessError, id: i64) -> BusinessError {
    let noun = match kind {
        MemoryCopyKind::Notes => "note",
        MemoryCopyKind::Scripts => "script",
    };

    match error {
        BusinessError::Process(message) if message == format!("harness-memory: {noun} {id} not found") => {
            BusinessError::NotFound(format!("{noun} {id}"))
        }
        other => other,
    }
}

/// The server's command of `kind`: `notes` or `scripts`.
pub(super) fn kind_command(kind: MemoryCopyKind) -> &'static str {
    match kind {
        MemoryCopyKind::Notes => "notes",
        MemoryCopyKind::Scripts => "scripts",
    }
}
