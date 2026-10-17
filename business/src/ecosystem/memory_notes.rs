//! The notes agents keep in Harness Memory, for the Memory view (`memory_notes_list`,
//! `memory_note_get`, `memory_note_delete`, `memory_notes_delete_all`).
//!
//! Each call runs one `harness-memory notes` command as [`memory_cli`](super::memory_cli) runs the
//! server's commands, within [`LIST_TIMEOUT`]:
//!
//! - `notes list --json --offset N --limit N [--query Q] [--kind K] --sort
//!   updated|title|kind|scope|place --order O [--project P]` answers `{"notes": [...], "total": N, "kinds": [{"kind", "count"}]}`: the
//!   repository notes of the project and the global ones;
//! - `notes get <id> --json [--project P]` answers `{"note": {... "body"}, "links": [...]}`;
//! - `notes delete <id> [--project P]` prints `deleted note <id> <title>`;
//! - `notes delete --all --yes --json [--project P]` deletes every note the list shows without
//!   filters and answers `{"deleted": N, "team": M}` (see [`memory_cli::delete_all`](super::memory_cli::delete_all)).
//!
//! A note the server cannot see from the project is `harness-memory: note <id> not found` (exit
//! 1), which comes back as `notFound`. The notes view needs the server generation that asks before
//! running a script (the probe of [`MemoryScripts`]): an older one is `outdated`, and so is one
//! that answers a usage error. The app never opens the memory database.
//!
//! An entry this app cannot read (a kind it does not know) is skipped; so are the counts of such
//! kinds.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::memory_cli::{
    CliCall, LIST_OUTPUT_BYTES, LIST_TIMEOUT, ListWindow, count_of, delete_all, list_availability, project_args,
    run_json, run_text,
};
use super::memory_scripts::{MemoryScripts, unavailable_error};
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    MemoryDeleteAll, MemoryNotesPage, MemoryScriptsAvailability, NoteDetail, NoteKind, NoteKindCount, NoteSort,
    NoteSummary,
};
use crate::process::ProcessRunner;

/// What an outdated server lacks, in messages.
const NOTES_VIEW: &str = "the notes view";

/// Which notes [`list_notes`] reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesQuery {
    /// The page and the search.
    pub window: ListWindow,
    /// One kind only, or all.
    pub kind: Option<NoteKind>,
    /// `updated` unless given.
    pub sort: Option<NoteSort>,
}

/// One page of the repository notes of `project` (a project root; `None` for the global ones
/// only) and the global ones. Never fails: what went wrong is the page's availability, with no
/// notes.
pub async fn list_notes(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    project: Option<&Path>,
    query: &NotesQuery,
) -> MemoryNotesPage {
    let unavailable = |availability| MemoryNotesPage {
        availability,
        total: 0,
        kinds: Vec::new(),
        notes: Vec::new(),
    };

    let server = match probe.cli_server(env).await {
        Ok(server) => server,
        Err(availability) => return unavailable(availability),
    };

    let kind = query.kind.map(kind_name);
    let sort = match query.sort {
        Some(NoteSort::Title) => "title",
        Some(NoteSort::Kind) => "kind",
        Some(NoteSort::Scope) => "scope",
        Some(NoteSort::Place) => "place",
        Some(NoteSort::Updated) | None => "updated",
    };

    let args = notes_args(query.window.list_args(kind.map(|kind| ("--kind", kind)), sort, project));

    match run_json(env, runner, &server, call("notes list", args)).await {
        Ok(data) => notes_page(&data),
        Err(error) => unavailable(list_availability(error)),
    }
}

/// Note `id` with its body and links. `notFound` when the server cannot see it from `project` or
/// is not installed, `unsupported` when it predates the notes view, `process` with its (redacted)
/// line when it fails.
pub async fn get_note(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    id: i64,
    project: Option<&Path>,
) -> Result<NoteDetail> {
    let server = server(env, probe).await?;
    let args = by_id("get", id, Some("--json"), project);

    let data = run_json(env, runner, &server, call("notes get", args))
        .await
        .map_err(|error| not_found(error, id))?;

    let note = data
        .get("note")
        .cloned()
        .ok_or_else(|| BusinessError::Protocol("harness-memory notes get answered without `note`".to_owned()))?;

    Ok(NoteDetail {
        note: parse(note, "notes get")?,
        links: entries(&data, "links"),
    })
}

