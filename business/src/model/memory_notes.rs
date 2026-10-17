//! The notes agents keep in Harness Memory, shown in the Memory view (`memory_notes_list`,
//! `memory_note_get`, `memory_note_delete`).
//!
//! The app reads them with `harness-memory notes list|get|delete` and never opens the memory
//! database itself. A note's scope, place and author have the shapes of a script's.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::memory_scripts::{MemoryScopeCounts, MemoryScriptsAvailability, ScriptAuthor, ScriptPlace, ScriptScope};

/// What a note records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoteKind {
    /// What a piece of work came to.
    Outcome,
    /// Something that went wrong.
    Problem,
    /// How a problem was solved.
    Fix,
    /// A change that made something faster or cheaper.
    Optimisation,
    /// Where something lives.
    Location,
    /// A choice and why it was made.
    Decision,
    /// Anything else.
    Note,
}

/// How `memory_notes_list` sorts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoteSort {
    /// By the last change.
    Updated,
    /// By title.
    Title,
    /// By the kind's name.
    Kind,
    /// Global notes before the repository's (ascending).
    Scope,
    /// Notes on this computer before those in the team database (ascending).
    Place,
}

/// Ascending or descending.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SortOrder {
    /// A to Z, oldest first.
    Asc,
    /// Z to A, newest first.
    Desc,
}

/// One note without its body (`notes list`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NoteSummary {
    /// The server's id.
    #[ts(type = "number")]
    pub id: i64,
    /// What it records.
    pub kind: NoteKind,
    /// One line.
    pub title: String,
    /// Repository or global.
    #[ts(inline)]
    pub scope: ScriptScope,
    /// The repository key (`github.com/o/r`, or `path:/abs` for a folder without a remote) of a
    /// repository note.
    pub repo: Option<String>,
    /// The team database or this computer.
    #[ts(inline)]
    pub place: ScriptPlace,
    /// Short labels.
    pub tags: Vec<String>,
    /// The files, commands and names it is about.
    pub entities: Vec<String>,
    /// Who wrote it.
    #[ts(inline)]
    pub author: ScriptAuthor,
    /// RFC 3339.
    pub created_at: String,
    /// RFC 3339.
    pub updated_at: String,
    /// How often it was rewritten (1 for a new note).
    pub revision: u32,
    /// How often an agent recalled it.
    pub hits: u32,
}

/// One note with its body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NoteContent {
    /// Everything [`NoteSummary`] holds.
    #[serde(flatten)]
    pub summary: NoteSummary,
    /// The text, as the agent wrote it.
    pub body: String,
}

/// Which way a link between two notes points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoteLinkDirection {
    /// From this note to the other.
    Out,
    /// From the other note to this one.
    In,
}

/// A note linked to the one shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NoteLink {
    /// The other note's id.
    #[ts(type = "number")]
    pub id: i64,
    /// Its title.
    pub title: String,
    /// Its kind.
    pub kind: NoteKind,
    /// How they relate (`fixes`, `caused_by`, …), as the agent named it.
    pub relation: String,
    /// Which way the link points.
    pub direction: NoteLinkDirection,
}

/// The answer of `memory_note_get`: the note and its links (at most 50).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NoteDetail {
    /// The note with its body.
    pub note: NoteContent,
    /// The notes it links to and those that link to it.
    pub links: Vec<NoteLink>,
}

/// How many notes of one kind match the query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NoteKindCount {
    /// The kind.
    pub kind: NoteKind,
    /// Its notes under the current query and scope (every kind, whatever `kind` was asked for).
    pub count: u32,
}

/// The answer of `memory_notes_list`: one page of notes when available, empty otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryNotesPage {
    /// Whether the notes could be read.
    pub availability: MemoryScriptsAvailability,
    /// How many notes match the query, kind and scope (all pages).
    pub total: u32,
    /// The notes of each kind under the query and scope, whatever kind was asked for.
    pub kinds: Vec<NoteKindCount>,
    /// The notes of each scope under the query and kind; `None` from a server that does not
    /// combine filters.
    pub scopes: Option<MemoryScopeCounts>,
    /// Whether the server combines `scope` with `kind` (and answered the scope counts): false
    /// from an older one, whose list then follows the query and kind only (a `scope` asked for
    /// was dropped).
    pub combined_filters: bool,
    /// This page, as the server sorts it.
    pub notes: Vec<NoteSummary>,
}
