//! Copying the notes saved on this computer to the team database (`memory_notes_copy_preview`,
//! `memory_notes_copy_rows`, `memory_notes_copy_apply`) after a note scope moved to the team
//! database, and discarding local notes the server no longer reads
//! (`memory_notes_discard_local`). Scripts moved to the team database are copied the same way
//! ([`MemoryCopyKind`]).
//!
//! The app runs `harness-memory push --json` (`scripts push --json` for scripts, a dry run first)
//! and never opens a database. Ids are those of the local file (`localId`), every repository
//! counted.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::memory_scripts::ScriptScope;

/// What a copy to the team database (or discarding local copies) takes: notes (`push`) or scripts
/// (`scripts push`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MemoryCopyKind {
    /// Notes.
    #[default]
    Notes,
    /// Scripts.
    Scripts,
}

/// A keyword in place of a list of local note ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoteIdsWord {
    /// No note.
    None,
    /// Every note.
    All,
}

/// Which notes saved on this computer a command takes: a keyword, or those local-file ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum LocalNoteIds {
    /// `"none"` or `"all"`.
    Word(NoteIdsWord),
    /// Those ids.
    Ids(#[ts(type = "Array<number>")] Vec<i64>),
}

/// What a copy does, or did, with one local note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NotesCopyAction {
    /// The team database has no such note: it is added.
    Add,
    /// The team note has the same body: only its details change.
    Update,
    /// The local note is newer and differs: it replaces the teammate's text.
    Overwrite,
    /// The local note is newer and differs, and the team note stays as it is.
    Keep,
    /// The team note is the same.
    Identical,
    /// The team note is newer: it stays as it is.
    TeamNewer,
}

/// How many local notes a copy takes each way. A note that fails (or will fail) is counted in
/// `failed` alone, never in another count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesCopyCounts {
    /// Added to the team database.
    pub added: u32,
    /// Same body, details updated.
    pub updated: u32,
    /// Replaced a teammate's text (0 in a preview).
    pub overwritten: u32,
    /// Already the same in the team database.
    pub identical: u32,
    /// Older than the team note, left out.
    pub team_newer: u32,
    /// Newer than a team note that differs (overwritten or kept).
    pub conflicts: u32,
    /// Failed, or will fail (for instance a team note hidden on this computer matches it).
    pub failed: u32,
}

/// One local note of a copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesCopyRow {
    /// The note's id in the local file.
    #[ts(type = "number")]
    pub local_id: i64,
    /// Repository or global.
    #[ts(inline)]
    pub scope: ScriptScope,
    /// The repository key of a repository note.
    pub repo: Option<String>,
    /// One line.
    pub title: String,
    /// What the copy does with it (a conflict is `keep` in a preview).
    pub action: NotesCopyAction,
    /// True when the local note is newer and differs from the team note: it replaces the
    /// teammate's text only when picked.
    pub conflict: bool,
    /// The local note's last change (RFC 3339).
    pub local_updated_at: Option<String>,
    /// The team note's last change (RFC 3339); `None` when there is none.
    pub team_updated_at: Option<String>,
    /// True when it failed, or will fail: it is never copied, never picked to overwrite.
    pub failed: bool,
    /// Why it failed (redacted), as the server says it; `None` when it did not or the server gave
    /// no reason.
    pub failure: Option<String>,
}

/// Which rows of a copy preview to page through.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NotesCopyFilter {
    /// Every row.
    #[default]
    All,
    /// The conflicts that do not fail.
    Conflicts,
    /// The rows that fail.
    Failed,
}

/// One page of the rows of a copy preview, grouped by repository (global notes first) and sorted
/// by title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesCopyRows {
    /// The rows under the filter (all pages).
    pub total: u32,
    /// This page.
    pub rows: Vec<NotesCopyRow>,
}

/// The answer of `memory_notes_copy_preview`: what copying every local note of the scopes routed to
/// the team database would do, kept under `previewId` for paging and for the copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesCopyPreview {
    /// The id `memory_notes_copy_rows` and `memory_notes_copy_apply` take.
    pub preview_id: String,
    /// Why there is nothing to copy, as the server says it; `None` when there is.
    pub nothing: Option<String>,
    /// How many notes go each way.
    pub counts: NotesCopyCounts,
    /// How many rows there are (every local note the copy takes).
    pub total: u32,
    /// The first page of the rows (no filter), as `memory_notes_copy_rows` pages them.
    pub rows: Vec<NotesCopyRow>,
}

/// A local note the copy could not take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesCopyFailure {
    /// The note's id in the local file; `None` for a failure that is no one note's (a relation
    /// that was not pushed, local copies that could not be deleted).
    #[ts(type = "number | null")]
    pub local_id: Option<i64>,
    /// Its title, when the copy listed it (never for a failure without a note).
    pub title: Option<String>,
    /// Why (redacted).
    pub message: String,
}

/// The answer of `memory_notes_copy_apply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesCopyResult {
    /// How many notes went each way (failed ones left out).
    pub counts: NotesCopyCounts,
    /// How many local copies were deleted from this computer (added, updated, overwritten and
    /// identical ones).
    pub deleted: u32,
    /// How many local notes are still on this computer and did not fail (conflicts not picked,
    /// team newer, copied but not deleted); the failed ones are `failed`.
    pub kept: u32,
    /// The notes that could not be copied.
    pub failed: Vec<NotesCopyFailure>,
    /// The local ids of the notes still on this computer that did not fail (conflicts not picked,
    /// team newer, copied but not deleted): what the caller may discard by id, never a failed note
    /// nor one changed on this computer during the copy. Empty when a failure is no one note's.
    #[ts(type = "Array<number>")]
    pub leftover: Vec<i64>,
}
