//! What agents may save to Harness Memory: the guards of every chat, of each repository and of
//! every place (`memory_guards_get`, `memory_guards_update`, `memory_chat_guards_set`), and the saves
//! held for the user's review (`memory_held_list`, `memory_held_save`, `memory_held_skip`).
//!
//! The guards live in the server's data folder (`guards/global.json`, `guards/chats/<key>.json`),
//! the held saves in its `held/` folder; see `crate::ecosystem::memory_guards`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use ts_rs::TS;

/// What a chat allows of one kind (notes or scripts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KindGuard {
    /// Agents of the chat may save this kind.
    pub on: bool,
    /// A save of this kind waits for the user's review (`note_save`, `note_update`,
    /// `script_save`; deletes and links pass).
    pub ask: bool,
}

impl Default for KindGuard {
    fn default() -> Self {
        Self { on: true, ask: false }
    }
}

/// The guards of one chat, as in `guards/chats/<key>.json`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryChatGuards {
    /// Nothing of either kind is saved while the chat is paused.
    pub paused: bool,
    /// Notes (`note_*`, `entity_*`).
    pub notes: KindGuard,
    /// Scripts (`script_save`, `script_delete`).
    pub scripts: KindGuard,
}

/// What `memory_chat_guards_set` did with a write of a chat's guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryChatGuardsSet {
    /// The guards were written. False for a late write: one of the same or a higher revision came
    /// first, and its guards stay.
    pub applied: bool,
    /// The last revision written for the chat (this one when applied). After a late write the UI
    /// counts on from above it and writes once more.
    #[ts(type = "number")]
    pub revision: u64,
}

/// The memory identity a chat starts with (`AgentStartRequest.memoryChat`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryChatStart {
    /// The chat's key: a lowercase UUID, kept by the chat across restarts and resumes. The harness
    /// gets it as `HARNESS_MEMORY_SESSION` once the chat's guards file is written (never while
    /// Harness Memory's data folder is unknown).
    pub key: String,
    /// The chat's guards, written before the harness starts.
    pub guards: MemoryChatGuards,
}

/// What every chat allows of one kind, as in `guards/global.json` (version 2). Saving is opt-in:
/// agents save nothing of the kind until the user turns it on for every project or for a
/// repository, and a pause stops it everywhere.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GlobalKindGuard {
    /// Nothing of this kind is saved by any agent, whatever else is on.
    pub paused: bool,
    /// Agents may save this kind in every project.
    pub every_project: bool,
    /// The repositories (absolute saving roots: a git worktree counts as its main repository, see
    /// `memory_saving_root`) where agents may save this kind.
    pub on_repos: Vec<String>,
    /// The project key each entry of `onRepos` had when the user turned it on (`repoKeys` in the
    /// file), by entry: `None` for a folder that has no project, such as the home folder. Harness
    /// Memory saves in a repository only while its key is still the one pinned, and an entry with
    /// no pin turns nothing on. Never sent to the UI.
    #[serde(skip)]
    #[ts(skip)]
    pub repo_keys: BTreeMap<String, Option<String>>,
    /// The entries of `onRepos` that turn nothing on: the folder is gone, its path holds a link,
    /// it is not in its canonical form (`names_itself`), or it has no pin in `repoKeys`. Computed
    /// when the guards are read or changed, never written to the file.
    #[serde(default)]
    pub dead_repos: Vec<String>,
    /// The entries of `onRepos` whose project key is not the one pinned when they were turned on
    /// (the remote of the repository changed): Harness Memory refuses to save there until the user
    /// turns the project on again, which pins the key it has now. Computed by `memory_guards_get`
    /// and `memory_guards_update` with one question to the installed server, empty when the server
    /// cannot be asked, and never written to the file.
    #[serde(default)]
    pub changed_repos: Vec<String>,
}

