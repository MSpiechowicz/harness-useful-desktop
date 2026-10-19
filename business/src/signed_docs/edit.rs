//! Writes and deletes of one kind's documents by the app window ([`HookAuthor::Desktop`]) or by
//! the MCP server on behalf of an agent ([`HookAuthor::Agent`]), the rules every kind shares:
//!
//! - Every write sets `updatedBy` to its author (a new file gets it as `createdBy` too) and keeps
//!   what the kind keeps of the file it rewrites (its builder's job) and its unknown top-level
//!   fields ([`DocKind::to_bytes`]). An edit or an agent's delete names the SHA-256 of the version
//!   it changes.
//! - Only the app window approves: its writes store the approval and a copy of the approved bytes
//!   with the file. It never saves over a pending file (the user approves or rejects it first). An
//!   agent's write never touches the app config folder, and an agent removes only documents an
//!   agent created.
//! - The app window refuses a new document when a scope holds the limit of approved ones, an
//!   agent when it holds the limit of valid ones.

use serde_json::{Map, Value};

use super::approvals::Approvals;
use super::listing::sort_files;
use super::scope::{ScopedDoc, check_room};
use super::{DocFile, DocKind, DocStore, changed_since_shown, not_found, read_file};
use crate::error::{BusinessError, Result};
use crate::model::{HookApproval, HookAuthor};

/// What a write saved.
pub(crate) struct Saved<K: DocKind> {
    /// The document written.
    pub doc: K::Doc,
    /// The hash of its bytes.
    pub sha256: String,
    /// The approvals after the write (the app window's write approved it).
    pub approvals: Approvals<K>,
}

impl<K: DocKind> DocStore<K> {
    /// Creates (`id` absent) or edits the document `id` at version `expected_sha256` as `author`.
    /// `build` makes the document from its id, the object of the file it rewrites (empty for a new
    /// one) and that file parsed, when valid. The app window's write is approved at once, an
    /// agent's is pending. `InvalidInput` for a document that breaks the format, a full scope, an
    /// edit without `expected_sha256` or an app window edit of a pending file, `Conflict` when the
    /// file changed since, `NotFound` for an edit of a missing file, `Forbidden` when the folder is
    /// writable by other users. The app window's write fails before it changes anything when the
    /// approvals cannot be read.
    pub(crate) fn save_doc(
        &self,
        id: Option<&str>,
        expected_sha256: Option<&str>,
        author: HookAuthor,
        build: impl FnOnce(&str, &Map<String, Value>, Option<&K::Doc>) -> Result<K::Doc>,
    ) -> Result<Saved<K>> {
        self.ensure_dirs()?;

        let files = self.read_files().map_err(BusinessError::Forbidden)?;
        let (id, existing) = self.target_of(id, expected_sha256, &files)?;

        let approvals = match author {
            HookAuthor::Desktop => Approvals::load_for_update(self.config_dir())?,
            HookAuthor::Agent => self.read_approvals(),
        };

        if author == HookAuthor::Desktop
            && let Some(file) = existing
            && !approvals.is_approved(&file.id, &file.sha256)
        {
            return Err(BusinessError::InvalidInput(format!(
                "the {} is waiting for review: approve or reject it first",
                K::NOUN
            )));
        }

        let base = existing.map(DocFile::object).unwrap_or_default();
        let previous = existing.and_then(|file| K::parse(&file.id, &file.bytes).ok());

        let doc = build(&id, &base, previous.as_ref())?;
        let bytes = K::to_bytes(&doc, base);

        // The same rules as every reader, so no writer saves a file the readers reject.
        K::parse(&id, &bytes).map_err(BusinessError::InvalidInput)?;

        check_room::<K, K::Doc>(counted::<K>(&files, author, &approvals).iter(), &doc)?;

        let expected = existing.map(|file| file.sha256.as_str());
        let sha256 = self.write_file(&id, &bytes, expected)?;

        let approvals = match author {
            HookAuthor::Desktop => self.record_approval(approvals, &id, &bytes)?,
            HookAuthor::Agent => approvals,
        };

        Ok(Saved { doc, sha256, approvals })
    }

    /// Deletes the file `id` as `author`. The app window deletes any file (valid or not) and drops
    /// its approval; `expected_sha256` is optional for it, and a missing file is not an error. An
    /// agent must name the current version (`Conflict` otherwise, `NotFound` for a missing file)
    /// and may remove only a valid document with `createdBy: "agent"` (`Forbidden` otherwise: the
    /// user decides).
    pub(crate) fn delete_as(&self, id: &str, author: HookAuthor, expected_sha256: Option<&str>) -> Result<()> {
        let by_desktop = author == HookAuthor::Desktop;

        self.delete_doc(id, expected_sha256, by_desktop, |current| match author {
            HookAuthor::Agent => check_agent_removal::<K>(id, current, expected_sha256),
            HookAuthor::Desktop => Ok(()),
        })
    }

