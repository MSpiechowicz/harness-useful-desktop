//! The routines folder: listing the files with their status, and writes and deletes by the app
//! window ([`HookAuthor::Desktop`]) or by the MCP server on behalf of an agent
//! ([`HookAuthor::Agent`]).
//!
//! - Only names that are exactly `<id>.json` are read, and only regular files (no symbolic links).
//!   On Unix the whole folder is ignored while group or others can write to it.
//! - Per scope (global, each project) at most [`MAX_ROUTINES_PER_SCOPE`] routines are used:
//!   approved ones first, each group by `createdAt` then `id`; the rest are listed as invalid.
//! - Every write sets `updatedBy` to its author (a new file gets it as `createdBy` too) and keeps
//!   `createdAt`, `createdBy` and unknown top-level fields. An edit or an agent's delete names the
//!   SHA-256 of the version it changes, checked again right before the rename.
//! - Only the app window approves: its writes are approved at once and it never saves over a
//!   pending file. An agent's write stays pending until the user approves that exact version, and
//!   an agent removes only routines an agent created.
//!
//! The folder, the approvals and the checks every signed document shares are
//! [`DocStore`](crate::signed_docs::DocStore)'s ([`RoutineStore`] is that store for routines).
//! Deleting a routine's run records and schedule entry is the app window's
//! ([`super::RunStore::delete`], [`super::ScheduleState::forget`]).

use std::path::PathBuf;
use std::time::SystemTime;

use super::RoutineKind;
use super::draft::draft_routine;
use super::view::{counts_of, local_views, view_of};
use crate::error::{BusinessError, Result};
use crate::model::{HookAuthor, Routine, RoutineDraft, RoutineListing, RoutineView};
use crate::signed_docs::scope::FileOwner;
use crate::signed_docs::{DocStore, check_id};

/// Most routines used per scope (global, and each project separately).
pub const MAX_ROUTINES_PER_SCOPE: usize = 64;

/// The routines folder (`<root>/routines`, the root from [`super::routines_root`] with
/// [`DocStore::from_env`]) and the app config folder that holds the approvals.
pub type RoutineStore = DocStore<RoutineKind>;

impl RoutineStore {
    /// `<root>/routines`, where the files are.
    pub fn routines_dir(&self) -> PathBuf {
        self.docs_dir()
    }

    /// Every routine file with its status (see [`RoutineListing`]). A missing folder lists
    /// nothing. Reads the app config folder, never writes it, and never the team copies (see
    /// [`super::team::app_listing`]).
    pub fn list(&self) -> RoutineListing {
        self.listing(|_| true)
    }

    /// The routine files a listing for `project` (a project root; `None`: global only) shows: the
    /// global routines, that project's (those over the limit included), and every file that is
    /// not a valid routine, since its scope is unknown.
    pub fn list_for(&self, project: Option<&str>) -> RoutineListing {
        self.listing(|owner| owner.shown_in(project))
    }

    fn listing(&self, keep: impl Fn(&FileOwner) -> bool) -> RoutineListing {
        let (routines, folder_problem) = match self.sorted_files() {
            Ok((sorted, approvals)) => {
                let approved = |id: &str| self.approved_version(&approvals, id);

                let routines = local_views(sorted, &approvals, &approved)
                    .into_iter()
                    .filter(|(_, owner)| keep(owner))
                    .map(|(view, owner)| RoutineView {
                        root: owner.root(),
                        ..view
                    })
                    .collect();

                (routines, None)
            }
            Err(problem) => (Vec::new(), Some(problem)),
        };

        RoutineListing {
            counts: counts_of(&routines),
            routines,
            folder_problem,
            team: None,
        }
    }

    /// The routine file `id` as [`RoutineStore::list`] shows it. `NotFound` when there is none,
    /// `Forbidden` when the folder is not read.
    pub fn get(&self, id: &str) -> Result<RoutineView> {
        check_id::<RoutineKind>(id)?;

        let listing = self.list();

        if let Some(problem) = listing.folder_problem {
            return Err(BusinessError::Forbidden(problem));
        }

        listing
            .routines
            .into_iter()
            .find(|view| view.id == id)
            .ok_or_else(|| BusinessError::NotFound("the routine".to_owned()))
    }

    /// The approved earlier version of routine `id` (the copy kept when it was approved), while
    /// its approval still names that copy.
    pub fn approved_snapshot(&self, id: &str) -> Option<Routine> {
        self.approved_version(&self.read_approvals(), id)
    }

    /// Creates or edits a routine as `author`. The app window's write is approved at once; an
    /// agent's is pending. `InvalidInput` for a draft that breaks the format, a 65th routine in a
    /// scope, an edit without `expectedSha256` or an app window edit of a pending file (approve or
    /// reject it first), `Conflict` when the edited file changed since `expectedSha256`,
    /// `NotFound` for an edit of a missing file or a missing project folder, `Forbidden` when the
    /// folder is writable by other users. The app window's write fails before it changes anything
    /// when the approvals cannot be read.
    pub fn save(&self, draft: &RoutineDraft, author: HookAuthor, now: SystemTime) -> Result<RoutineView> {
        let saved = self.save_doc(
            draft.id.as_deref(),
            draft.expected_sha256.as_deref(),
            author,
            |id, base, previous| draft_routine(draft, id, author, base, previous, now),
        )?;

        let approved = |id: &str| self.approved_version(&saved.approvals, id);

        Ok(view_of(saved.doc, saved.sha256, &saved.approvals, &approved))
    }

    /// Deletes the routine file `id` as `author`. The app window deletes any file (valid or not)
    /// and drops its approval and approved copy; `expected_sha256` is optional for it, and a
    /// missing file is not an error. An agent must name the current version (`Conflict`
    /// otherwise, `NotFound` for a missing file) and may remove only a valid routine with
    /// `createdBy: "agent"` (`Forbidden` otherwise: the user decides).
    pub fn delete(&self, id: &str, author: HookAuthor, expected_sha256: Option<&str>) -> Result<()> {
        self.delete_as(id, author, expected_sha256)
    }

    /// The user approves the current bytes of routine `id` (app window only). `Conflict` when they
    /// are not the version the user reviewed (`expected_sha256`), `InvalidInput` for an invalid
    /// file, `NotFound` for a missing one.
    pub fn approve(&self, id: &str, expected_sha256: &str) -> Result<()> {
        self.approve_file(id, expected_sha256)
    }

    /// The user rejects the pending version of routine `id` (`expected_sha256`, app window only):
    /// a changed routine gets its approved version back (approved again at once), a never-approved
    /// one is deleted. Returns the restored routine's view, `None` after a delete. `Conflict` when
    /// the file changed since it was shown, `NotFound` when there is no file or no kept copy of the
    /// approved version, `InvalidInput` for an approved routine.
    pub fn reject(&self, id: &str, expected_sha256: &str) -> Result<Option<RoutineView>> {
        if self.reject_pending(id, expected_sha256)? {
            self.get(id).map(Some)
        } else {
            Ok(None)
        }
    }

    /// "Delete all routines" for `project` (a project root; `None`: global only): every file
    /// [`RoutineStore::list_for`] shows, with its approval and approved copy. Team routines and
    /// another project's routines are never touched. Approvals that cannot be read stop it before
    /// anything is deleted, a folder that is not read is `Forbidden`. Returns the ids deleted (their
    /// run records and schedule entries go too).
    pub fn delete_all(&self, project: Option<&str>) -> Result<Vec<String>> {
        self.delete_all_for(project)
    }
}