impl GlobalKindGuard {
    /// The entries of `onRepos` that have a pin: the only ones that can turn anything on.
    pub fn pinned_repos(&self) -> Vec<String> {
        self.on_repos
            .iter()
            .filter(|entry| self.repo_keys.contains_key(*entry))
            .cloned()
            .collect()
    }
}

/// The guards of every chat (`memory_guards_get`, `memory_guards_update`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryGuards {
    /// Notes.
    pub notes: GlobalKindGuard,
    /// Scripts.
    pub scripts: GlobalKindGuard,
    /// The file on disk is still version 1 (saving on, `offRepos`) and could not be moved to version 2
    /// yet: what is shown is its pauses with saving off, and Harness Memory still follows the old file
    /// until the move works. Computed when the guards are read, never written to the file.
    #[serde(default)]
    pub migration_pending: bool,
}

/// The kind of save a guard covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MemoryGuardKind {
    /// Notes.
    Notes,
    /// Scripts.
    Scripts,
}

/// One change to the guards of every chat (`memory_guards_update`), applied to what is saved now:
/// the fields left out stay as they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryGuardsPatch {
    /// The kind changed.
    pub kind: MemoryGuardKind,
    /// Pauses (`true`) or resumes (`false`) the kind everywhere.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub paused: Option<bool>,
    /// Turns saving of the kind on for every project (`true`) or back to the listed repositories
    /// only (`false`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub every_project: Option<bool>,
    /// A repository (any folder in it) where agents may save the kind from now on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub add_repo: Option<String>,
    /// A repository turned on before, off again (as listed or any folder in it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub remove_repo: Option<String>,
}

/// What a held save would save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HeldKind {
    /// A note.
    Note,
    /// A script.
    Script,
}

/// The tool an agent called.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum HeldTool {
    /// A new note (or one over the note of that title).
    NoteSave,
    /// A change to note `arguments.id`.
    NoteUpdate,
    /// A new script, or one over the script of that name.
    ScriptSave,
}

/// One save an agent made while "Ask before saving" was on, waiting for the user
/// (`held/<heldId>.json`). Not saved: not in the store, search, lists or trust.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HeldItem {
    /// The held save's id (a UUID).
    pub held_id: String,
    /// The key of the chat whose agent saved it.
    pub chat_key: String,
    /// When it was held (RFC 3339).
    pub created_at: String,
    /// The MCP client that sent it (`omp`, `opencode`); empty when it did not name itself.
    pub client: String,
    /// Note or script.
    pub kind: HeldKind,
    /// The tool the agent called.
    pub tool: HeldTool,
    /// The project the agent named (`project_path`), else the server's folder; `None` when neither
    /// is known.
    pub project_path: Option<String>,
    /// The git root of that project; `None` outside a repository.
    pub repo_root: Option<String>,
    /// The SHA-256 (lowercase hex) of the held content: `memory_held_save` passes it back, so it
    /// saves only what the user saw. `None` from a server that does not report it.
    pub revision: Option<String>,
    /// The tool arguments, in the server's normal form.
    pub arguments: JsonValue,
    /// For `note_update` the note as saved now (`{id, title, body, kind, tags, entities, scope}`),
    /// for a `script_save` that replaces a script that script (`{name, description, language,
    /// body, timeoutSecs, scope}`), otherwise `null`.
    pub current: JsonValue,
}

/// What `memory_held_save` saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum HeldSaved {
    /// Note `id`.
    Note {
        /// The note's id.
        #[ts(type = "number")]
        id: i64,
        /// True when a held change to a team note was kept on this computer because the team
        /// database could not be reached, and is applied when it is back. False otherwise.
        #[serde(default)]
        queued: bool,
    },
    /// Script `name` (untrusted until the user trusts it).
    Script {
        /// The script's name.
        name: String,
    },
}

/// What `memory_held_listen` sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum MemoryHeldEvent {
    /// The held saves changed: list them again.
    Changed,
}
