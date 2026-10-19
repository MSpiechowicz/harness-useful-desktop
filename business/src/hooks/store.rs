//! The hooks folder: listing the files with their status, and writes and deletes by the app
//! window ([`HookAuthor::Desktop`]) or by the MCP server on behalf of an agent
//! ([`HookAuthor::Agent`]).
//!
//! - Only names that are exactly `<id>.json` are read, so temp files, leftovers and editor files
//!   are ignored, and so is anything that is not a regular file (symbolic links included). On Unix
//!   the whole folder is ignored while group or others can write to it.
//! - Per scope (global, each project) at most [`MAX_HOOKS_PER_SCOPE`] hooks are used: approved
//!   ones first, each group by `createdAt` then `id`; the rest are listed as invalid. The app
//!   window refuses a new hook when a scope holds the limit of approved hooks, an agent when it
//!   holds the limit of valid ones.
//! - Every write sets `updatedBy` to its author (a new file gets it as `createdBy` too) and keeps
//!   `createdAt`, `createdBy`, `seed`, `requires` and unknown top-level fields of the file it
//!   rewrites. An edit or an agent's delete names the SHA-256 of the version it changes.
//! - Only the app window approves: its writes store the approval and a copy of the approved bytes
//!   with the file, so they run at once. It never saves over a pending file (the user approves or
//!   rejects it first). An agent's write never touches the app config folder, so it stays pending
//!   until the user approves that exact version, and an agent removes only hooks an agent created.
//! - Each write checks again, right before its rename, that the file still holds the version it
//!   read, and fails with a conflict otherwise. This is best effort, not a compare-and-swap across
//!   processes: a write that lands between the check and the rename is replaced. It fails closed,
//!   since only the exact approved bytes run: the worst case is a lost edit or a pending hook.
//!
//! The folder, the approvals and the checks every signed document shares are
//! [`DocStore`](crate::signed_docs::DocStore)'s ([`HookStore`] is that store for hooks).

use std::path::PathBuf;
use std::time::SystemTime;

use serde_json::Map;

use super::HookKind;
use super::draft::draft_hook;
use super::view::{FileOwner, view_of, views};
use crate::error::{BusinessError, Result};
use crate::model::{Hook, HookAuthor, HookDraft, HookListing, HookView};
use crate::signed_docs::{DocStore, changed_since_read, check_id, not_found};

pub use super::view::{HookCapabilities, MAX_HOOKS_PER_SCOPE};

/// The hooks folder (`<root>/hooks`, the root from [`super::hooks_root`] with
/// [`DocStore::from_env`]) and the app config folder that holds the approvals.
pub type HookStore = DocStore<HookKind>;

impl HookStore {
    /// `<root>/hooks`, where the files are.
    pub fn hooks_dir(&self) -> PathBuf {
        self.docs_dir()
    }

    /// Every hook file with its status (see [`HookListing`]). A missing folder lists nothing.
    /// Reads the app config folder, never writes it, and never the team copies (see
    /// [`super::team::app_listing`] for the app window's listing with them).
    pub fn list(&self, capabilities: HookCapabilities) -> HookListing {
        self.listing(capabilities, |_| true)
    }

    /// The hook files a chat in `project` (a project root; `None`: global only) sees, as
    /// [`HookStore::list`] shows them: the global hooks, that project's (those over the limit
    /// included), and every file that is not a valid hook, since its scope is unknown.
    pub fn list_for(&self, capabilities: HookCapabilities, project: Option<&str>) -> HookListing {
        self.listing(capabilities, |owner| owner.shown_in(project))
    }

    fn listing(&self, capabilities: HookCapabilities, keep: impl Fn(&FileOwner) -> bool) -> HookListing {
        match self.read_files() {
            Ok(files) => {
                let approvals = self.read_approvals();
                let approved = |id: &str| self.approved_version(&approvals, id);

                let hooks = views(files, &approvals, capabilities, &approved)
                    .into_iter()
                    .filter(|(_, owner)| keep(owner))
                    .map(|(view, _)| view)
                    .collect();

                HookListing {
                    hooks,
                    folder_problem: None,
                    team: None,
                }
            }
            Err(problem) => HookListing {
                hooks: Vec::new(),
                folder_problem: Some(problem),
                team: None,
            },
        }
    }

