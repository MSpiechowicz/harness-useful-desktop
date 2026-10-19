//! Removing hooks in bulk for [`HookStore`] (app window only): the Hooks view's "Delete all
//! hooks". Dropping the approvals of files that are gone is
//! [`DocStore::prune_approvals`](crate::signed_docs::DocStore::prune_approvals).

use super::store::HookStore;
use crate::error::Result;

impl HookStore {
    /// Deletes every hook file the Hooks view lists for a chat in `project` (a project root;
    /// `None`: global only, see [`HookStore::list_for`]; team hooks are never touched): the global hooks, that project's (pending
    /// and over-the-limit ones included), and every file that is not a valid hook, with their
    /// approvals and approved copies. Another project's hooks stay, even over its limit. The seed record is kept, so a Harness
    /// family hook deleted this way is not created again at launch. Approvals that cannot be read
    /// stop it before anything is deleted, a folder that is not read is `Forbidden`. Returns how
    /// many files were deleted.
    pub fn delete_all(&self, project: Option<&str>) -> Result<usize> {
        self.delete_all_for(project).map(|ids| ids.len())
    }
}
