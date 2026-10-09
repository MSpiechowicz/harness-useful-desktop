//! Resolving an opaque session id to its session file.
//!
//! OMP and PI both store sessions as `<sessions dir>/<encoded cwd>/<timestamp>_<id>.jsonl`. The
//! lookup is a bounded, shallow scan of file names only: no file is opened, companion
//! directories are not entered, symlinks are not followed and deny-listed paths are skipped.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{BusinessError, Result};
use crate::security::is_denied;

/// Directory entries examined at most per lookup.
const MAX_ENTRIES: usize = 100_000;

/// Longest accepted session id.
const MAX_ID_LEN: usize = 128;

/// True for a session id that is safe as a file-name suffix and as a CLI argument: 1 to 128 of
/// `A-Z a-z 0-9 . _ -`, not starting with `-` or `.`.
pub fn is_safe_session_id(id: &str) -> bool {
    let allowed = |character: char| character.is_ascii_alphanumeric() || "._-".contains(character);

    !id.is_empty() && id.len() <= MAX_ID_LEN && !id.starts_with(['-', '.']) && id.chars().all(allowed)
}

/// The session file whose name ends with `_<id>.jsonl` under `sessions_dir`.
pub fn find_session_file(sessions_dir: &Path, id: &str) -> Result<PathBuf> {
    if !is_safe_session_id(id) {
        return Err(BusinessError::InvalidInput("invalid session id".to_owned()));
    }

    let suffix = format!("_{id}.jsonl");
    let mut budget = MAX_ENTRIES;

    for project_dir in directories(sessions_dir, &mut budget) {
        let Ok(entries) = fs::read_dir(&project_dir) else {
            continue;
        };

        for entry in entries.flatten() {
            if budget == 0 {
                return Err(not_found(id));
            }
            budget -= 1;

            let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
            let matches = entry.file_name().to_str().is_some_and(|name| name.ends_with(&suffix));
            let path = entry.path();

            if is_file && matches && is_denied(&path).is_none() {
                return Ok(path);
            }
        }
    }

    Err(not_found(id))
}

/// The real (non-symlink) subdirectories of `dir`.
fn directories(dir: &Path, budget: &mut usize) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut found = Vec::new();

    for entry in entries.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;

        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            found.push(entry.path());
        }
    }

    found
}

fn not_found(id: &str) -> BusinessError {
    BusinessError::NotFound(format!("session {id}"))
}
