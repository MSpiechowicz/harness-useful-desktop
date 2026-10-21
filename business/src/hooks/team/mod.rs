//! Team hooks: hooks a teammate shared through the Harness Memory team database, kept on this
//! machine as copies the app window runs (see "Team hooks" in `docs/hooks-format.md`).
//!
//! The desktop never connects to the team database. `harness-memory hooks …` reads and writes the
//! team's hooks ([`crate::ecosystem::memory_hooks`]), and [`crate::signed_docs::team`] keeps what
//! it answered: `<hooks root>/team/` ([`TeamFolder`]) with one envelope per team hook and
//! `projects.json`, written by a refresh round ([`fetch_team`], [`apply_round`]). This module adds
//! what is particular to hooks:
//!
//! - [`body`]: the team body rules (a `git:` repository key as `project`, never a `seed`), the
//!   body a local hook is shared as, and the local file a team hook is copied to.
//! - [`views`]: each team hook's status, and the app window's listing that adds the team hooks
//!   after the local ones ([`app_listing`]).
//! - [`actions`]: what the user does to a team hook on this machine (approve, turn off, copy to
//!   the local folder) and the local half of sharing and removing one.
//!
//! Only the app window reads the team folder. [`HookStore::list`](super::HookStore::list) (and
//! with it `harness-desktop mcp`) never does, so agents neither see nor edit team hooks, and
//! "Delete all hooks" never touches them. A team hook runs while sharing is on, and only with
//! "Trust team hooks" on or after the user approved its exact bytes (the approvals file of local
//! hooks, keyed by id and SHA-256 of the body). A local hook with the same id wins: the team copy
//! is a duplicate and never runs.

mod actions;
mod body;
mod views;

use std::path::Path;

pub use actions::{
    ShareRequest, approve_team_hook, copy_team_hook, forget_team_hook, record_share, set_team_enabled, share_request,
};
pub use body::{parse_team_hook, share_body};
pub use views::{TeamViews, app_listing, app_listing_all, team_view, team_views};

pub use crate::signed_docs::team::{
    PROJECTS_FILE, ProjectKey, REPO_KEY_PREFIX, SyncCounts, TEAM_FOLDER, TeamFetch, TeamFile, TeamRow, apply_round,
    clear_team, usable_root,
};

use super::HookKind;
use crate::env::Env;
use crate::process::ProcessRunner;

/// The team folder of one hooks root (`<root>/team`).
pub type TeamFolder = crate::signed_docs::team::TeamFolder<HookKind>;

/// Lists the team hooks on `server`: the global ones, then those of each of `roots` (see
/// [`crate::signed_docs::team::fetch_team`]).
pub async fn fetch_team(env: &Env, runner: &dyn ProcessRunner, server: &Path, roots: &[String]) -> TeamFetch {
    crate::signed_docs::team::fetch_team::<HookKind>(env, runner, server, roots).await
}
