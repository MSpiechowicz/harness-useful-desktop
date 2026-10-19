//! What the Notes and Scripts views send to create or edit a note or a script in Harness Memory
//! (`memory_note_save`, `memory_script_save`), and what saving it answers.
//!
//! The app saves them through `harness-memory mcp` (`note_save`, `note_update`, `script_save`)
//! and never opens the memory database itself.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::memory_notes::NoteKind;
use super::memory_scripts::{ScriptScope, ScriptSummary};

/// A note to create, or the new fields of one to edit (`memory_note_save`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NoteDraft {
    /// One line, 1 to 120 characters.
    pub title: String,
    /// 1 to 4000 characters.
    pub body: String,
    /// What it records.
    pub kind: NoteKind,
    /// At most 8, each 1 to 32 characters of `a-z`, `0-9`, `_` and `-` (lowercased, duplicates
    /// dropped).
    pub tags: Vec<String>,
    /// Files, modules, components or concepts the note is about: at most 10, each one line of 1
    /// to 80 characters (duplicates dropped). Sent only when creating a note: the server's
    /// `note_update` cannot change them, so an edit keeps the note's own.
    #[serde(default)]
    pub entities: Vec<String>,
    /// Repository (needs a project) or global. Kept as it is on an edit.
    #[ts(inline)]
    pub scope: ScriptScope,
}

/// What became of a note sent to `memory_note_save`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum NoteSaveOutcome {
    /// A new note.
    Created,
    /// An existing note was rewritten (the one edited, or a global note of the same title).
    Updated,
    /// Another note of the project's repository has this title: nothing was saved.
    Exists,
}

/// The answer of `memory_note_save`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NoteSaved {
    /// Created, updated, or refused for its title.
    #[ts(inline)]
    pub outcome: NoteSaveOutcome,
    /// The note saved, or for `exists` the note that has the title; `None` when the server's
    /// answer does not say.
    #[ts(type = "number | null")]
    pub id: Option<i64>,
}

/// The interpreters Harness Memory runs a script with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScriptLanguage {
    /// POSIX `sh`.
    Sh,
    /// Bash.
    Bash,
    /// Python 3.
    Python,
    /// Node.js.
    Node,
    /// Bun.
    Bun,
}

/// A script to create, or the new fields of one to edit (`memory_script_save`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScriptDraft {
    /// Unique per scope: `a-z` or `0-9`, then up to 63 of `a-z`, `0-9`, `_` and `-`. Kept as it
    /// is on an edit.
    pub name: String,
    /// 1 to 500 characters.
    pub description: String,
    /// The interpreter.
    pub language: ScriptLanguage,
    /// The source code, not blank, at most 64 KiB.
    pub body: String,
    /// The run time limit, 1 to 600 seconds.
    pub timeout_secs: u32,
    /// Repository (needs a project) or global. Kept as it is on an edit.
    #[ts(inline)]
    pub scope: ScriptScope,
}

/// What became of a script sent to `memory_script_save`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ScriptSaveOutcome {
    /// A new script.
    Created,
    /// The script of that name in that scope was replaced.
    Replaced,
    /// A script of that name exists in that scope and replacing it was not asked for: nothing
    /// was saved.
    Exists,
}

/// The answer of `memory_script_save`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScriptSaved {
    /// Created, replaced, or refused for its name.
    #[ts(inline)]
    pub outcome: ScriptSaveOutcome,
    /// The script saved, or for `exists` the script that has the name.
    #[ts(type = "number")]
    pub id: i64,
    /// The script as saved; `None` for `exists`.
    pub script: Option<ScriptSummary>,
}
