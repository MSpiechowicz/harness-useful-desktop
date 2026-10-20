//! `harness_session_totals`: how many saved sessions a PI-family harness holds across every
//! project, and when one was last written, from directory entries and file metadata alone.
//!
//! A session is counted by the listing's file rule (see [`scan`](super::scan)): a top-level,
//! non-hidden, non-deny-listed regular file named `<timestamp>_<id>.jsonl` with a usable id.
//! Files are never opened, so empty files are skipped but drafts and empty copies (left out or
//! flagged by `sessions_list` after reading them) are counted. Symlinks are never followed below
//! the store itself, and companion directories are never entered.
//!
//! The walk examines at most a given number of directory entries in total; past it, the totals
//! are marked `capped`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::session_id_of;
use crate::agent::rfc3339;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::SessionTotals;
use crate::security::is_denied;

/// Directory entries examined at most per count, across every folder of the store.
pub const MAX_TOTALS_ENTRIES: usize = 200_000;

/// Where a harness keeps the session files of every project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStore {
    /// One folder per project below this root (the harness default).
    PerProject(PathBuf),
    /// One configured folder that holds the sessions of every project.
    Shared(PathBuf),
}

/// The store a configured session directory `raw` names, without a project: `~` is expanded; a
/// relative value resolves against each project, so it has no machine-wide store
/// (`Unsupported`).
pub fn configured_store(env: &Env, raw: &str) -> Result<SessionStore> {
    let expanded = env.expand_home(raw.trim());

    if !expanded.is_absolute() {
        return Err(BusinessError::Unsupported(
            "the configured session folder depends on the project".to_owned(),
        ));
    }

    Ok(SessionStore::Shared(expanded))
}

/// The totals of `store`, examining at most `max_entries` directory entries. A missing or
/// unreadable store has no sessions.
pub fn session_totals(store: &SessionStore, max_entries: usize) -> SessionTotals {
    let mut walk = Walk {
        budget: max_entries,
        count: 0,
        newest: None,
        capped: false,
    };

    match store {
        SessionStore::Shared(dir) => walk.files(dir),
        SessionStore::PerProject(root) => walk.projects(root),
    }

    SessionTotals {
        count: u32::try_from(walk.count).unwrap_or(u32::MAX),
        last_used: walk.newest.map(rfc3339),
        capped: walk.capped,
    }
}

/// The running totals of one count.
struct Walk {
    /// Directory entries that may still be examined.
    budget: usize,
    count: usize,
    newest: Option<SystemTime>,
    capped: bool,
}

impl Walk {
    /// Counts the session files of each project folder below `root` (real directories only).
    fn projects(&mut self, root: &Path) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };

        let mut folders = Vec::new();

        for entry in entries.flatten() {
            if !self.take() {
                return;
            }

            // `file_type` does not follow symlinks, so linked folders are skipped.
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                folders.push(entry.path());
            }
        }

        for folder in folders {
            if self.capped {
                return;
            }

            self.files(&folder);
        }
    }

    /// Counts the session files directly in `dir`.
    fn files(&mut self, dir: &Path) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            if !self.take() {
                return;
            }

            if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }

            let path = entry.path();
            let is_session = entry
                .file_name()
                .to_str()
                .is_some_and(|name| !name.starts_with('.') && session_id_of(name).is_some());

            if !is_session || is_denied(&path).is_some() {
                continue;
            }

            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if metadata.len() == 0 {
                continue;
            }

            let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            self.count += 1;
            self.newest = self.newest.max(Some(modified));
        }
    }

    /// Spends one entry of the budget; false (and capped) once it is used up.
    fn take(&mut self) -> bool {
        if self.budget == 0 {
            self.capped = true;
            return false;
        }

        self.budget -= 1;
        true
    }
}
