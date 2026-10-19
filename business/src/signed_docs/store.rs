//! The approval bookkeeping of a [`DocStore`] that every kind shares: approving and dropping
//! approvals with the files, the target of an edit, deleting, approving and restoring one file,
//! and removing approvals whose file is gone.
//!
//! Only the app window approves: its writes store the approval and a copy of the approved bytes
//! with the file. An agent's write never touches the app config folder. A writer that cannot read
//! the approvals stops before it changes anything.

use super::approvals::Approvals;
use super::folder::changed_since_read;
use super::snapshots::{approved_bytes, approved_doc, remove_snapshot, save_snapshot, snapshot_ids};
use super::team::TeamFolder;
use super::{DocFile, DocKind, DocStore, TEAM_APPROVAL_PREFIX, check_id, read_file, sha256_hex, team_approval_key};
use crate::error::{BusinessError, Result};

impl<K: DocKind> DocStore<K> {
    /// Approves `bytes` for the approval key `key` (a local file's id, or a team copy's
    /// [`team_approval_key`]) in `approvals` (loaded for the update), saves them and keeps a copy
    /// of the bytes. Returns the saved approvals.
    pub(crate) fn record_approval(&self, mut approvals: Approvals<K>, key: &str, bytes: &[u8]) -> Result<Approvals<K>> {
        approvals.insert(key, &sha256_hex(bytes));
        approvals.save(self.config_dir())?;

        save_snapshot::<K>(self.config_dir(), key, bytes);
        Ok(approvals)
    }

    /// The approvals as a reader sees them: none when the file cannot be read (logged).
    pub(crate) fn read_approvals(&self) -> Approvals<K> {
        Approvals::read(self.config_dir()).unwrap_or_else(|error| {
            tracing::warn!(
                code = ?error.code(),
                "the {} approvals cannot be read, every {} is pending",
                K::NOUN,
                K::NOUN
            );
            Approvals::default()
        })
    }

    /// The approved earlier version of `id` (the copy kept when it was approved), while
    /// `approvals` still names that copy.
    pub(crate) fn approved_version(&self, approvals: &Approvals<K>, id: &str) -> Option<K::Doc> {
        approved_doc::<K>(self.config_dir(), id, approvals.get(id)?)
    }

