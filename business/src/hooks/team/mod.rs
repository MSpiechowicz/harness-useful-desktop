//! Team hooks: hooks a teammate shared through the Harness Memory team database, kept on this
//! machine as copies the app window runs (see "Team hooks" in `docs/hooks-format.md`).
//!
//! The desktop never connects to the team database. `harness-memory hooks …` reads and writes the
//! team's hooks ([`crate::ecosystem::memory_hooks`]), and this module keeps what it answered:
//!
//! - [`folder`]: `<hooks root>/team/` (`0700`), one `<id>.json` envelope per team hook (`0600`,
//!   replaced in one rename) and `projects.json`, the repository key of each project root a list
//!   was asked for. A refresh writes what the server listed and removes what it no longer lists for the scopes it
//!   asked for (see [`TeamFolder::apply`]).
//! - [`body`]: the team body rules (a `git:` repository key as `project`, never a `seed`), the
//!   body a local hook is shared as, and the local file a team hook is copied to.
//! - [`views`]: each team hook's status, and the app window's listing that adds the team hooks
//!   after the local ones ([`app_listing`]).
//! - [`actions`]: what the user does to a team hook on this machine (approve, turn off, copy to
//!   the local folder) and the local half of sharing and removing one.
//! - [`sync`]: one refresh round's `hooks list` queries.
//!
//! Only the app window reads the team folder. [`HookStore::list`](super::HookStore::list) (and
//! with it `harness-desktop mcp`) never does, so agents neither see nor edit team hooks, and
//! "Delete all hooks" never touches them. A team hook runs while sharing is on, and only with
//! "Trust team hooks" on or after the user approved its exact bytes (the approvals file of local
//! hooks, keyed by id and SHA-256 of the body). A local hook with the same id wins: the team copy
//! is a duplicate and never runs.

mod actions;
mod body;
mod folder;
mod sync;
mod views;

use serde::{Deserialize, Serialize};

pub use actions::{
    ShareRequest, apply_round, approve_team_hook, clear_team, copy_team_hook, forget_team_hook, record_share,
    set_team_enabled, share_request,
};
pub use body::{parse_team_hook, share_body};
pub use folder::{SyncCounts, TeamFile, TeamFolder};
pub use sync::{TeamFetch, fetch_team, usable_root};
pub use views::{TeamViews, app_listing, team_view, team_views};

/// The folder inside the hooks root that holds the team copies.
pub const TEAM_FOLDER: &str = "team";

/// The root-to-repository-key map inside [`TEAM_FOLDER`].
pub const PROJECTS_FILE: &str = "projects.json";

/// The prefix of a repository key that may be shared (`git:host/owner/repo`).
pub const REPO_KEY_PREFIX: &str = "git:";

/// One team hook as `harness-memory hooks list|get|put` answers it, and as its envelope keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamRow {
    /// The hook id (32 lowercase hex characters), the id of the local hook it was shared from.
    pub id: String,
    /// The repository key of a project hook (`git:host/owner/repo`), `None` for a global one.
    pub repo: Option<String>,
    /// Lowercase SHA-256 of `body`.
    pub sha256: String,
    /// Who shared it (a free-text label).
    pub author: String,
    /// RFC 3339 UTC.
    pub created_at: String,
    /// RFC 3339 UTC.
    pub updated_at: String,
    /// The hook file (version 1) as stored in the team database.
    pub body: String,
}

/// The identity `harness-memory` gives a project root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectKey {
    /// `git:host/owner/repo`, or `path:…` for a repository without an `origin` remote.
    pub key: String,
    /// True when hooks of this project can be shared (a `git:` key).
    pub shareable: bool,
}

impl ProjectKey {
    /// True when the project's hooks may leave the machine: a `git:` key the server calls
    /// shareable.
    pub fn can_share(&self) -> bool {
        self.shareable && self.key.starts_with(REPO_KEY_PREFIX)
    }
}
