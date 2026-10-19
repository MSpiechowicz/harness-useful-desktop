//! The notes agents keep in Harness Memory, for the Memory view (`memory_notes_list`,
//! `memory_note_get`, `memory_note_delete`, `memory_notes_delete_all`).
//!
//! Each call runs one `harness-memory notes` command as [`memory_cli`](super::memory_cli) runs the
//! server's commands, within [`LIST_TIMEOUT`]:
//!
//! - `notes list --json --offset N --limit N [--query Q] [--kind K] [--scope repo|global] --sort
//!   updated|title|kind|scope|place --order O [--project P] [--hidden]` answers `{"notes": [...],
//!   "total": N, "kinds": [{"kind", "count"}], "scopes": {"repo", "global"}}`: the repository notes
//!   of the project and the global ones, each with `readOnly` and `hidden`, and with a team
//!   database `hiddenCount` and `localCounts` (`{"repo", "global"}`, every repository). `--hidden`
//!   lists the team notes hidden on this machine instead. A server that predates combined filters
//!   answers without `scopes` and refuses `--scope` (a usage error): the list is then asked once
//!   more without it (see [`run_list`]);
//! - `notes get <id> --json [--project P] [--hidden]` answers `{"note": {... "body"}, "links":
//!   [...]}`; a hidden note only with `--hidden`;
//! - `notes hide|unhide` and `notes discard-local` are [`memory_hidden`](super::memory_hidden)'s,
//!   shared with scripts;
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
    ListAnswer, ListWindow, count_of, counts_by_scope, delete_all, list_availability, run_json, run_list, run_text,
    scope_counts, scope_name,
};
pub(super) use super::memory_hidden::ids_on_stdin;
use super::memory_hidden::{HIDDEN_FLAG, by_id, call, entry_server, kind_args, not_found};
use super::memory_scripts::MemoryScripts;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{
    MemoryCopyKind, MemoryDeleteAll, MemoryNotesPage, MemoryScriptsAvailability, NoteDetail, NoteKind, NoteKindCount,
    NoteSort, NoteSummary, ScriptScope,
};
use crate::process::ProcessRunner;

/// What the notes calls run: `harness-memory notes`.
const NOTES: MemoryCopyKind = MemoryCopyKind::Notes;

/// Which notes [`list_notes`] reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesQuery {
    /// The page and the search.
    pub window: ListWindow,
    /// One kind only, or all.
    pub kind: Option<NoteKind>,
    /// One scope only, or both (dropped by a server that does not combine filters).
    pub scope: Option<ScriptScope>,
    /// `updated` unless given.
    pub sort: Option<NoteSort>,
    /// List the team notes hidden on this machine instead of the visible ones.
    pub hidden: bool,
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
        scopes: None,
        combined_filters: false,
        notes: Vec::new(),
        hidden_count: None,
        local_counts: None,
    };

    let server = match probe.cli_server(env).await {
        Ok(server) => server,
        Err(availability) => return unavailable(availability),
    };

    let sort = match query.sort {
        Some(NoteSort::Title) => "title",
        Some(NoteSort::Kind) => "kind",
        Some(NoteSort::Scope) => "scope",
        Some(NoteSort::Place) => "place",
        Some(NoteSort::Updated) | None => "updated",
    };

    let kind: Vec<(&str, &str)> = query.kind.map(|kind| ("--kind", kind_name(kind))).into_iter().collect();
    let mut filters = kind.clone();
    filters.extend(query.scope.map(|scope| ("--scope", scope_name(scope))));

    let hidden = query.hidden.then(|| OsString::from(HIDDEN_FLAG));
    let listed = |filters: &[(&str, &str)]| {
        let mut args = kind_args(NOTES, query.window.list_args(filters, sort, project));
        args.extend(hidden.clone());
        args
    };

    let args = listed(&filters);
    let fallback = query.scope.map(|_| listed(&kind));

    match run_list(env, runner, &server, "notes list", args, fallback).await {
        Ok(answer) => notes_page(answer),
        Err(error) => unavailable(list_availability(error)),
    }
}

/// Note `id` with its body and links; with `hidden` a team note hidden on this machine (only).
/// `notFound` when the server cannot see it from `project` or is not installed, `unsupported` when
/// it predates the notes view (or `hidden`), `process` with its (redacted) line when it fails.
pub async fn get_note(
    env: &Env,
    probe: &MemoryScripts,
    runner: &dyn ProcessRunner,
    id: i64,
    project: Option<&Path>,
    hidden: bool,
) -> Result<NoteDetail> {
    let server = server(env, probe).await?;
    let flags: &[&str] = if hidden { &["--json", HIDDEN_FLAG] } else { &["--json"] };
    let args = by_id(NOTES, "get", id, flags, project);

    let data = run_json(env, runner, &server, call("notes get", args))
        .await
        .map_err(|error| not_found(NOTES, error, id))?;

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
    let args = by_id(NOTES, "delete", id, &[], project);

    run_text(env, runner, &server, call("notes delete", args))
        .await
        .map_err(|error| not_found(NOTES, error, id))?;

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
pub(super) async fn server(env: &Env, probe: &MemoryScripts) -> Result<PathBuf> {
    entry_server(env, probe, NOTES).await
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
fn notes_page(answer: ListAnswer) -> MemoryNotesPage {
    let data = &answer.data;
    let notes: Vec<NoteSummary> = entries(data, "notes");

    MemoryNotesPage {
        availability: MemoryScriptsAvailability::Available,
        total: count_of(data.get("total")).unwrap_or(u32::try_from(notes.len()).unwrap_or(u32::MAX)),
        kinds: entries::<NoteKindCount>(data, "kinds"),
        scopes: scope_counts(data),
        combined_filters: answer.combined_filters,
        notes,
        hidden_count: count_of(data.get("hiddenCount")),
        local_counts: counts_by_scope(data, "localCounts"),
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
