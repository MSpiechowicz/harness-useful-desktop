//! What a list of documents is asked with (`knowledge_docs_list`): the repository scope, the sort
//! and the filters.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{KnowledgePlace, SortOrder};

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
    /// By the first linked repository, global documents first unless reversed.
    Repos,
    /// By how often an agent found the document useful, most first unless reversed. Asked of a
    /// server that lists `docs-useful` only.
    Useful,
    /// By when an agent last found the document useful, latest first unless reversed. Asked of a
    /// server that lists `docs-useful` only.
    LastUseful,
}

impl KnowledgeSort {
    /// Whether the sort reads the usefulness counts, which a server lists as `docs-useful` only.
    #[must_use]
    pub fn needs_useful(self) -> bool {
        matches!(self, Self::Useful | Self::LastUseful)
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
}
