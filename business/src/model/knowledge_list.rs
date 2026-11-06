//! What a list of documents is asked with (`knowledge_docs_list`): the repository scope, the sort
//! and the filters.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{AuthorFilter, KnowledgePlace, SortOrder};

/// Which documents a list shows by their repository links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeScope {
    /// Every document.
    All,
    /// The documents linked to no repository.
    Global,
    /// The documents linked to the repository with this key.
    Repo {
        /// The repository key (see [`KnowledgeLink::repo`]).
        key: String,
    },
}

/// How a list is sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeSort {
    /// By title, A to Z unless reversed.
    Title,
    /// By the last change, newest first unless reversed.
    Updated,
    /// By size, largest first unless reversed.
    Size,
    /// By format name, A to Z unless reversed.
    Format,
    /// By chunk count, most first unless reversed.
    Chunks,
    /// By how many paths the links narrow to together, most first unless reversed. Asked of a
    /// server that lists `docs-sort-paths` only.
    Paths,
    /// By the first linked repository, global documents first unless reversed.
    Repos,
    /// By how often an agent found the document useful, most first unless reversed. Asked of a
    /// server that lists `docs-useful` only.
    Useful,
    /// By when an agent last found the document useful, latest first unless reversed. Asked of a
    /// server that lists `docs-useful` only.
    LastUseful,
    /// By the name of who first imported the document, A to Z unless reversed. Asked of a server
    /// that lists `docs-author-filter` only.
    AddedBy,
}

impl KnowledgeSort {
    /// Whether the sort reads the usefulness counts, which a server lists as `docs-useful` only.
    #[must_use]
    pub fn needs_useful(self) -> bool {
        matches!(self, Self::Useful | Self::LastUseful)
    }

    /// Whether the sort counts the paths, which a server lists as `docs-sort-paths` only.
    #[must_use]
    pub fn needs_paths(self) -> bool {
        self == Self::Paths
    }

    /// Whether the sort reads the writer, which a server lists as `docs-author-filter` only.
    #[must_use]
    pub fn needs_author_filter(self) -> bool {
        self == Self::AddedBy
    }
}

/// Which documents a list keeps by whether agents found them useful (`--useful`). Asked of a
/// server that lists `docs-useful` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum KnowledgeUsefulFilter {
    /// Documents an agent found useful at least once.
    Ever,
    /// Documents shown at least three times and never found useful.
    Never,
}

/// Which documents a list keeps by their state (`--status`). A live document counts for the state of
/// its pending version too, because that is what its people wait on. Asked of a server that lists
/// `docs-drafts` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum KnowledgeStatusFilter {
    /// Live documents, with or without a pending version.
    Live,
    /// Drafts, and live documents whose pending version is a draft.
    Draft,
    /// Documents waiting for approval, and live ones whose pending version waits.
    Waiting,
    /// Documents with changes requested, and live ones whose pending version has them.
    ChangesRequested,
    /// Live documents that have a pending version.
    Pending,
}

/// What a list of documents is asked with: the filters and the page (see
/// [`SortOrder`] for the direction).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct KnowledgeListFilter {
    /// By repository (every document unless given).
    pub scope: Option<KnowledgeScope>,
    /// By format (`markdown`, `pdf`, ...).
    pub format: Option<String>,
    /// By the place the document is kept in, when both are in use (every place unless given).
    pub place: Option<KnowledgePlace>,
    /// The sort (`updated` unless given).
    pub sort: Option<KnowledgeSort>,
    /// The direction (the sort's own unless given).
    pub order: Option<SortOrder>,
    /// By whether agents found the document useful (every document unless given). Dropped for a
    /// server without `docs-useful`.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub useful: Option<KnowledgeUsefulFilter>,
    /// By state (live documents and drafts, never a pending version of its own, unless given).
    /// Dropped for a server without `docs-drafts`.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub status: Option<KnowledgeStatusFilter>,
    /// By who first imported the document (every document unless given). Dropped for a server
    /// without `docs-author-filter`. `invalidInput` for a name the servers would refuse.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub author: Option<AuthorFilter>,
}