    /// The id an edit or a new document writes and the file it replaces (`None` for a new one,
    /// `id` absent): an edit names the version it changes (`expected_sha256`). `NotFound` for a
    /// missing file, `Conflict` when it changed since.
    pub(crate) fn target_of<'a>(
        &self,
        id: Option<&str>,
        expected_sha256: Option<&str>,
        files: &'a [DocFile],
    ) -> Result<(String, Option<&'a DocFile>)> {
        let Some(id) = id else {
            return Ok((uuid::Uuid::new_v4().simple().to_string(), None));
        };

        check_id::<K>(id)?;

        let existing = files.iter().find(|file| file.id == id).ok_or_else(not_found::<K>)?;

        let Some(expected) = expected_sha256 else {
            return Err(BusinessError::InvalidInput(
                "an edit names the version it changes".to_owned(),
            ));
        };

        if existing.sha256 != expected {
            return Err(changed_since_read::<K>());
        }

        Ok((id.to_owned(), Some(existing)))
    }

    /// Deletes the file `id`. `Conflict` when it is not at `expected_sha256` (when given), and
    /// `check` sees the file first (an agent's rules). The app window (`by_desktop`) drops its
    /// approval and approved copy too, and stops before anything changes when the approvals
    /// cannot be read. A missing file is not an error here.
    pub(crate) fn delete_doc(
        &self,
        id: &str,
        expected_sha256: Option<&str>,
        by_desktop: bool,
        check: impl FnOnce(Option<&DocFile>) -> Result<()>,
    ) -> Result<()> {
        check_id::<K>(id)?;

        let path = self.file_path(id);
        let current = read_file::<K>(&path, id);

        if let Some(expected) = expected_sha256
            && current.as_ref().is_some_and(|file| file.sha256 != expected)
        {
            return Err(changed_since_read::<K>());
        }

        check(current.as_ref())?;

        // Read before anything changes: approvals that cannot be read stop the delete.
        let approvals = if by_desktop {
            Some(Approvals::<K>::load_for_update(self.config_dir())?)
        } else {
            None
        };

        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(BusinessError::from_io(format!("deleting a {}", K::NOUN), error)),
        }

        if let Some(mut approvals) = approvals {
            if approvals.remove(id) {
                approvals.save(self.config_dir())?;
            }

            remove_snapshot::<K>(self.config_dir(), id);
        }

        Ok(())
    }

    /// The user approves the current bytes of `id` (app window only). `Conflict` when they are not
    /// the version the user reviewed (`expected_sha256`), `InvalidInput` for an invalid file,
    /// `NotFound` for a missing one.
    pub(crate) fn approve_file(&self, id: &str, expected_sha256: &str) -> Result<()> {
        check_id::<K>(id)?;

        let file = read_file::<K>(&self.file_path(id), id).ok_or_else(not_found::<K>)?;

        if file.sha256 != expected_sha256 {
            return Err(changed_since_shown::<K>());
        }

        K::parse(id, &file.bytes).map_err(BusinessError::InvalidInput)?;

        let approvals = Approvals::load_for_update(self.config_dir())?;
        self.record_approval(approvals, id, &file.bytes).map(drop)
    }

    /// Puts the approved version of `id` (its kept copy) back over the pending file at
    /// `expected_sha256`, so it is approved again at once. `NotFound` without a kept copy,
    /// `Conflict` when the file changed since.
    pub(crate) fn restore_approved(&self, id: &str, expected_sha256: &str) -> Result<()> {
        let approvals = Approvals::<K>::read(self.config_dir())?;
        let bytes = approvals
            .get(id)
            .and_then(|sha256| approved_bytes::<K>(self.config_dir(), id, sha256))
            .ok_or_else(|| BusinessError::NotFound(format!("the approved version of the {}", K::NOUN)))?;

        self.write_file(id, &bytes, Some(expected_sha256)).map(drop)
    }

    /// Drops the approvals and approved copies of documents that have no `<id>.json` entry at
    /// all, neither in the folder nor among the team copies (an agent removed it, it was deleted
    /// outside the app, or it left the team; app window only). An entry that is there but is not
    /// a regular file or cannot be read keeps its approval. Does nothing while the folder is
    /// missing or not fully read, so a folder that is away for a moment keeps its approvals.
    /// Returns how many documents lost their approval.
    pub fn prune_approvals(&self) -> Result<usize> {
        let is_folder = std::fs::metadata(self.docs_dir()).is_ok_and(|metadata| metadata.is_dir());

        if !is_folder || self.read_files().is_err() {
            return Ok(0);
        }

        let Some(entries) = self.entry_ids() else {
            return Ok(0);
        };
        let team = TeamFolder::of(self).ids();

        // A kind that keeps team approvals apart keeps a local approval for a local file only, and
        // a `team:<id>` one for a team copy only.
        let present = |key: &str| match key.strip_prefix(TEAM_APPROVAL_PREFIX) {
            Some(id) => team.contains(id),
            None if K::SEPARATE_TEAM_APPROVALS => entries.contains(key),
            None => entries.contains(key) || team.contains(key),
        };

        let mut approvals = Approvals::<K>::load_for_update(self.config_dir())?;
        let gone: Vec<String> = approvals.ids().filter(|id| !present(id)).map(str::to_owned).collect();

        for id in &gone {
            approvals.remove(id);
        }

        if !gone.is_empty() {
            approvals.save(self.config_dir())?;
        }

        for id in snapshot_ids::<K>(self.config_dir()) {
            if !present(&id) {
                remove_snapshot::<K>(self.config_dir(), &id);
            }
        }

        Ok(gone.len())
    }

    /// Deletes the files `ids` with their approvals and approved copies (app window only).
    /// Approvals that cannot be read stop it before anything is deleted. Returns the ids of the
    /// files deleted.
    pub(crate) fn delete_ids(&self, ids: &[String]) -> Result<Vec<String>> {
        let mut approvals = Approvals::<K>::load_for_update(self.config_dir())?;
        let mut deleted = Vec::new();

        for id in ids {
            match std::fs::remove_file(self.file_path(id)) {
                Ok(()) => deleted.push(id.clone()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    // What is gone so far loses its approval before the failure is reported.
                    approvals.save(self.config_dir())?;
                    return Err(BusinessError::from_io(format!("deleting a {}", K::NOUN), error));
                }
            }

            approvals.remove(id);
            remove_snapshot::<K>(self.config_dir(), id);
        }

        approvals.save(self.config_dir())?;
        Ok(deleted)
    }

    /// The user approves the team copy of `id` at version `sha256` (the approvals file of local
    /// files under [`team_approval_key`], with a kept copy for the diff of a later version). `Conflict` when the copy changed
    /// since, `NotFound` without a copy, `InvalidInput` for a body that breaks the team rules or a
    /// duplicate (the local file with this id is the one that is used).
    pub(crate) fn approve_team_copy(&self, folder: &TeamFolder<K>, id: &str, sha256: &str) -> Result<()> {
        let row = folder.row_at(id, sha256)?;
        let bytes = row.body.as_bytes();

        K::parse_team(id, bytes).map_err(BusinessError::InvalidInput)?;

        if read_file::<K>(&self.file_path(id), id).is_some() {
            return Err(BusinessError::InvalidInput(format!(
                "a local {} has this id, so it runs instead",
                K::NOUN
            )));
        }

        let approvals = Approvals::load_for_update(self.config_dir())?;
        self.record_approval(approvals, &team_approval_key::<K>(id), bytes)
            .map(drop)
    }

    /// Drops the approvals and approved copies of the team documents `ids` that left this
    /// machine, so the same bytes published again wait for approval, and what the kind's state
    /// keeps for them ([`DocKind::forget_team_state`]). An id a local file has keeps its state,
    /// and (when one approval serves both, [`DocKind::SEPARATE_TEAM_APPROVALS`] off) its approval:
    /// that belongs to the local file.
    pub(crate) fn forget_team_approvals(&self, ids: &[String]) -> Result<()> {
        let orphans: Vec<String> = ids
            .iter()
            .filter(|id| std::fs::symlink_metadata(self.file_path(id)).is_err())
            .cloned()
            .collect();

        let keys: Vec<String> = if K::SEPARATE_TEAM_APPROVALS {
            ids.iter().map(|id| team_approval_key::<K>(id)).collect()
        } else {
            orphans.clone()
        };

        if keys.is_empty() {
            return Ok(());
        }

        let mut approvals = Approvals::<K>::load_for_update(self.config_dir())?;
        let mut changed = false;

        for key in &keys {
            changed |= approvals.remove(key);
            remove_snapshot::<K>(self.config_dir(), key);
        }

        if changed {
            approvals.save(self.config_dir())?;
        }

        if orphans.is_empty() {
            return Ok(());
        }

        K::forget_team_state(self.config_dir(), &orphans)
    }
}

/// `NotFound`: there is no such file.
pub(crate) fn not_found<K: DocKind>() -> BusinessError {
    BusinessError::NotFound(format!("the {} file", K::NOUN))
}

/// `Conflict`: the file is no longer the version the user was shown.
pub(crate) fn changed_since_shown<K: DocKind>() -> BusinessError {
    BusinessError::Conflict(format!("the {} changed since it was shown", K::NOUN))
}
