//! What every kind's listing shares: the files of a folder sorted into the documents used and the
//! invalid files ([`sort_files`]), the approved earlier version a pending edit is compared with,
//! why a local document cannot be shared ([`share_block`]), and which documents the app window's
//! listing shows and the repository each belongs to ([`ListingScope`]). Each kind builds its own
//! views from these.

use std::collections::{BTreeMap, BTreeSet};

use super::approvals::Approvals;
use super::scope::{FileOwner, InvalidFile, ScopedDoc, ranked, within_limits};
use super::team::ProjectKey;
use super::{DocFile, DocKind, DocStore};
use crate::model::{HookApproval, HookOrigin, HookScope, ShareBlock};

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

/// Which documents the app window's listing shows, and the roots it may match to a repository key.
pub(crate) struct ListingScope<'a> {
    /// The active project root (`None`: global only).
    pub project: Option<&'a str>,
    /// Every local file and every team copy, not only those a chat in `project` sees.
    pub all: bool,
    /// True for a project root a refresh of this session queried, so its repository key
    /// (`projects.json`) may be used.
    pub verified: &'a dyn Fn(&str) -> bool,
}

impl ListingScope<'_> {
    /// The active project root while its repository key may be used.
    pub(crate) fn verified_project(&self) -> Option<&str> {
        self.project.filter(|root| (self.verified)(root))
    }

    /// The project root and the repository key of a listed document. A local one has the root its
    /// file names (`root`) and the key `projects` holds for that root while it is verified. A team
    /// one has its repository key (`team_repo`) and a root only when a verified root has that key
    /// (the active project first): a team document is never matched to a folder on what an
    /// earlier session left behind.
    pub(crate) fn repo_of(
        &self,
        origin: HookOrigin,
        root: Option<&str>,
        team_repo: Option<&str>,
        projects: &BTreeMap<String, ProjectKey>,
    ) -> (Option<String>, Option<String>) {
        let key_of = |root: &str| {
            projects
                .get(root)
                .filter(|_| (self.verified)(root))
                .map(|project| project.key.as_str())
        };

        match origin {
            HookOrigin::Local => (root.map(str::to_owned), root.and_then(key_of).map(str::to_owned)),
            HookOrigin::Team => {
                let Some(key) = team_repo else {
                    return (None, None);
                };

                let root = self
                    .project
                    .into_iter()
                    .chain(projects.keys().map(String::as_str))
                    .find(|root| key_of(root) == Some(key));

                (root.map(str::to_owned), Some(key.to_owned()))
            }
        }
    }
}
