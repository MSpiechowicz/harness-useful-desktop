//! The team copies as every kind's team listing starts from: each envelope checked
//! ([`checked_doc`]) with its approval, sharer, the local version it was shared from and whether a
//! local file has its id ([`team_entries`]). Each kind decides the status and builds its view.

use std::collections::BTreeSet;

use super::folder::{TeamFile, TeamFolder};
use super::{TeamRow, author_label, checked_doc};
use crate::model::HookApproval;
use crate::signed_docs::approvals::Approvals;
use crate::signed_docs::fields::loose_name;
use crate::signed_docs::listing::earlier_version;
use crate::signed_docs::scope::ranked;
use crate::signed_docs::snapshots::approved_bytes;
use crate::signed_docs::state::TeamState;
use crate::signed_docs::{DocKind, DocStore, read_file, sha256_hex, team_approval_key};

/// One team copy as read.
pub(crate) struct TeamEntry<K: DocKind> {
    /// The file name stem.
    pub id: String,
    /// The row, `None` when the envelope cannot be used.
    pub row: Option<TeamRow>,
    /// Lowercase SHA-256 of the body (empty without a row).
    pub sha256: String,
    /// How this machine's approvals relate to the body.
    pub approval: HookApproval,
    /// A readable name even for a body that breaks the rules.
    pub name: Option<String>,
    /// The sharer, without control or invisible characters.
    pub author: Option<String>,
    /// The local document with the same id changed since it was shared.
    pub local_changed: bool,
    /// A local file has this id: the local one wins.
    pub duplicate: bool,
    /// The document, or why the copy cannot be used.
    pub doc: Result<K::Doc, String>,
    /// The approved earlier version of a changed body (none for a duplicate), for the diff.
    pub approved: Option<K::Doc>,
}

/// Every team copy in `folder`: approvals from `approvals`, `sharedFrom` from `state` and
/// duplicates from `store`'s folder. Valid ones in team order (global, then project, then
/// `createdAt`, then `id`), the others after them by id.
pub(crate) fn team_entries<K: DocKind, S: TeamState>(
    store: &DocStore<K>,
    folder: &TeamFolder<K>,
    state: &S,
    approvals: &Approvals<K>,
) -> Vec<TeamEntry<K>> {
    let local_ids = store.entry_ids().unwrap_or_default();

    let mut valid = Vec::new();
    let mut invalid = Vec::new();

    for file in folder.read() {
        let entry = entry_of(store, file, state, approvals, &local_ids);

        match &entry.doc {
            Ok(doc) => valid.push((ranked(doc, 2), entry)),
            Err(_) => invalid.push(entry),
        }
    }

    valid.sort_by(|left, right| left.0.cmp(&right.0));

    let mut entries: Vec<TeamEntry<K>> = valid.into_iter().map(|(_, entry)| entry).collect();
    entries.extend(invalid);

    entries
}

fn entry_of<K: DocKind, S: TeamState>(
    store: &DocStore<K>,
    file: TeamFile,
    state: &S,
    approvals: &Approvals<K>,
    local_ids: &BTreeSet<String>,
) -> TeamEntry<K> {
    let row = match file.row {
        Ok(row) => row,
        Err(problem) => {
            return TeamEntry {
                id: file.id,
                row: None,
                sha256: String::new(),
                approval: HookApproval::Never,
                name: None,
                author: None,
                local_changed: false,
                duplicate: false,
                doc: Err(problem),
                approved: None,
            };
        }
    };

    let bytes = row.body.as_bytes();
    let sha256 = sha256_hex(bytes);
    let approval = approvals.approval(&team_approval_key::<K>(&row.id), &sha256);
    let doc = checked_doc::<K>(&row, &sha256);
    let duplicate = local_ids.contains(&row.id);

    let approved = match (&doc, duplicate) {
        // A duplicate's approval is the local document's: there is no team version to compare.
        (Ok(_), false) => earlier_version(approval, &row.id, &|id| approved_copy(store, approvals, id)),
        _ => None,
    };

    let local_changed = state.shared_from(&row.id).is_some_and(|shared| {
        read_file::<K>(&store.file_path(&row.id), &row.id).is_some_and(|local| local.sha256 != shared)
    });

    TeamEntry {
        id: row.id.clone(),
        name: loose_name(bytes),
        author: Some(author_label(&row.author)),
        sha256,
        approval,
        local_changed,
        duplicate,
        doc,
        approved,
        row: Some(row),
    }
}

/// The approved earlier version of team document `id` (its kept copy), for the diff.
fn approved_copy<K: DocKind>(store: &DocStore<K>, approvals: &Approvals<K>, id: &str) -> Option<K::Doc> {
    let key = team_approval_key::<K>(id);
    let bytes = approved_bytes::<K>(store.config_dir(), &key, approvals.get(&key)?)?;

    K::parse_team(id, &bytes).ok()
}
