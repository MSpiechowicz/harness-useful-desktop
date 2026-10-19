//! Scopes of signed documents: a document is global or belongs to one project root. Per scope at
//! most [`DocKind::MAX_PER_SCOPE`] documents are used, approved ones first (so files waiting for
//! review never push an approved one out), each group by `createdAt` then `id`; the rest are
//! invalid ("over the limit") and listed only with their own scope. Global documents come before
//! project ones, then `createdAt`, then `id` ([`ranked`]).

use std::collections::BTreeMap;
use std::path::Path;

use super::approvals::Approvals;
use super::fields::timestamp_key;
use super::{DocFile, DocKind, project_identity};
use crate::error::{BusinessError, Result};
use crate::model::{HookAuthor, HookScope};

/// What the scope rules need of a parsed document.
pub trait ScopedDoc {
    /// The document id.
    fn doc_id(&self) -> &str;

    /// Global or project.
    fn doc_scope(&self) -> HookScope;

    /// The project root (or, in a team body, the repository key) of a project document.
    fn doc_project(&self) -> Option<&str>;

    /// `createdAt` (RFC 3339 UTC).
    fn doc_created_at(&self) -> &str;

    /// `createdBy`: who may remove it.
    fn doc_created_by(&self) -> HookAuthor;
}

/// Implements [`ScopedDoc`] for a document type with the fields `id`, `scope`, `project`,
/// `created_at` and `created_by` (every kind's parsed file).
macro_rules! scoped_doc {
    ($doc:ty) => {
        impl $crate::signed_docs::scope::ScopedDoc for $doc {
            fn doc_id(&self) -> &str {
                &self.id
            }

            fn doc_scope(&self) -> $crate::model::HookScope {
                self.scope
            }

            fn doc_project(&self) -> Option<&str> {
                self.project.as_deref()
            }

            fn doc_created_at(&self) -> &str {
                &self.created_at
            }

            fn doc_created_by(&self) -> $crate::model::HookAuthor {
                self.created_by
            }
        }
    };
}

pub(crate) use scoped_doc;

/// The scope a document counts against: `None` for global, the project root otherwise.
pub(crate) fn scope_key<D: ScopedDoc>(doc: &D) -> Option<&str> {
    match doc.doc_scope() {
        HookScope::Global => None,
        HookScope::Project => doc.doc_project(),
    }
}

/// Whose a listed file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileOwner {
    /// A global document, used or over the limit.
    Global,
    /// A document of this project root, used or over the limit.
    Project(String),
    /// A file that is not a valid document: its scope is unknown.
    Unknown,
}

impl FileOwner {
    pub(crate) fn of<D: ScopedDoc>(doc: &D) -> Self {
        match (doc.doc_scope(), doc.doc_project()) {
            (HookScope::Global, _) => Self::Global,
            (HookScope::Project, Some(project)) => Self::Project(project.to_owned()),
            (HookScope::Project, None) => Self::Unknown,
        }
    }

    /// True when a list for `project` (a project root; `None`: global only) shows the file: a
    /// global document, a document of that project, or a file whose scope is unknown.
    pub(crate) fn shown_in(&self, project: Option<&str>) -> bool {
        match self {
            Self::Global | Self::Unknown => true,
            Self::Project(root) => project == Some(root.as_str()),
        }
    }
}

/// Where a document goes: by origin and scope, then `createdAt`, then `id`.
pub(crate) type RunOrder = (u8, [u32; 7], String);

/// The order of `doc` among documents of rank `base` (local `0`, team `2`): global before
/// project, then `createdAt`, then `id`.
pub(crate) fn ranked<D: ScopedDoc>(doc: &D, base: u8) -> RunOrder {
    let rank = match doc.doc_scope() {
        HookScope::Global => base,
        HookScope::Project => base + 1,
    };

    (
        rank,
        timestamp_key(doc.doc_created_at()).unwrap_or_default(),
        doc.doc_id().to_owned(),
    )
}

