//! The files of a folder that is no git repository, found by walking it.
//!
//! The walk is bounded, not complete: it goes level by level (shallow files first), never follows
//! a symbolic link, skips the folders no one wants to mention (`.git`, `node_modules`, `target`,
//! `dist`, `build`, `.venv`, `__pycache__`), skips names that are not UTF-8, goes at most
//! [`MAX_WALK_DEPTH`] folders deep and looks at no more than [`MAX_WALK_ENTRIES`] directory entries.
//! Hitting a limit ends the walk without an error.

use std::collections::VecDeque;
use std::fs::{self, DirEntry};
use std::path::Path;

/// Folders below the project the walk enters (the project itself is depth 0).
pub(super) const MAX_WALK_DEPTH: usize = 12;

/// Directory entries the walk looks at, kept or skipped.
pub(super) const MAX_WALK_ENTRIES: usize = 20_000;

/// Folder names (compared without regard to case) the walk does not enter, besides `.git`.
const SKIPPED_NAMES: [&str; 6] = ["node_modules", "target", "dist", "build", ".venv", "__pycache__"];

/// The project-relative paths (with `/`) of the regular files below `project`.
pub(super) fn walk(project: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut looked_at = 0;
    let mut queue = VecDeque::from([(String::new(), 0)]);

    while let Some((relative, depth)) = queue.pop_front() {
        let Ok(read) = fs::read_dir(project.join(&relative)) else {
            continue;
        };

        let mut children = Vec::new();
        for child in read.flatten().take(MAX_WALK_ENTRIES - looked_at) {
            looked_at += 1;

            if let Some(listed) = listed(&child) {
                children.push(listed);
            }
        }

        children.sort();

        for (name, is_dir) in children {
            if is_skipped(&name, is_dir) {
                continue;
            }

            let path = if relative.is_empty() {
                name
            } else {
                format!("{relative}/{name}")
            };

            if !is_dir {
                files.push(path);
            } else if depth < MAX_WALK_DEPTH {
                queue.push_back((path, depth + 1));
            }
        }

        if looked_at >= MAX_WALK_ENTRIES {
            break;
        }
    }

    files
}

/// The name of a regular file or folder, and whether it is a folder. `None` for a symbolic link,
/// anything else that is no file or folder, and a name that is not UTF-8.
fn listed(child: &DirEntry) -> Option<(String, bool)> {
    let kind = child.file_type().ok()?;
    let name = child.file_name().into_string().ok()?;

    (kind.is_dir() || kind.is_file()).then_some((name, kind.is_dir()))
}

/// `.git` (a folder, or the file of a linked work tree) and the folders of [`SKIPPED_NAMES`].
fn is_skipped(name: &str, is_dir: bool) -> bool {
    name == ".git" || (is_dir && SKIPPED_NAMES.iter().any(|skipped| name.eq_ignore_ascii_case(skipped)))
}
