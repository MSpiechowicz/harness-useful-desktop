//! The shared team database of Harness Memory (`memory_remote_get`, `memory_remote_set`,
//! `memory_remote_test`): the `database_url`, `database_token`, note, script and hook routing in
//! the server's own `config.toml`.
//!
//! The URL and the token are secrets. They travel from the webview to the app only; the app returns
//! a redacted URL (`scheme://user@host:port/path`) and whether a token is saved, never either value.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The kind of team database, picked by the URL scheme (and, for a `mongodb://` URL, by the
/// `database_kind` key or the host).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MemoryRemoteKind {
    /// PostgreSQL: `postgres://` or `postgresql://`.
    Postgres,
    /// libSQL (remote SQLite, e.g. Turso or sqld): `libsql://`, `https://`, or `http://` for this
    /// machine only. Its auth token is kept apart from the URL.
    Libsql,
    /// MongoDB: `mongodb://` or `mongodb+srv://`.
    Mongodb,
    /// Amazon DocumentDB: `mongodb://` or `mongodb+srv://`, saved with `database_kind =
    /// "documentdb"`. A saved URL without that key is DocumentDB when a host ends in
    /// `.docdb.amazonaws.com` or `.docdb-elastic.amazonaws.com` (or their `.com.cn` forms).
    Documentdb,
}

/// Where the server keeps hooks or routines while a team database is set (notes and scripts take
/// [`MemoryNotesRouting`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MemoryNotesTarget {
    /// In the team database, shared with everyone using it (the default).
    #[default]
    Remote,
    /// In the local SQLite file on this machine only.
    Local,
}

/// Where the server keeps one scope of notes (`repository_notes`, `global_notes`) or the scripts
/// (`scripts`) while a team database is set. Hooks and routines take [`MemoryNotesTarget`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MemoryNotesRouting {
    /// In the team database, shared with everyone using it (the default).
    #[default]
    Remote,
    /// In the local SQLite file on this machine only.
    Local,
    /// Read from the team database and this machine, new notes or scripts saved on this machine
    /// (`remote+local`). Team notes or scripts are read-only there.
    RemoteLocal,
}

/// The team database settings as saved in Harness Memory's `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryRemote {
    /// True when the file holds a `database_url`.
    pub enabled: bool,
    /// The kind of the saved URL: its scheme, and for a `mongodb://` URL the `database_kind` key
    /// or, without it, the host (see [`MemoryRemoteKind::Documentdb`]); `None` without a URL or for
    /// a scheme the server does not support.
    pub kind: Option<MemoryRemoteKind>,
    /// The saved URL without its password, query and fragment (`scheme://user@host:port/path`).
    pub url_redacted: Option<String>,
    /// True when the file holds a non-empty `database_token`.
    pub has_token: bool,
    /// Where repository notes go (`repository_notes`; `remote` when unset).
    pub repository_notes: MemoryNotesRouting,
    /// Where global notes go (`global_notes`; `remote` when unset).
    pub global_notes: MemoryNotesRouting,
    /// Where scripts go (`scripts`; `remote` when unset).
    pub scripts: MemoryNotesRouting,
    /// True when the installed Harness Memory keeps scripts (its tools include `script_list`), so
    /// the `scripts` key may be saved. Older servers refuse a file with that key.
    pub scripts_supported: bool,
    /// True when `trust_team_scripts = true`: every script read from the team database runs without
    /// asking.
    pub trust_team_scripts: bool,
    /// True when the installed Harness Memory asks the user before running a script it has not
    /// approved (its `script_run` takes no `sha`), so the `trust_team_scripts` key may be saved.
    /// Older servers refuse a file with that key.
    pub trust_team_scripts_supported: bool,
    /// True when the installed Harness Memory keeps team hooks (`harness-memory hooks config`
    /// answers), so the `hooks` and `trust_team_hooks` keys may be saved. Older servers refuse a
    /// file with those keys.
    pub hooks_supported: bool,
    /// True when `hooks = "remote"`: hooks the user shares go to the team database, and this
    /// machine runs the team's hooks.
    pub share_hooks: bool,
    /// True when `trust_team_hooks = true`: team hooks run without local approval.
    pub trust_team_hooks: bool,
    /// True when the installed Harness Memory keeps team routines (`harness-memory routines config`
    /// answers), so the `routines` and `trust_team_routines` keys may be saved. Older servers refuse
    /// a file with those keys.
    pub routines_supported: bool,
    /// True when `routines = "remote"`: routines the user shares go to the team database, and this
    /// machine lists the team's routines (each runs only once turned on here).
    pub share_routines: bool,
    /// True when `trust_team_routines = true`: every team routine turned on here runs without local
    /// approval, commands and agent routines that run tools without asking included.
    pub trust_team_routines: bool,
    /// True when the installed Harness Memory can use Amazon DocumentDB as the team database
    /// (`harness-memory kinds` lists `documentdb`), so `kind` may be `documentdb` and the
    /// `database_kind` key may be saved. Older servers refuse a file with that key.
    pub documentdb_supported: bool,
    /// True when the installed Harness Memory lists `notes-remote-local`, `notes-hide` and
    /// `push-json` in `harness-memory features`, so a note scope may be saved as `remoteLocal`,
    /// team notes hidden on this machine and local notes copied to the team database.
    pub team_and_local_supported: bool,
    /// True when the installed Harness Memory lists `scripts-remote-local`, `scripts-hide` and
    /// `scripts-push-json` in `harness-memory features`, so scripts may be saved as `remoteLocal`,
    /// team scripts hidden on this machine and local scripts copied to the team database.
    pub scripts_team_and_local_supported: bool,
    /// True when the installed Harness Memory lists `save-guards` in `harness-memory features`: it
    /// applies the guards of each chat, repository and place, holds saves for review, and saves
    /// the user's own notes and scripts through commands agents cannot reach.
    pub save_guards_supported: bool,
    /// True when the installed Harness Memory lists `embedder` in `harness-memory features`: it
    /// runs the local embedder behind vector search.
    pub embedder_supported: bool,
    /// True when the installed Harness Memory lists `settings` in `harness-memory features`: it
    /// has `settings get|set` (the English notes setting).
    pub settings_supported: bool,
    /// True when the installed Harness Memory lists `notes-lang` in `harness-memory features`:
    /// notes carry a `lang` and `notes list` filters by it.
    pub notes_lang_supported: bool,
    /// The config file, home-relative for display (`~/.config/harness-memory/config.toml`).
    pub config_path: String,
    /// What is wrong with the file, if anything: unreadable, not valid TOML, keys the server does
    /// not know, a value of the wrong type, or permissions that let other users read or change it.
    pub problem: Option<String>,
}

/// The outcome of `harness-memory check` against a team database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryRemoteTest {
    /// True when the server reached the database.
    pub ok: bool,
    /// The server's one-line answer (redacted), or why it could not run.
    pub message: String,
}