    /// The hook file `id` as [`HookStore::list`] shows it. `NotFound` when there is none,
    /// `Forbidden` when the folder is not read.
    pub fn get(&self, id: &str, capabilities: HookCapabilities) -> Result<HookView> {
        check_id::<HookKind>(id)?;

        let listing = self.list(capabilities);

        if let Some(problem) = listing.folder_problem {
            return Err(BusinessError::Forbidden(problem));
        }

        listing
            .hooks
            .into_iter()
            .find(|view| view.id == id)
            .ok_or_else(|| BusinessError::NotFound("the hook".to_owned()))
    }

    /// The approved earlier version of hook `id` (the copy kept when it was approved), while its
    /// approval still names that copy.
    pub fn approved_snapshot(&self, id: &str) -> Option<Hook> {
        self.approved_version(&self.read_approvals(), id)
    }

    /// Creates or edits a hook as `author`. The app window's write is approved at once; an
    /// agent's is pending. `InvalidInput` for a draft that breaks the format, a 65th hook in a
    /// scope, an edit without `expectedSha256` or an app window edit of a pending file (approve or
    /// reject it first), `Conflict` when the edited file changed since `expectedSha256`,
    /// `NotFound` for an edit of a missing file or a missing project folder, `Forbidden` when the
    /// folder is writable by other users. The app window's write fails before it changes anything
    /// when the approvals cannot be read.
    pub fn save(
        &self,
        draft: &HookDraft,
        author: HookAuthor,
        capabilities: HookCapabilities,
        now: SystemTime,
    ) -> Result<HookView> {
        let saved = self.save_doc(
            draft.id.as_deref(),
            draft.expected_sha256.as_deref(),
            author,
            |id, base, previous| draft_hook(draft, id, author, base, previous, now),
        )?;

        let approved = |id: &str| self.approved_version(&saved.approvals, id);

        Ok(view_of(
            saved.doc,
            saved.sha256,
            &saved.approvals,
            capabilities,
            &approved,
        ))
    }

    /// Deletes the hook file `id` as `author`. The app window deletes any file (valid or not) and
    /// drops its approval; `expected_sha256` is optional for it, and a missing file is not an
    /// error. An agent must name the current version (`Conflict` otherwise, `NotFound` for a
    /// missing file) and may remove only a valid hook with `createdBy: "agent"` (`Forbidden`
    /// otherwise: the user decides).
    pub fn delete(&self, id: &str, author: HookAuthor, expected_sha256: Option<&str>) -> Result<()> {
        self.delete_as(id, author, expected_sha256)
    }

    /// The user approves the current bytes of hook `id` (app window only). `Conflict` when they
    /// are not the version the user reviewed (`expected_sha256`), `InvalidInput` for an invalid
    /// file, `NotFound` for a missing one.
    pub fn approve(&self, id: &str, expected_sha256: &str) -> Result<()> {
        self.approve_file(id, expected_sha256)
    }

    /// The user rejects the pending version of hook `id` (`expected_sha256`, app window only): a
    /// changed hook gets its approved version back (the kept copy, so it is approved again at
    /// once), a never-approved one is deleted. Returns the restored hook's view, `None` after a
    /// delete. `Conflict` when the file changed since it was shown, `NotFound` when there is no
    /// file or no kept copy of the approved version, `InvalidInput` for an approved hook.
    pub fn reject(&self, id: &str, expected_sha256: &str, capabilities: HookCapabilities) -> Result<Option<HookView>> {
        if self.reject_pending(id, expected_sha256)? {
            self.get(id, capabilities).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Writes a new hook file and approves it (the Harness family hooks). Returns its hash.
    pub(crate) fn create_approved(&self, hook: &Hook) -> Result<String> {
        self.create_approved_from(hook, Map::new())
    }

    /// Replaces the file of `hook.id`, while it still hashes to `expected_sha256`, with `hook`
    /// (its unknown fields kept) and approves it, pending or not: the user's explicit request for
    /// a Harness family hook. Returns the new hash.
    pub(crate) fn rewrite_approved(&self, hook: &Hook, expected_sha256: &str) -> Result<String> {
        self.ensure_dirs()?;

        let files = self.read_files().map_err(BusinessError::Forbidden)?;
        let existing = files
            .iter()
            .find(|file| file.id == hook.id)
            .ok_or_else(not_found::<HookKind>)?;

        if existing.sha256 != expected_sha256 {
            return Err(changed_since_read::<HookKind>());
        }

        self.write_approved(&files, hook, existing.object(), Some(expected_sha256))
    }
}
