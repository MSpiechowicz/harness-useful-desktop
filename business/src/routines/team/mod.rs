//! Team routines: routines a teammate shared through the Harness Memory team database, kept on
//! this machine as copies only the app window reads (see "Team routines" in
//! `docs/routines-format.md`).
//!
//! The desktop never connects to the team database. `harness-memory routines …` reads and writes
//! the team's routines ([`crate::ecosystem::memory_team_docs`] with [`RoutineKind`]), and
//! [`crate::signed_docs::team`] keeps what it answered in `<routines root>/team/` ([`TeamFolder`]).
//! This module adds what is particular to routines:
//!
//! - Team routines are a catalogue: they arrive off, and run only on a machine that turned them on
//!   (`teamEnabled` in `routines-state.json`, a team project routine with the one folder it runs
//!   in there).
//! - [`views`]: each team routine's status, in this order: Invalid, Duplicate (a local file has
//!   its id), Off (not turned on here, or `enabled: false`), Pending ("Run team routines without
//!   asking" is off and the exact bytes are not approved; trust covers every team routine, one
//!   that runs tools without asking included), Paused (a project routine's folder is missing or
//!   no longer the repository), Active.
//! - [`actions`]: turning one on or off, approving, copying it to the local folder, and the local
//!   halves of sharing and removing one.
//! - [`confirm_team_copy`]: at the start of a run, a team routine that runs on trust alone must
//!   still be in the team database with the bytes of its copy.
//! - Approvals of team copies are kept apart from local routines' (`team:<id>`, see
//!   [`crate::signed_docs::DocKind::SEPARATE_TEAM_APPROVALS`]).
//!
//! Only the app window reads the team folder: [`RoutineStore::list`](super::RoutineStore::list)
//! (and with it `harness-desktop mcp`) never does, and "Delete all routines" never touches it. A
//! copy that leaves the folder takes its approval, approved copy and `teamEnabled` entry with it
//! ([`forget_team_entries`]), unless a local routine has the same id.
//!
//! A refresh round is [`crate::signed_docs::team::fetch_team`] and [`apply_round`] with
//! [`RoutineKind`]; its roots must include the folders team routines run in here
//! ([`super::state::RoutinesState::team_folders`]), so their repository keys stay known.

mod actions;
mod confirm;
mod views;

use std::path::Path;

pub use actions::{
    ShareRequest, approve_team_routine, copy_team_routine, forget_team_routine, record_share, set_team_enabled,
    share_request,
};
pub use confirm::confirm_team_copy;
pub use views::{TeamRoutineViews, app_listing, app_listing_all, team_view, team_views};

pub use crate::signed_docs::team::{ProjectKey, SyncCounts, TeamFetch, TeamFile, TeamRow, apply_round, clear_team};

use super::RoutineKind;
use super::state::{load_state, save_state};
use crate::error::Result;
use crate::signed_docs::state::TeamState;

/// The team folder of one routines root (`<root>/team`).
pub type TeamFolder = crate::signed_docs::team::TeamFolder<RoutineKind>;

/// Drops the `teamEnabled` and `sharedFrom` entries of the team routines `ids` that left this
/// machine (see [`crate::signed_docs::DocKind::forget_team_state`]).
pub(crate) fn forget_team_entries(config_dir: &Path, ids: &[String]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }

    let mut state = load_state(config_dir)?;
    let before = state.clone();

    for id in ids {
        state.forget_team(id);
    }

    if state == before {
        return Ok(());
    }

    save_state(config_dir, &state)
}
