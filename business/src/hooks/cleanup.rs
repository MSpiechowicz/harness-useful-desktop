//! Removing hooks in bulk for [`HookStore`] (app window only): dropping the approvals of files that
//! are gone, and the Hooks view's "Delete all hooks".

use super::approvals::{Approvals, remove_snapshot, snapshot_ids};
use super::store::{HookCapabilities, HookStore};
use super::team::TeamFolder;
use crate::error::{BusinessError, Result};

impl HookStore {
    /// Drops the approvals and approved copies of hooks that have no `<id>.json` entry at all,
    /// neither in the hooks folder nor among the team copies (an agent removed it, it was deleted
    /// outside the app, or it left the team; app window only). An entry that is there but is not a
    /// regular file or cannot be read keeps its approval. Does nothing while the hooks folder is
    /// missing or not fully read, so a folder that is away for a moment keeps its approvals.
    /// Returns how many hooks lost their approval.
    pub fn prune_approvals(&self) -> Result<usize> {
        let is_folder = std::fs::metadata(self.hooks_dir()).is_ok_and(|metadata| metadata.is_dir());

        if !is_folder || self.read_files().is_err() {
            return Ok(0);
        }

        let Some(entries) = self.entry_ids() else {
            return Ok(0);
        };
        let team = TeamFolder::of(self).ids();
        let present = |id: &str| entries.contains(id) || team.contains(id);

        let mut approvals = Approvals::load_for_update(self.config_dir())?;
        let gone: Vec<String> = approvals.ids().filter(|id| !present(id)).map(str::to_owned).collect();

        for id in &gone {
            approvals.remove(id);
        }

        if !gone.is_empty() {
            approvals.save(self.config_dir())?;
        }

        for id in snapshot_ids(self.config_dir()) {
            if !present(&id) {
                remove_snapshot(self.config_dir(), &id);
            }
        }

        Ok(gone.len())
    }

    /// Deletes every hook file the Hooks view lists for a chat in `project` (a project root;
    /// `None`: global only, see [`HookStore::list_for`]; team hooks are never touched): the global hooks, that project's (pending
    /// and over-the-limit ones included), and every file that is not a valid hook, with their
    /// approvals and approved copies. Another project's hooks stay, even over its limit. The seed record is kept, so a Harness
    /// family hook deleted this way is not created again at launch. Approvals that cannot be read
    /// stop it before anything is deleted, a folder that is not read is `Forbidden`. Returns how
    /// many files were deleted.
    pub fn delete_all(&self, project: Option<&str>) -> Result<usize> {
        let listing = self.list_for(HookCapabilities::default(), project);

        if let Some(problem) = listing.folder_problem {
            return Err(BusinessError::Forbidden(problem));
        }

        let ids: Vec<String> = listing.hooks.into_iter().map(|view| view.id).collect();

        let mut approvals = Approvals::load_for_update(self.config_dir())?;
        let mut deleted = 0;

        for id in &ids {
            match std::fs::remove_file(self.file_path(id)) {
                Ok(()) => deleted += 1,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    // What is gone so far loses its approval before the failure is reported.
                    approvals.save(self.config_dir())?;
                    return Err(BusinessError::from_io("deleting a hook", error));
                }
            }

            approvals.remove(id);
            remove_snapshot(self.config_dir(), id);
        }

        approvals.save(self.config_dir())?;
        Ok(deleted)
    }
}
