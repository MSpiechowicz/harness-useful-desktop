//! The shared team database of Harness Memory (`memory_remote_get`, `memory_remote_set`,
//! `memory_remote_test`): the `database_url`, `database_token`, note, script and hook routing in
//! the server's own `config.toml`.
//!
//! The URL and the token are secrets. They travel from the webview to the app only; the app returns
//! a redacted URL (`scheme://user@host:port/path`) and whether a token is saved, never either value.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The kind of team database, picked by the URL scheme.
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
}

/// Where the server keeps one scope of notes while a team database is set.
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

/// The team database settings as saved in Harness Memory's `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryRemote {
    /// True when the file holds a `database_url`.
    pub enabled: bool,
    /// The kind the saved URL's scheme names; `None` without a URL or for a scheme the server does
    /// not support.
    pub kind: Option<MemoryRemoteKind>,
    /// The saved URL without its password, query and fragment (`scheme://user@host:port/path`).
    pub url_redacted: Option<String>,
    /// True when the file holds a non-empty `database_token`.
    pub has_token: bool,
    /// Where repository notes go (`repository_notes`; `remote` when unset).
    pub repository_notes: MemoryNotesTarget,
    /// Where global notes go (`global_notes`; `remote` when unset).
    pub global_notes: MemoryNotesTarget,
    /// Where scripts go (`scripts`; `remote` when unset).
    pub scripts: MemoryNotesTarget,
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
