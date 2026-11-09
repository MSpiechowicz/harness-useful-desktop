//! The searchable list: the listed files of a project plus the folders that hold them.
//!
//! A path is listed only when it is safe to put in a prompt and not on the deny-list. A folder is
//! derived from the listable files below it, so a folder that holds nothing but denied files is never
//! offered, and neither is a denied folder. Nothing in a `.ssh` folder is offered either: the shared
//! deny-list names its keys, not the rest of it (`config`, `known_hosts`, `authorized_keys`). Nor is
//! anything in a `.git` folder (its `config` can hold a token), however it is spelled.
//!
//! The list comes from a `.git/index` or a folder the project does not control, so its names are
//! untrusted: see [`is_listable`] for what a name may not hold and how long it may be. At most
//! 50,000 folders and 16 MiB of folder paths are derived in all, the shallow ones first (see
//! [`folders`]), so no index costs more than a fixed amount.

use std::collections::HashSet;
use std::path::Path;

use crate::security::is_denied;

mod folders;
mod names;

/// The longest path listed, in bytes (what a diff path may be as well).
const MAX_PATH_BYTES: usize = 4096;

/// The most folders and files a path may nest.
const MAX_COMPONENTS: usize = 128;

/// One file or folder of the list.
pub(super) struct Entry {
    /// Project-relative path with `/` and no trailing `/`.
    pub(super) path: Box<str>,
    pub(super) is_dir: bool,
    pub(super) changed: bool,
    /// Number of `/` in the path, saturating: a shallower path ranks first among equals.
    pub(super) depth: u8,
}

impl Entry {
    fn new(path: &str, is_dir: bool, changed: bool) -> Self {
        let depth = path.bytes().filter(|&byte| byte == b'/').count();

        Self {
            path: path.into(),
            is_dir,
            changed,
            depth: u8::try_from(depth).unwrap_or(u8::MAX),
        }
    }
}

/// The paths `git status` reports as changed, and the tracked files it reports as deleted.
#[derive(Default)]
pub(super) struct Changed {
    paths: HashSet<String>,
    /// Tracked files the working tree no longer has: they are not listed.
    deleted: HashSet<String>,
    /// Untracked folders status listed as one entry (`dir/`): everything below them is untracked.
    folders: HashSet<String>,
}

impl Changed {
    pub(super) fn add(&mut self, path: &str) {
        if path.ends_with('/') {
            self.folders.insert(path.to_owned());
        } else {
            self.paths.insert(path.to_owned());
        }
    }

    /// `path` is a tracked file that was deleted from the working tree: it is left out of the list
    /// (a search would only drop it on the disk check, after the changed files crowded the best
    /// results out) and is not a changed file.
    pub(super) fn add_deleted(&mut self, path: &str) {
        self.deleted.insert(path.to_owned());
    }

    fn contains(&self, path: &str) -> bool {
        if self.paths.contains(path) {
            return true;
        }

        path.match_indices('/')
            .any(|(slash, _)| self.folders.contains(&path[..=slash]))
    }
}

/// The files among `paths` that can be offered, then the folders that hold them. A path another
/// path repeats is listed once (git lists a conflicted file once per stage).
pub(super) fn entries<'a>(paths: impl Iterator<Item = &'a str>, changed: &Changed) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut listed_folders: HashSet<&str> = HashSet::new();
    let mut parents: Vec<&str> = Vec::new();
    let mut known_parents: HashSet<&str> = HashSet::new();
    let mut previous = "";

    for listed in paths {
        // `ls-files --others` prints a nested git repository as `dir/`: it is offered as a folder.
        let (path, is_dir) = match listed.strip_suffix('/') {
            Some(folder) => (folder, true),
            None => (listed, false),
        };

        if listed == previous || (!is_dir && changed.deleted.contains(path)) || !is_listable(path) {
            continue;
        }

        previous = listed;

        if is_dir {
            // A folder git printed whole is listed once, and not derived from the files below it.
            if !listed_folders.insert(path) {
                continue;
            }

            entries.push(Entry::new(path, true, changed.contains(listed)));
        } else {
            entries.push(Entry::new(path, false, changed.contains(path)));
        }

        if let Some((parent, _)) = path.rsplit_once('/')
            && known_parents.insert(parent)
        {
            parents.push(parent);
        }
    }

    let derived = folders::derive(parents, &listed_folders);
    entries.extend(derived.into_iter().map(|folder| Entry::new(folder, true, false)));

    entries
}

/// True for a path that is a plain project-relative one and that the deny-list does not name.
///
/// The length and the number of components are checked first, so a path that is too long costs
/// nothing more. The path may not hold a control character, a replacement character (bytes that are
/// no text), an invisible or bidirectional one or a `\`, apart from the few that real names use
/// in the places they belong (see [`names::is_clean`]). Every component must be a name: not empty,
/// not `.` or `..`, with no whitespace at its start or end (a folder named ` ` would otherwise put a
/// space in front of a `/` command). On Windows no component may hold a `:` (a drive-relative path, an
/// NTFS stream). Elsewhere a colon is an ordinary character (`a:b.md`).
fn is_listable(path: &str) -> bool {
    if path.len() > MAX_PATH_BYTES || path.bytes().filter(|&byte| byte == b'/').count() >= MAX_COMPONENTS {
        return false;
    }

    if !names::is_clean(path) {
        return false;
    }

    path.split('/').all(is_name) && is_offered(path)
}

/// True for one component of a project-relative path.
fn is_name(component: &str) -> bool {
    if matches!(component, "" | "." | "..") {
        return false;
    }

    if component.starts_with(char::is_whitespace) || component.ends_with(char::is_whitespace) {
        return false;
    }

    // A drive-relative path (`C:x`) and an NTFS stream (`a:b`) are real only on Windows.
    !(cfg!(windows) && component.contains(':'))
}

/// True when the deny-list does not name `path` and no folder of it is `.ssh` or `.git`, however
/// their case is written.
pub(super) fn is_offered(path: &str) -> bool {
    let private = path
        .split('/')
        .any(|component| component.eq_ignore_ascii_case(".ssh") || component.eq_ignore_ascii_case(".git"));

    !private && is_denied(Path::new(path)).is_none()
}
