//! The notes agents keep in Harness Memory, shown in the Memory view (`memory_notes_list`,
//! `memory_note_get`, `memory_note_delete`).
//!
//! The app reads them with `harness-memory notes list|get|delete` and never opens the memory
//! database itself. A note's scope, place and author have the shapes of a script's.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::memory_scripts::{
    MemoryPlaceCounts, MemoryScopeCounts, MemoryScriptsAvailability, ScriptAuthor, ScriptPlace, ScriptScope,
};

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
    /// By how often an agent used the note, most first. Asked of a server that lists
    /// `notes-useful` only.
    Useful,
    /// By when an agent last used the note, latest first. Asked of a server that lists
    /// `notes-useful` only.
    LastUseful,
}

impl NoteSort {
    /// Whether the sort reads the usefulness counts, which a server lists as `notes-useful` only.
    #[must_use]
    pub fn needs_useful(self) -> bool {
        matches!(self, Self::Useful | Self::LastUseful)
    }
}

/// Which notes `memory_notes_list` keeps by the language the server detected (`--lang`). Asked of a
/// server that lists `notes-lang` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoteLangFilter {
    /// Notes written in English.
    En,
    /// Notes flagged as not written in English.
    Other,
    /// Notes whose language was not detected yet.
    Unknown,
}

/// Which notes `memory_notes_list` keeps by whether agents found them useful (`--useful`). Asked of
/// a server that lists `notes-useful` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum NoteUsefulFilter {
    /// Notes an agent found useful at least once.
    Ever,
    /// Notes shown at least three times and never found useful.
    Never,
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
    /// True for a team note in a scope routed `remote+local`: it cannot be changed or deleted
    /// from this machine, only hidden here. False from a server that does not say.
    #[serde(default)]
    pub read_only: bool,
    /// True for a team note hidden on this machine (listed only when hidden notes are asked for).
    /// False from a server that does not say.
    #[serde(default)]
    pub hidden: bool,
    /// The language the server detected: `en`, `other` (flagged, it gets a rewrite hint) or
    /// nothing yet. `None` from a server that does not say.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub lang: Option<String>,
    /// How often agents were shown the note. `None` from a server that does not say.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub shown: Option<u32>,
    /// How often an agent found the note useful. `None` from a server that does not say.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub useful: Option<u32>,
    /// RFC 3339: when an agent last found the note useful. `None` when none did yet, or from a
    /// server that does not say.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub last_useful_at: Option<String>,
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
    /// Its notes under the current query, scope and place (every kind, whatever `kind` was asked
    /// for).
    pub count: u32,
}

/// The answer of `memory_notes_list`: one page of notes when available, empty otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryNotesPage {
    /// Whether the notes could be read.
    pub availability: MemoryScriptsAvailability,
    /// How many notes match the query, kind, scope and place (all pages).
    pub total: u32,
    /// The notes of each kind under the query, scope and place, whatever kind was asked for.
    pub kinds: Vec<NoteKindCount>,
    /// The notes of each scope under the query, kind and place; `None` from a server that does
    /// not combine filters.
    pub scopes: Option<MemoryScopeCounts>,
    /// The notes of each place under the query, kind and scope; `None` from a server that does
    /// not filter by place, whose list then keeps every place (a `place` asked for was dropped).
    pub places: Option<MemoryPlaceCounts>,
    /// Whether the server combines `scope` with `kind` (and answered the scope counts): false
    /// from an older one, whose list then follows the query and kind only (a `scope` asked for
    /// was dropped).
    pub combined_filters: bool,
    /// This page, as the server sorts it.
    pub notes: Vec<NoteSummary>,
    /// How many team notes in the view are hidden on this machine; `None` without a team
    /// database or from a server that does not say.
    pub hidden_count: Option<u32>,
    /// How many notes of each scope the local file on this machine holds, every repository
    /// counted; `None` without a team database or from a server that does not say.
    pub local_counts: Option<MemoryScopeCounts>,
    /// The server could not reach the team database and answered from the notes on this
    /// computer instead, so a team's notes are missing from this page: it is not what is kept.
    pub offline: bool,
    /// With `offline`, when this page was last read from the team database and kept on this
    /// computer; the page is that copy. `None` when no copy of this page was kept (the page is
    /// empty) or the page is live.
    pub cached_at: Option<String>,
}