/// Deletes note `id`. `notFound` when the server cannot see it from `project` or is not
/// installed, `unsupported` when it predates the notes view, `process` with its (redacted) line
/// when it fails.
pub async fn delete_note(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    id: i64,
    project: Option<&Path>,
) -> Result<()> {
    let server = server(env, probe).await?;
    let args = by_id("delete", id, None, project);

    run_text(env, runner, &server, call("notes delete", args))
        .await
        .map_err(|error| not_found(error, id))?;

    tracing::info!(id, "memory note deleted");
    Ok(())
}

/// Deletes every note [`list_notes`] shows for `project` without filters (all pages): how many,
/// and how many of them were in the team database. `notFound` when the server is not installed,
/// `unsupported` when it predates the notes view or deleting them all, `process` with its
/// (redacted) line when it fails.
pub async fn delete_all_notes(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    project: Option<&Path>,
) -> Result<MemoryDeleteAll> {
    let server = server(env, probe).await?;

    delete_all(env, runner, &server, "notes", project).await
}

/// The installed server when it has the notes view, else the error that says why not.
async fn server(env: &Env, probe: &MemoryScripts) -> Result<PathBuf> {
    probe
        .cli_server(env)
        .await
        .map_err(|availability| unavailable_error(availability, NOTES_VIEW))
}

fn call(name: &str, args: Vec<OsString>) -> CliCall<'_> {
    CliCall {
        name,
        args,
        timeout: LIST_TIMEOUT,
        output_bytes: LIST_OUTPUT_BYTES,
    }
}

/// `notes` and then `rest`.
fn notes_args(rest: Vec<OsString>) -> Vec<OsString> {
    let mut args = vec![OsString::from("notes")];
    args.extend(rest);

    args
}

/// `notes <command> <id> [<flag>] [--project <root>]`.
fn by_id(command: &str, id: i64, flag: Option<&str>, project: Option<&Path>) -> Vec<OsString> {
    let mut rest: Vec<OsString> = vec![command.into(), id.to_string().into()];
    rest.extend(flag.map(OsString::from));
    rest.extend(project_args(project));

    notes_args(rest)
}

/// `error` as `notFound` when it is the server's line for a note it cannot see.
fn not_found(error: BusinessError, id: i64) -> BusinessError {
    match error {
        BusinessError::Process(message) if message == format!("harness-memory: note {id} not found") => {
            BusinessError::NotFound(format!("note {id}"))
        }
        other => other,
    }
}

/// The server's name of `kind`.
fn kind_name(kind: NoteKind) -> &'static str {
    match kind {
        NoteKind::Outcome => "outcome",
        NoteKind::Problem => "problem",
        NoteKind::Fix => "fix",
        NoteKind::Optimisation => "optimisation",
        NoteKind::Location => "location",
        NoteKind::Decision => "decision",
        NoteKind::Note => "note",
    }
}

/// The page in a `notes list --json` answer. A server that does not say `total` has the notes it
/// sent.
fn notes_page(data: &Value) -> MemoryNotesPage {
    let notes: Vec<NoteSummary> = entries(data, "notes");

    MemoryNotesPage {
        availability: MemoryScriptsAvailability::Available,
        total: count_of(data.get("total")).unwrap_or(u32::try_from(notes.len()).unwrap_or(u32::MAX)),
        kinds: entries::<NoteKindCount>(data, "kinds"),
        notes,
    }
}

/// The entries of array `field` this app can read; the others are skipped (logged).
fn entries<T: DeserializeOwned>(data: &Value, field: &str) -> Vec<T> {
    let values = data.get(field).and_then(Value::as_array).cloned().unwrap_or_default();
    let count = values.len();

    let read: Vec<T> = values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect();

    if read.len() < count {
        tracing::warn!(
            field,
            skipped = count - read.len(),
            "memory notes: unreadable entries were skipped"
        );
    }

    read
}

/// `value` as a note DTO. The parser's message is dropped: it can quote the note.
fn parse<T: DeserializeOwned>(value: Value, command: &str) -> Result<T> {
    serde_json::from_value(value)
        .map_err(|_| BusinessError::Protocol(format!("harness-memory {command} answered an unreadable note")))
}
