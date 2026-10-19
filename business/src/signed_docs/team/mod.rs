//! Team copies of signed documents: documents a teammate shared through the Harness Memory team
//! database, kept on this machine as copies only the app window reads (see "Team hooks" in
//! `docs/hooks-format.md`; every kind follows it with its own names).
//!
//! The desktop never connects to the team database. `harness-memory <plural> …` reads and writes
//! the team's documents ([`crate::ecosystem::memory_team_docs`]), and this module keeps what it
//! answered:
//!
//! - [`folder`]: `<root>/team/` (`0700`), one `<id>.json` envelope per team document (`0600`,
//!   replaced in one rename) and `projects.json`, the repository key of each project root a list
//!   was asked for. A refresh writes what the server listed and removes what it no longer lists
//!   for the scopes it asked for ([`TeamFolder::apply`]).
//! - [`envelope`]: an envelope's content, the checks of a team body against its row, and a body
//!   with another `project`.
//! - [`sync`]: one refresh round's lists ([`fetch_team`]) and how a round is written with the
//!   approvals of what left ([`apply_round`], [`clear_team`]).
//! - [`actions`]: the local halves of sharing, removing and copying a team document, and
//!   [`entries`] the copies as each kind's team listing starts from.

pub mod actions;
pub(crate) mod entries;
pub mod envelope;
pub mod folder;
pub mod sync;

use serde::{Deserialize, Serialize};

pub use actions::{ShareRequest, copy_team_doc, forget_team_doc, record_share, share_body, share_request};
pub use envelope::{check_repo, checked_doc, with_project};
pub use folder::{SyncCounts, TeamFile, TeamFolder};
pub use sync::{TeamFetch, apply_round, clear_team, fetch_team, usable_root};

use super::fields::MAX_NAME_CHARS;
use super::unicode::is_hidden_char;

/// The folder inside a kind's root that holds the team copies.
pub const TEAM_FOLDER: &str = "team";

/// The root-to-repository-key map inside [`TEAM_FOLDER`].
pub const PROJECTS_FILE: &str = "projects.json";

/// The prefix of a repository key that may be shared (`git:host/owner/repo`).
pub const REPO_KEY_PREFIX: &str = "git:";

/// One team document as `harness-memory <plural> list|get|put` answers it, and as its envelope
/// keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamRow {
    /// The document id (32 lowercase hex characters), the id of the local document it was shared
    /// from.
    pub id: String,
    /// The repository key of a project document (`git:host/owner/repo`), `None` for a global one.
    pub repo: Option<String>,
    /// Lowercase SHA-256 of `body`.
    pub sha256: String,
    /// Who shared it (a free-text label).
    pub author: String,
    /// RFC 3339 UTC.
    pub created_at: String,
    /// RFC 3339 UTC.
    pub updated_at: String,
    /// The document file (version 1) as stored in the team database.
    pub body: String,
}

/// The identity `harness-memory` gives a project root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectKey {
    /// `git:host/owner/repo`, or `path:…` for a repository without an `origin` remote.
    pub key: String,
    /// True when documents of this project can be shared (a `git:` key).
    pub shareable: bool,
}

impl ProjectKey {
    /// True when the project's documents may leave the machine: a `git:` key the server calls
    /// shareable.
    pub fn can_share(&self) -> bool {
        self.shareable && self.key.starts_with(REPO_KEY_PREFIX)
    }
}

/// `author` (a free-text label from the team database) without control or invisible characters,
/// at most [`MAX_NAME_CHARS`] long.
pub(crate) fn author_label(author: &str) -> String {
    author
        .chars()
        .filter(|character| !character.is_control() && !is_hidden_char(*character))
        .take(MAX_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_owned()
}
