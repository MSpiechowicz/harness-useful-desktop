//! What every kind's listing shares: the files of a folder sorted into the documents used and the
//! invalid files ([`sort_files`]), the approved earlier version a pending edit is compared with,
//! and why a local document cannot be shared ([`share_block`]). Each kind builds its own views
//! from these.

use std::collections::{BTreeMap, BTreeSet};

use super::approvals::Approvals;
use super::scope::{FileOwner, InvalidFile, ScopedDoc, ranked, within_limits};
use super::team::ProjectKey;
use super::{DocFile, DocKind, DocStore};
use crate::model::{HookApproval, HookScope, ShareBlock};

/// A folder's files as a listing shows them.
pub(crate) struct SortedFiles<D> {
    /// The valid documents used (within their scope's limit), each with its file: global before
    /// project, then `createdAt`, then `id`.
    pub used: Vec<(D, DocFile)>,
    /// The files that are not used, by id: why, and whose they are (a file over its scope's limit
    /// belongs to that scope, any other invalid file to every scope).
    pub invalid: Vec<InvalidFile>,
}

/// Sorts `files` (a folder as read) into the documents used and the invalid files, with
/// `approvals` deciding which documents a full scope keeps (approved ones first).
pub(crate) fn sort_files<K: DocKind>(files: Vec<DocFile>, approvals: &Approvals<K>) -> SortedFiles<K::Doc> {
    let mut valid = Vec::new();
    let mut invalid: Vec<InvalidFile> = Vec::new();

    for file in files {
        if file.unreadable {
            invalid.push((file, "the file could not be read".to_owned(), FileOwner::Unknown));
            continue;
        }

        match K::parse(&file.id, &file.bytes) {
            Ok(doc) => valid.push((doc, file)),
            Err(problem) => invalid.push((file, problem, FileOwner::Unknown)),
        }
    }

    let (mut used, over) = within_limits(valid, approvals);
    invalid.extend(over);

    used.sort_by_key(|(doc, _)| ranked(doc, 0));
    invalid.sort_by(|left, right| left.0.id.cmp(&right.0.id));

    SortedFiles { used, invalid }
}

impl<K: DocKind> DocStore<K> {
    /// The folder's files sorted for a listing, with the approvals they were sorted by, or why the
    /// folder is not read.
    pub(crate) fn sorted_files(&self) -> Result<(SortedFiles<K::Doc>, Approvals<K>), String> {
        let files = self.read_files()?;
        let approvals = self.read_approvals();

        Ok((sort_files(files, &approvals), approvals))
    }
}

/// The approved earlier version of `id` for a file whose approval is `approval`: only a changed
/// file has one (`approved` finds the kept copy).
pub(crate) fn earlier_version<D>(approval: HookApproval, id: &str, approved: &dyn Fn(&str) -> Option<D>) -> Option<D> {
    match approval {
        HookApproval::Changed => approved(id),
        HookApproval::Approved | HookApproval::Never => None,
    }
}

/// Why the local document `doc` (`None`: an invalid file; `pending`: waiting for review) cannot be
/// shared: invalid, never shared by its kind ([`DocKind::not_shared`]), pending, a project without
/// an `origin` remote (by `projects`), or already on the team (`team_ids`).
pub(crate) fn share_block<K: DocKind>(
    doc: Option<&K::Doc>,
    pending: bool,
    projects: &BTreeMap<String, ProjectKey>,
    team_ids: &BTreeSet<String>,
) -> Option<ShareBlock> {
    let Some(doc) = doc else {
        return Some(ShareBlock::Invalid);
    };

    if K::not_shared(doc).is_some() {
        return Some(ShareBlock::Seed);
    }

    if pending {
        return Some(ShareBlock::Pending);
    }

    let no_origin = doc.doc_scope() == HookScope::Project
        && doc
            .doc_project()
            .and_then(|root| projects.get(root))
            .is_some_and(|key| !key.can_share());

    if no_origin {
        return Some(ShareBlock::NoOrigin);
    }

    team_ids.contains(doc.doc_id()).then_some(ShareBlock::OnTeam)
}
