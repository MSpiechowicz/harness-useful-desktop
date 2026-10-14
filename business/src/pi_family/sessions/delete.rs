//! `session_delete`: removes a saved session the way OMP's `deleteSessionWithArtifacts` does.
//!
//! The session file is unlinked first, then its companion directory (the file name without
//! `.jsonl`) and finally stale rewrite backups (`<file name>.<snowflake>.bak`), which the
//! harness pickers would otherwise restore. Symlinks are never followed: a linked companion
//! directory or backup is left alone, and the session file itself is resolved like
//! [`find_session`] does, which only matches regular files.
//!
//! Only a real session is deleted: its id must name exactly one file, that file must be named
//! the way the harnesses name session files (`<timestamp>_<id>.jsonl`, the timestamp as OMP's
//! and PI's `toISOString().replace(/[:.]/g, "-")`, e.g. `2026-10-09T10-00-00-000Z`) and start
//! with the session header of that id (the rule the listing uses). A configured session folder
//! may hold other `*_<id>.jsonl` files.

use std::fs;
use std::path::Path;

use super::scan::{copy_parent, has_session_header};
use super::{MAX_ENTRIES, find_only_session, find_session};
use crate::error::{BusinessError, Result};

/// Deletes the session `id` from the session folder `dir` with its companion directory and
/// backups. With `empty_copy_only`, a session that is not an empty copy (a bare copy whose
/// parent is in `dir`, see [`SessionSummary::empty_copy`](crate::model::SessionSummary)) fails
/// with `InvalidInput` and nothing is removed.
///
/// Errors: `InvalidInput` for an unsafe id, an id carried by several files, a file not named
/// like a harness session file or without the session's header, `NotFound` for an unknown id, `Io` when the session file or its
/// companion directory cannot be removed. Backups are removed best-effort.
pub fn delete_session(dir: &Path, id: &str, empty_copy_only: bool) -> Result<()> {
    let path = find_only_session(dir, id)?;
    // `find_only_session` only returns `<stem>.jsonl` names.
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    let stem = name.strip_suffix(".jsonl").unwrap_or(name);

    if !is_harness_stem(stem, id) {
        return Err(BusinessError::InvalidInput(format!(
            "the file of session {id} is not named like a session file"
        )));
    }

    if !has_session_header(&path, id) {
        return Err(BusinessError::InvalidInput(format!(
            "the file of session {id} is not a session"
        )));
    }

    if empty_copy_only && !is_empty_copy(dir, id, &path) {
        return Err(BusinessError::InvalidInput(format!(
            "session {id} is not an empty copy"
        )));
    }

    fs::remove_file(&path).map_err(|error| BusinessError::from_io(format!("deleting session {id}"), error))?;

    let companion = dir.join(stem);

    // `symlink_metadata` does not follow links, so a linked directory is not a directory here.
    if fs::symlink_metadata(&companion).is_ok_and(|metadata| metadata.is_dir()) {
        fs::remove_dir_all(&companion).map_err(|error| {
            BusinessError::from_io(format!("session {id} was deleted but not its companion folder"), error)
        })?;
    }

    remove_backups(dir, name);
    Ok(())
}

/// True for `<timestamp>_<id>` with a file-safe ISO timestamp (`YYYY-MM-DDTHH-MM-SS-mmmZ`).
fn is_harness_stem(stem: &str, id: &str) -> bool {
    const SHAPE: &[u8] = b"0000-00-00T00-00-00-000Z";

    let Some((timestamp, rest)) = stem.split_at_checked(SHAPE.len()) else {
        return false;
    };

    let shaped = timestamp.bytes().zip(SHAPE).all(|(byte, &expected)| match expected {
        b'0' => byte.is_ascii_digit(),
        _ => byte == expected,
    });

    shaped && rest.strip_prefix('_') == Some(id)
}

/// True when the session file at `path` is a bare copy whose parent is another session of `dir`.
fn is_empty_copy(dir: &Path, id: &str, path: &Path) -> bool {
    copy_parent(path).is_some_and(|parent| parent != id && find_session(dir, &parent).is_ok())
}

/// Removes the regular files `<name>.*.bak` of `dir`, ignoring failures.
fn remove_backups(dir: &Path, name: &str) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let prefix = format!("{name}.");

    for entry in entries.take(MAX_ENTRIES).flatten() {
        let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        let file_name = entry.file_name();
        let is_backup = file_name
            .to_str()
            .is_some_and(|candidate| candidate.starts_with(&prefix) && candidate.ends_with(".bak"));

        if is_file && is_backup {
            fs::remove_file(entry.path()).ok();
        }
    }
}