/// A file listed as invalid: why, and whose it is.
pub(crate) type InvalidFile = (DocFile, String, FileOwner);

/// Splits the valid documents of a folder (each with its file) into the ones used and the ones
/// over their scope's limit (with the problem and whose they are). Used ones come back approved
/// first, then in local order ([`ranked`] with `0`).
pub(crate) fn within_limits<K: DocKind, D: ScopedDoc>(
    valid: Vec<(D, DocFile)>,
    approvals: &Approvals<K>,
) -> (Vec<(D, DocFile)>, Vec<InvalidFile>) {
    let mut valid = valid;

    // Approved ones first (so files waiting for review cannot push them out), the oldest first
    // within each group.
    valid.sort_by_key(|(doc, file)| (!approvals.is_approved(&file.id, &file.sha256), ranked(doc, 0)));

    let mut counts: BTreeMap<Option<String>, usize> = BTreeMap::new();
    let mut used = Vec::new();
    let mut over = Vec::new();

    for (doc, file) in valid {
        let count = counts.entry(scope_key(&doc).map(str::to_owned)).or_default();
        *count += 1;

        // An over-the-limit document is listed as invalid, but only where its scope is.
        if *count > K::MAX_PER_SCOPE {
            let problem = format!("over the limit of {} {} in this scope", K::MAX_PER_SCOPE, K::PLURAL);
            over.push((file, problem, FileOwner::of(&doc)));
        } else {
            used.push((doc, file));
        }
    }

    (used, over)
}

/// Refuses `doc` when its scope already holds [`DocKind::MAX_PER_SCOPE`] other documents
/// (`others`: the documents it counts against).
pub(crate) fn check_room<'a, K: DocKind, D: ScopedDoc + 'a>(
    others: impl Iterator<Item = &'a D>,
    doc: &D,
) -> Result<()> {
    let count = others
        .filter(|other| other.doc_id() != doc.doc_id() && scope_key(*other) == scope_key(doc))
        .count();

    if count >= K::MAX_PER_SCOPE {
        return Err(BusinessError::InvalidInput(format!(
            "a scope holds at most {} {}",
            K::MAX_PER_SCOPE,
            K::PLURAL
        )));
    }

    Ok(())
}

/// `createdAt` and `createdBy` an edit keeps from the file it rewrites: the valid `previous`
/// document's, else (an invalid file being fixed) what `base`, its object, still holds of them.
pub(crate) fn kept_origin<D: ScopedDoc>(
    base: &serde_json::Map<String, serde_json::Value>,
    previous: Option<&D>,
) -> (Option<String>, Option<HookAuthor>) {
    if let Some(previous) = previous {
        return (
            Some(previous.doc_created_at().to_owned()),
            Some(previous.doc_created_by()),
        );
    }

    let text = |key: &str| base.get(key).and_then(serde_json::Value::as_str);

    let created_at = text("createdAt")
        .filter(|time| timestamp_key(time).is_some())
        .map(str::to_owned);
    let created_by = match text("createdBy") {
        Some("agent") => Some(HookAuthor::Agent),
        Some("desktop") => Some(HookAuthor::Desktop),
        _ => None,
    };

    (created_at, created_by)
}

/// The project root a project draft stores: `previous` (the file's) when the draft names it
/// again, else the project identity of the folder `folder` names (which must exist and be a
/// folder).
pub(crate) fn draft_project<K: DocKind>(folder: Option<&str>, previous: Option<&str>) -> Result<String> {
    let Some(folder) = folder.filter(|folder| !folder.is_empty()) else {
        return Err(BusinessError::InvalidInput(format!(
            "a project {} needs a project",
            K::NOUN
        )));
    };

    if previous == Some(folder) {
        return Ok(folder.to_owned());
    }

    let root = project_identity(Path::new(folder))?;

    root.into_os_string()
        .into_string()
        .map_err(|_| BusinessError::InvalidInput("the project path is not valid UTF-8".to_owned()))
}
