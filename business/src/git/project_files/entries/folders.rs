//! The folders above the parents of the listed files, shallowest first.
//!
//! The folders are found level by level: the top ones of every parent, then the second ones, and so
//! on. When the repository holds more folders than [`MAX_DERIVED_FOLDERS`], the shallow ones are the
//! ones offered, wherever their files sit in git's order. A folder is named by the folder above it and
//! its own name (a group number and a short component), so no long prefix is hashed or copied, and a
//! folder many parents share is found once. Nothing here is an error: the folders stop when a limit
//! is hit and the files stay listed. Besides the number of folders offered, the bytes of every folder
//! path looked at are limited ([`MAX_FOLDER_BYTES`]), the refused ones too: checking a folder against
//! the deny-list reads its whole path.

use std::collections::HashSet;
use std::collections::hash_map::{Entry as Slot, HashMap};

use super::is_offered;

/// The most folders derived in all. Only a folder that is offered counts.
const MAX_DERIVED_FOLDERS: usize = 50_000;

/// The most bytes of folder paths looked at in all, whether the folder is offered or refused: a few
/// hundred paths of 4 KB hold millions of folder bytes, and each one is checked against the deny-list.
const MAX_FOLDER_BYTES: usize = 16 * 1024 * 1024;

/// The most (parent, level) pairs looked at in all: a crafted list of 200,000 files that are 128
/// folders deep costs no more than this. Every folder check of the deny-list is one of them.
const MAX_STEPS: usize = 2_000_000;

/// Where the search for the folders of one parent has got to.
struct Climb<'a> {
    parent: &'a str,
    /// The folder above, as numbered on the previous level (0 for the project itself).
    group: usize,
    /// Where the next component of `parent` starts.
    start: usize,
}

/// The offered folders above `parents` (distinct folder paths), shallowest first, except the ones in
/// `listed`: git printed those as folders itself.
pub(super) fn derive<'a>(parents: Vec<&'a str>, listed: &HashSet<&str>) -> Vec<&'a str> {
    let mut climbs: Vec<Climb<'a>> = parents
        .into_iter()
        .map(|parent| Climb {
            parent,
            group: 0,
            start: 0,
        })
        .collect();
    let mut folders = Vec::new();
    let mut steps = 0;
    let mut bytes = 0;

    while !climbs.is_empty() {
        let mut level: HashMap<(usize, &str), usize> = HashMap::new();
        let mut deeper = Vec::new();

        for climb in climbs {
            if steps >= MAX_STEPS {
                return folders;
            }

            steps += 1;

            let rest = &climb.parent[climb.start..];
            let end = rest.find('/').map_or(climb.parent.len(), |slash| climb.start + slash);
            let number = level.len() + 1;

            let group = match level.entry((climb.group, &climb.parent[climb.start..end])) {
                Slot::Occupied(known) => *known.get(),
                Slot::Vacant(new) => {
                    new.insert(number);

                    let folder = &climb.parent[..end];

                    bytes += folder.len();

                    if bytes > MAX_FOLDER_BYTES {
                        return folders;
                    }

                    if !listed.contains(folder) && is_offered(folder) {
                        if folders.len() >= MAX_DERIVED_FOLDERS {
                            return folders;
                        }

                        folders.push(folder);
                    }

                    number
                }
            };

            if end < climb.parent.len() {
                deeper.push(Climb {
                    parent: climb.parent,
                    group,
                    start: end + 1,
                });
            }
        }

        climbs = deeper;
    }

    folders
}
