//! The team database of Harness Knowledge (`knowledge_remote_get`, `knowledge_remote_set`,
//! `knowledge_remote_test`): the `[database]` table with `url` and `token` in the server's own
//! `config.toml`, and what `harness-knowledge check --json` says about a database.
//!
//! The URL and the token are secrets, as for Harness Memory: they travel from the webview to the
//! app only, and the app returns a redacted URL and whether a token is saved.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::MemoryRemoteKind;

/// The team database settings as saved in Harness Knowledge's `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeRemote {
    /// True when the file holds a `[database]` `url`.
    pub enabled: bool,
    /// The kind of the saved URL, by its scheme (never DocumentDB: it is not supported); `None`
    /// without a URL or for a scheme the server does not support.
    pub kind: Option<MemoryRemoteKind>,
    /// The saved URL without its password, query and fragment (`scheme://user@host:port/path`).
    pub url_redacted: Option<String>,
    /// True when the file holds a non-empty `token`.
    pub has_token: bool,
    /// True when the installed Harness Knowledge lists `embedder` in `harness-knowledge features`.
    pub embedder_supported: bool,
    /// The config file, home-relative for display (`~/.config/harness-knowledge/config.toml`).
    pub config_path: String,
    /// What is wrong with the file, if anything: unreadable, not valid TOML, keys the server does
    /// not know, a value of the wrong type, or permissions that let other users read or change it.
    pub problem: Option<String>,
}

/// What the database lacks, as `check --json` names it (`problem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeProblem {
    /// MongoDB without `mongot`: no `$search` and `$vectorSearch` (use Atlas or MongoDB 8.2+).
    NoMongot,
    /// PostgreSQL without the `vector` extension.
    NoPgvector,
    /// The extension exists, but the database user may not create it.
    PgvectorNoPrivilege,
    /// A pgvector older than the server needs.
    PgvectorTooOld,
    /// A libSQL server without vector support.
    LibsqlNoVector,
    /// A SQLite or libSQL without FTS5 full-text search.
    NoFts5,
    /// A search index is still building: try again in a moment.
    IndexBuilding,
    /// The server could not be reached.
    Unreachable,
    /// The server refused the credentials.
    Auth,
    /// A problem code this app does not know: `message` says what it is.
    Other,
}

/// How far the vector index is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeIndexState {
    /// It answers.
    Ready,
    /// It is still building.
    Building,
    /// It does not exist or cannot work.
    Missing,
}

/// What the database can do, as `check --json` probed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeCapabilities {
    /// Keyword search works.
    pub keyword: bool,
    /// Vector search works.
    pub vector: bool,
    /// How far the vector index is.
    pub vector_index: KnowledgeIndexState,
}

/// The outcome of `harness-knowledge check` against a database (`knowledge_remote_test`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeCheck {
    /// True when the database has everything Harness Knowledge needs.
    pub ok: bool,
    /// What the server answered for (`sqlite`, `postgres`, `mongodb`, `libsql`), none when it
    /// could not be run.
    pub backend: Option<String>,
    /// What is missing, when something is.
    pub problem: Option<KnowledgeProblem>,
    /// The server's sentence about it (redacted), or why it could not run.
    pub message: Option<String>,
    /// What works, none when the server could not be run.
    pub capabilities: Option<KnowledgeCapabilities>,
    /// The model that embedded the chunks already stored, once any were.
    pub model: Option<String>,
}