    /// The user rejects the pending version of `id` (`expected_sha256`, app window only): a
    /// changed document gets its approved version back (the kept copy, approved again at once), a
    /// never-approved one is deleted. True when it was restored, false after a delete. `Conflict`
    /// when the file changed since it was shown, `NotFound` when there is no file or no kept copy,
    /// `InvalidInput` for an approved document, `Forbidden` when the folder is not read.
    pub(crate) fn reject_pending(&self, id: &str, expected_sha256: &str) -> Result<bool> {
        super::check_id::<K>(id)?;

        let files = self.read_files().map_err(BusinessError::Forbidden)?;
        let file = files.iter().find(|file| file.id == id).ok_or_else(not_found::<K>)?;

        if file.sha256 != expected_sha256 {
            return Err(changed_since_shown::<K>());
        }

        match self.read_approvals().approval(id, &file.sha256) {
            HookApproval::Approved => Err(BusinessError::InvalidInput(format!("the {} is not pending", K::NOUN))),
            HookApproval::Never => {
                self.delete_as(id, HookAuthor::Desktop, Some(expected_sha256))?;
                Ok(false)
            }
            HookApproval::Changed => {
                self.restore_approved(id, expected_sha256)?;
                Ok(true)
            }
        }
    }

    /// Writes a new file for `doc` over `base` (the object it is made from, whose unknown fields it
    /// keeps) and approves it. Returns its hash.
    pub(crate) fn create_approved_from(&self, doc: &K::Doc, base: Map<String, Value>) -> Result<String> {
        self.ensure_dirs()?;

        let files = self.read_files().map_err(BusinessError::Forbidden)?;

        self.write_approved(&files, doc, base, None)
    }

    /// Writes `doc` (over `base`, the object of the file it replaces) while the file is at version
    /// `expected`, and approves it; `files` is the folder as read. Returns the new hash.
    pub(crate) fn write_approved(
        &self,
        files: &[DocFile],
        doc: &K::Doc,
        base: Map<String, Value>,
        expected: Option<&str>,
    ) -> Result<String> {
        let id = doc.doc_id();
        let bytes = K::to_bytes(doc, base);
        K::parse(id, &bytes).map_err(BusinessError::Internal)?;

        let approvals = Approvals::load_for_update(self.config_dir())?;
        check_room::<K, K::Doc>(counted::<K>(files, HookAuthor::Desktop, &approvals).iter(), doc)?;

        let sha256 = self.write_file(id, &bytes, expected)?;
        self.record_approval(approvals, id, &bytes)?;

        Ok(sha256)
    }

    /// Deletes every file a listing for `project` (a project root; `None`: global only) shows: the
    /// global documents, that project's (pending and over-the-limit ones included), and every file
    /// that is not a valid document, with their approvals and approved copies. Another project's
    /// documents stay, even over its limit; team copies are never touched. Approvals that cannot
    /// be read stop it before anything is deleted, a folder that is not read is `Forbidden`.
    /// Returns the ids of the files deleted.
    pub(crate) fn delete_all_for(&self, project: Option<&str>) -> Result<Vec<String>> {
        let files = self.read_files().map_err(BusinessError::Forbidden)?;
        let sorted = sort_files(files, &self.read_approvals());

        let used = sorted
            .used
            .iter()
            .filter(|(doc, _)| super::scope::FileOwner::of(doc).shown_in(project))
            .map(|(_, file)| file.id.clone());
        let invalid = sorted
            .invalid
            .iter()
            .filter(|(_, _, owner)| owner.shown_in(project))
            .map(|(file, _, _)| file.id.clone());

        let ids: Vec<String> = used.chain(invalid).collect();

        self.delete_ids(&ids)
    }
}

/// The documents a new one counts against: for the app window the approved valid ones (so files
/// waiting for review cannot fill a scope), for an agent every valid one.
fn counted<K: DocKind>(files: &[DocFile], author: HookAuthor, approvals: &Approvals<K>) -> Vec<K::Doc> {
    files
        .iter()
        .filter(|file| author == HookAuthor::Agent || approvals.is_approved(&file.id, &file.sha256))
        .filter_map(|file| K::parse(&file.id, &file.bytes).ok())
        .collect()
}

/// An agent removes only the version it read of a valid document an agent created.
fn check_agent_removal<K: DocKind>(id: &str, current: Option<&DocFile>, expected_sha256: Option<&str>) -> Result<()> {
    if expected_sha256.is_none() {
        return Err(BusinessError::InvalidInput(
            "a removal names the version it removes".to_owned(),
        ));
    }

    let Some(file) = current else {
        return Err(not_found::<K>());
    };

    let created_by_agent = K::parse(id, &file.bytes).is_ok_and(|doc| doc.doc_created_by() == HookAuthor::Agent);

    if !created_by_agent {
        return Err(BusinessError::Forbidden(format!(
            "only the user can remove this {}, ask them to do it in Harness Desktop",
            K::NOUN
        )));
    }

    Ok(())
}

/// The file `id` of `store` when it is there at version `sha256` and readable; `Conflict`
/// otherwise (it changed since it was shown).
pub(crate) fn file_at<K: DocKind>(store: &DocStore<K>, id: &str, sha256: &str) -> Result<DocFile> {
    read_file::<K>(&store.file_path(id), id)
        .filter(|file| !file.unreadable && file.sha256 == sha256)
        .ok_or_else(changed_since_shown::<K>)
}
