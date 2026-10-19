//! What the Notes view shows to review a CSV or JSON file of notes before importing it
//! (`memory_notes_import_preview`), and what importing the rows the user kept answers
//! (`memory_notes_import_apply`).
//!
//! The file is read in Rust and kept there with its plan: the webview gets a preview of each row
//! and sends back only row indexes. Problems are codes (with the field they are about), never
//! English text, so the UI words them in its language.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::memory_notes::NoteKind;
use super::memory_scripts::ScriptScope;

/// The shape of an imported file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NotesImportFormat {
    /// A header line, then one note per record (`,`, `;` or tab separated, RFC 4180 quoting).
    Csv,
    /// An array of notes, or an object with one under `notes`.
    Json,
}

/// The field of a row a problem is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NotesImportField {
    /// The note's title.
    Title,
    /// Its text.
    Body,
    /// Its kind.
    Kind,
    /// Its tags.
    Tags,
    /// Its scope.
    Scope,
}

/// What is wrong with a row; a row with any problem cannot be imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NotesImportProblemCode {
    /// The field is empty (a title or a body).
    Missing,
    /// The field is longer than Harness Memory keeps (a title over 120 characters, a body over
    /// 4000, a tag over 32).
    TooLong,
    /// The title has more than one line.
    MultiLine,
    /// More than 8 tags.
    TooMany,
    /// A tag with a character other than `a-z`, `0-9`, `_` and `-` (after lowercasing).
    InvalidChars,
    /// Not one of the note kinds.
    UnknownKind,
    /// Not `repo` (or `repository`, `project`) or `global`.
    UnknownScope,
    /// A repository note, but no project is open.
    NeedsProject,
    /// A JSON value of the wrong type (a number for a title), or a row that is not an object (no
    /// field then).
    WrongType,
}

/// One problem of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesImportProblem {
    /// What is wrong.
    #[ts(inline)]
    pub code: NotesImportProblemCode,
    /// Where, when it is one field.
    #[ts(inline)]
    pub field: Option<NotesImportField>,
}

/// Why a picked file cannot be read as notes at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NotesImportFileProblem {
    /// Over 2 MiB.
    TooLarge,
    /// Not UTF-8 text.
    NotUtf8,
    /// A CSV file without a `title` and a `body` column.
    MissingColumns,
    /// JSON that does not parse.
    InvalidJson,
    /// JSON that is neither an array of notes nor an object with one under `notes`.
    WrongJsonShape,
    /// A CSV record the reader cannot read (see the line).
    InvalidCsv,
    /// A blank file, or one without a single note.
    Empty,
}

/// One note of the file, as it would be imported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesImportRow {
    /// Its position among the file's notes, from 0 (blank CSV records are not counted).
    pub index: u32,
    /// The title, trimmed (cut at 200 characters with `…`).
    pub title: String,
    /// The kind (`note` when the file gives none or one the app does not know).
    pub kind: NoteKind,
    /// The tags, lowercased without duplicates.
    pub tags: Vec<String>,
    /// The scope the file gives; `None` takes the dialog's default scope.
    #[ts(inline)]
    pub scope: Option<ScriptScope>,
    /// The start of the body on one line (at most 200 characters, `…` when cut).
    pub body_preview: String,
    /// Empty when the row can be imported.
    pub problems: Vec<NotesImportProblem>,
    /// The index of the first row with the same title (any case and spacing), when this is a
    /// later one.
    pub duplicate_of: Option<u32>,
}

/// The review of a picked file (`memory_notes_import_preview`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesImportPlan {
    /// Passed back to `memory_notes_import_apply`; good for one import of this project.
    pub plan_id: String,
    /// The picked file's name, without its folder.
    pub file_name: String,
    /// How the file was read.
    #[ts(inline)]
    pub format: NotesImportFormat,
    /// The file has more notes than the 500 shown.
    pub truncated: bool,
    /// The notes, in the file's order.
    pub rows: Vec<NotesImportRow>,
}

/// The answer of `memory_notes_import_preview` for a picked file: its review, or why it cannot
/// be read as notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum NotesImportPreview {
    /// The file's notes, kept in Rust under `plan.planId`.
    Plan {
        /// The review.
        plan: NotesImportPlan,
    },
    /// The file cannot be read as notes; nothing is kept.
    Refused {
        /// The picked file's name, without its folder.
        file_name: String,
        /// Why.
        reason: NotesImportFileProblem,
        /// The line of an `invalidCsv` file, when known (from 1).
        line: Option<u32>,
    },
}

/// Progress of `memory_notes_import_apply`, sent after each row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesImportProgress {
    /// The row just saved (or refused).
    pub index: u32,
    /// How many of the selected rows are done.
    pub done: u32,
    /// How many rows are selected.
    pub total: u32,
}

/// A row not saved because a note of its scope has its title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesImportExisting {
    /// The row.
    pub index: u32,
    /// The note that has the title; `None` when the server's answer does not say.
    #[ts(type = "number | null")]
    pub id: Option<i64>,
}

/// A row the server refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesImportFailure {
    /// The row.
    pub index: u32,
    /// Why, as the server (redacted) or the app says it.
    pub message: String,
}

/// The answer of `memory_notes_import_apply`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesImportResult {
    /// New notes.
    pub created: u32,
    /// Notes of the same title the server rewrote by itself (`memory_note_save`'s `updated`).
    pub updated: u32,
    /// Notes of the same title replaced (`replace` asked for it).
    pub replaced: u32,
    /// Rows skipped for a note of the same title in their scope.
    pub exists: Vec<NotesImportExisting>,
    /// Rows the server refused.
    pub failed: Vec<NotesImportFailure>,
}
