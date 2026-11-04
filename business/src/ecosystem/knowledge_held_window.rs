//! The review queue's window kept in the app: the server answers every waiting proposal (at most
//! 100), so the scope and the sort of the Knowledge view's Review tab are applied here.

use crate::model::{KnowledgeHeldItem, KnowledgeScope};

/// The column the review queue is sorted by. The queue is small (the server keeps at most 100
/// proposals), so it is sorted here, newest or Z first unless the window asks otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeldSort {
    Title,
    Client,
    Project,
    /// The person the proposal is written for (`addedBy`), proposals without a name last.
    AddedBy,
    #[default]
    Proposed,
}

impl HeldSort {
    /// The sort a column key names (`title`, `client`, `project`, `addedBy`, `proposed`), else the time.
    pub fn of_key(key: Option<&str>) -> Self {
        match key {
            Some("title") => Self::Title,
            Some("client") => Self::Client,
            Some("project") => Self::Project,
            Some("addedBy") => Self::AddedBy,
            _ => Self::Proposed,
        }
    }
}

/// Whether a proposal belongs to `scope`, as a document would: global when it links to no
/// repository, else the repositories it links to (and the one its agent worked in).
pub(super) fn in_scope(item: &KnowledgeHeldItem, scope: &KnowledgeScope) -> bool {
    match scope {
        KnowledgeScope::All => true,
        KnowledgeScope::Global => item.links.is_empty(),
        KnowledgeScope::Repo { key } => {
            item.links.iter().any(|link| &link.repo == key) || item.repo_key.as_deref() == Some(key.as_str())
        }
    }
}

/// Sorts `items` by `sort`, Z to A or newest first when `descending`, ties by the time. A proposal
/// with no name is the last of the `AddedBy` order in either direction.
pub(super) fn sort_items(items: &mut [KnowledgeHeldItem], sort: HeldSort, descending: bool) {
    if sort == HeldSort::AddedBy {
        let name = |item: &KnowledgeHeldItem| {
            item.added_by
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_lowercase)
        };

        items.sort_by(|left, right| {
            let (left_name, right_name) = (name(left), name(right));
            let by = match (&left_name, &right_name) {
                (Some(left), Some(right)) if descending => right.cmp(left),
                (Some(left), Some(right)) => left.cmp(right),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            };

            by.then_with(|| right.created_at.cmp(&left.created_at))
        });

        return;
    }

    let folder = |item: &KnowledgeHeldItem| {
        item.project_path
            .as_deref()
            .and_then(|path| path.rsplit(['/', '\\']).find(|part| !part.is_empty()))
            .unwrap_or_default()
            .to_lowercase()
    };

    items.sort_by(|left, right| {
        let by = match sort {
            HeldSort::Title => left.title.to_lowercase().cmp(&right.title.to_lowercase()),
            HeldSort::Client => left.client.to_lowercase().cmp(&right.client.to_lowercase()),
            HeldSort::Project => folder(left).cmp(&folder(right)),
            HeldSort::AddedBy | HeldSort::Proposed => std::cmp::Ordering::Equal,
        };

        by.then_with(|| left.created_at.cmp(&right.created_at)).reverse()
    });

    if !descending {
        items.reverse();
    }
}
