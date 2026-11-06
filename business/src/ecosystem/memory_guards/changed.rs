//! Which entries of `onRepos` have a project key other than the one pinned when they were turned
//! on (`changedRepos`). This app cannot compute keys: the installed server is asked for the
//! current key of the entries [`checkable_repos`] returns
//! ([`memory_project_key`](crate::ecosystem::memory_project_key)) and [`with_changed_repos`]
//! compares them with the pins, by the rule Harness Memory applies when it saves.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use super::saving_root::{is_git_folder, names_itself};
use crate::model::{GlobalKindGuard, MemoryGuards};

/// The entries of both kinds whose current key can be compared with their pin, once each, sorted:
/// those that name themselves and have a pin. A git folder is left out: the folder alone has a
/// key made from its path, while the pin is the key of the worktree the user turned on, which a
/// remote can make a different one, so the key of the entry says nothing about a change.
pub fn checkable_repos(guards: &MemoryGuards) -> Vec<String> {
    let mut entries = BTreeSet::new();

    for kind in [&guards.notes, &guards.scripts] {
        for entry in &kind.on_repos {
            let path = Path::new(entry);

            if kind.repo_keys.contains_key(entry) && names_itself(path) && !is_git_folder(path) {
                entries.insert(entry.clone());
            }
        }
    }

    entries.into_iter().collect()
}

/// `guards` with each kind's `changed_repos` set: the checkable entries whose current key (in
/// `current`, by entry; an entry the server could not answer for is missing) is not their pin.
pub fn with_changed_repos(mut guards: MemoryGuards, current: &HashMap<String, Option<String>>) -> MemoryGuards {
    for kind in [&mut guards.notes, &mut guards.scripts] {
        kind.changed_repos = changed(kind, current);
    }

    guards
}

fn changed(kind: &GlobalKindGuard, current: &HashMap<String, Option<String>>) -> Vec<String> {
    kind.on_repos
        .iter()
        .filter(|entry| match (kind.repo_keys.get(*entry), current.get(*entry)) {
            (Some(pinned), Some(now)) => !same_key(pinned.as_deref(), now.as_deref()),
            _ => false,
        })
        .cloned()
        .collect()
}

/// True when a repository pinned with `pinned` still has the key `now` for Harness Memory: the
/// same key, or both made from a path (a linked worktree of a repository with no remote has its
/// own path key), or neither is a key (a folder with no project).
fn same_key(pinned: Option<&str>, now: Option<&str>) -> bool {
    match (pinned, now) {
        (None, None) => true,
        (Some(pinned), Some(now)) => pinned == now || (is_path_key(pinned) && is_path_key(now)),
        _ => false,
    }
}

fn is_path_key(key: &str) -> bool {
    key.starts_with("path:")
}
