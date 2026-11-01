//! The files and folders the user picked or dropped to import (`knowledge_pick_files`,
//! `knowledge_pick_folder`, `knowledge_drop_intake`): checked here, kept here, and named to
//! [`knowledge_import`](super::knowledge_import) by a token.
//!
//! The webview never names a path to import. A native picker or the window's native drop gives the
//! shell the paths, [`KnowledgeIntakes::put`] checks each one without following links and keeps the
//! good ones, and the import takes them back by the token it returned. A path is kept only when it
//! is absolute and exists as a regular file or a folder: a symbolic link, a device or a pipe is
//! left out with its reason, and so is every file beyond [`MAX_INTAKE_FILES`] (the most an import
//! takes). A folder is walked by the server, which skips links, hidden entries, `.git`,
//! `node_modules` and `target`, goes at most 16 levels deep and counts its files towards the same
//! limit.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use crate::error::{BusinessError, Result};
use crate::model::{KnowledgeIntake, KnowledgeIntakeEntry, KnowledgeSkipReason, KnowledgeSkipped};

/// The most files and folders one intake keeps, and the most files an import takes.
pub const MAX_INTAKE_FILES: usize = 5000;

/// The most paths of one drop that are looked at (a drop of a whole disk is not an import).
pub const MAX_DROPPED_PATHS: usize = 20_000;

/// How many intakes are kept for their import (the oldest goes first).
const KEPT: usize = 8;

/// The extensions the file picker offers, one entry per format an import reads: Markdown, text,
/// HTML (and Confluence exports), OpenAPI (JSON and YAML), AsciiDoc, reStructuredText, PDF and subtitle
/// transcripts (WebVTT, SRT).
pub const IMPORT_EXTENSIONS: [&str; 19] = [
    "md", "markdown", "mdown", "mkd", "txt", "text", "html", "htm", "xhtml", "json", "yaml", "yml", "adoc", "asciidoc",
    "asc", "rst", "pdf", "vtt", "srt",
];

/// What an import is given: the checked paths of one intake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntakePaths {
    /// The files, as absolute paths.
    pub files: Vec<String>,
    /// The folders, as absolute paths.
    pub folders: Vec<String>,
}

/// The intakes waiting for their import, and the last drop waiting to be taken.
#[derive(Debug, Default)]
pub struct KnowledgeIntakes {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    kept: VecDeque<(String, IntakePaths)>,
    dropped: Option<Vec<PathBuf>>,
}

impl KnowledgeIntakes {
    /// No intake yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks `paths` (see the module docs) and keeps the good ones under a new token. An intake
    /// with nothing good in it is still answered (with what was left out), but not kept.
    pub fn put(&self, paths: &[PathBuf]) -> KnowledgeIntake {
        let checked = check_paths(paths);
        let mut state = self.lock();

        let token = format!("intake-{}", uuid::Uuid::new_v4().simple());

        if !checked.files.is_empty() || !checked.folders.is_empty() {
            let kept = IntakePaths {
                files: checked.files.iter().map(|entry| entry.path.clone()).collect(),
                folders: checked.folders.iter().map(|entry| entry.path.clone()).collect(),
            };

            state.kept.push_back((token.clone(), kept));

            while state.kept.len() > KEPT {
                state.kept.pop_front();
            }
        }

        KnowledgeIntake {
            token,
            files: checked.files,
            folders: checked.folders,
            skipped: checked.skipped,
        }
    }

    /// The paths kept under `token`. `notFound` for one that was never given or is gone.
    pub fn get(&self, token: &str) -> Result<IntakePaths> {
        self.lock()
            .kept
            .iter()
            .find(|(kept, _)| kept == token)
            .map(|(_, paths)| paths.clone())
            .ok_or_else(|| BusinessError::NotFound("that selection of files".to_owned()))
    }

    /// Drops the paths kept under `token`.
    pub fn forget(&self, token: &str) {
        self.lock().kept.retain(|(kept, _)| kept != token);
    }

    /// Remembers what the window's native drop brought (at most [`MAX_DROPPED_PATHS`] paths), for
    /// [`Self::take_drop`]. A newer drop replaces it.
    pub fn stash_drop(&self, paths: &[PathBuf]) {
        let paths = paths.iter().take(MAX_DROPPED_PATHS).cloned().collect();

        self.lock().dropped = Some(paths);
    }

    /// The last drop as an intake (see [`Self::put`]), once: `None` when nothing was dropped since.
    pub fn take_drop(&self) -> Option<KnowledgeIntake> {
        let paths = self.lock().dropped.take()?;

        Some(self.put(&paths))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What checking a list of paths found.
#[derive(Debug, Default)]
pub struct Checked {
    /// The regular files.
    pub files: Vec<KnowledgeIntakeEntry>,
    /// The folders.
    pub folders: Vec<KnowledgeIntakeEntry>,
    /// What was left out and why.
    pub skipped: Vec<KnowledgeSkipped>,
}

/// Looks at each of `paths` without following links (see the module docs). Repeats count once.
pub fn check_paths(paths: &[PathBuf]) -> Checked {
    let mut checked = Checked::default();
    let mut seen: Vec<&Path> = Vec::new();

    for path in paths {
        if seen.contains(&path.as_path()) {
            continue;
        }

        seen.push(path);

        let name = name_of(path);
        let entries = checked.files.len() + checked.folders.len();

        match kind_of(path) {
            Err(reason) => checked.skipped.push(KnowledgeSkipped { name, reason }),
            Ok(_) if entries >= MAX_INTAKE_FILES => checked.skipped.push(KnowledgeSkipped {
                name,
                reason: KnowledgeSkipReason::TooMany,
            }),
            Ok(Found::File(bytes)) => checked.files.push(entry(path, name, Some(bytes))),
            Ok(Found::Folder) => checked.folders.push(entry(path, name, None)),
        }
    }

    checked
}

enum Found {
    File(u64),
    Folder,
}

/// What `path` is, without following a link; the reason it cannot be imported otherwise.
fn kind_of(path: &Path) -> std::result::Result<Found, KnowledgeSkipReason> {
    if !path.is_absolute() {
        return Err(KnowledgeSkipReason::Missing);
    }

    let metadata = std::fs::symlink_metadata(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => KnowledgeSkipReason::Missing,
        _ => KnowledgeSkipReason::Unreadable,
    })?;

    let kind = metadata.file_type();

    if kind.is_symlink() {
        Err(KnowledgeSkipReason::Symlink)
    } else if kind.is_file() {
        Ok(Found::File(metadata.len()))
    } else if kind.is_dir() {
        Ok(Found::Folder)
    } else {
        Err(KnowledgeSkipReason::NotFileOrFolder)
    }
}

fn entry(path: &Path, name: String, bytes: Option<u64>) -> KnowledgeIntakeEntry {
    KnowledgeIntakeEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        bytes,
    }
}

fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}
