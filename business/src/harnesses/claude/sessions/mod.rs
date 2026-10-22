//! Claude Code saved sessions, read from the session files (Claude Code 2.1.304).
//!
//! A project's sessions live in `<config>/projects/<slug>/` ([`super::paths`]) as `<uuid>.jsonl`
//! files, one JSON object per line. Next to each file a `<uuid>/` folder may hold subagent
//! transcripts, full tool outputs and the rename sidecar `custom-title.json`; only that sidecar is
//! ever read. Lines of unknown types are ignored.
//!
//! - Message lines (`user`, `assistant`, `system`) carry `uuid`, `parentUuid`, `isSidechain`,
//!   `sessionId`, `cwd`, `timestamp` and `message{role, content}`. They form a tree through
//!   `parentUuid`; the leaf is the last message line of the file. One API reply is split into one
//!   `assistant` line per content block, all with the same `message.id`: they are shown as one
//!   message ([`entries`]).
//! - Never shown: `attachment` lines (the user's CLAUDE.md, hook context, reminders) and
//!   `api-request*` lines (the full system prompt), subagent lines (`isSidechain`), lines only the
//!   model reads (`isMeta`). They stay in the parent chain: a child is attached to the nearest shown
//!   ancestor.
//! - Compaction: a `system` line `compact_boundary` starts a new chain (`parentUuid` null) and
//!   names the last message before it in `logicalParentUuid`; the `user` line after it
//!   (`isCompactSummary`) holds the summary and is shown as the compaction.
//! - Titles: the last `custom-title` line, else the rename sidecar, else the last `ai-title` line
//!   ([`list`]).
//!
//! The webview only sends an opaque session ref; its id must be a lowercase UUID and is resolved by a
//! shallow scan of the project's folder ([`find_session`]), so no other file can be named.
//! Deleting is in [`delete`]. Nothing here is redacted: the transcript is the user's own
//! conversation, as for the other harnesses, and no credential is ever read.

mod delete;
mod entries;
mod list;
mod transcript;

pub(super) use delete::delete_session;
pub(super) use entries::session_tree;
pub(super) use list::list_sessions;
pub(super) use transcript::{session_files_touched, session_messages};

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{BusinessError, Result};
use crate::model::{HarnessId, SessionRef};
use crate::pi_family::sessions::is_safe_session_id;
use crate::security::is_denied;

/// Directory entries examined at most per scan.
const MAX_DIR_ENTRIES: usize = 100_000;

/// The suffix of a session file.
const SESSION_SUFFIX: &str = ".jsonl";

/// The id of `session` when it is a Claude Code session id.
pub(super) fn checked_id(session: &SessionRef) -> Result<&str> {
    if session.harness != HarnessId::Claude {
        return Err(BusinessError::InvalidInput(
            "the session belongs to another harness".to_owned(),
        ));
    }

    if !is_session_id(&session.id) {
        return Err(BusinessError::InvalidInput("invalid session id".to_owned()));
    }

    Ok(&session.id)
}

/// True for a lowercase UUID (`8-4-4-4-12` hex digits), the only ids Claude Code gives sessions.
pub(super) fn is_session_id(id: &str) -> bool {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];

    let lowercase_hex = |group: &str| group.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    let groups: Vec<&str> = id.split('-').collect();

    is_safe_session_id(id)
        && groups.len() == GROUPS.len()
        && groups
            .iter()
            .zip(GROUPS)
            .all(|(group, length)| group.len() == length && lowercase_hex(group))
}

/// The session id of a `<uuid>.jsonl` file name.
pub(super) fn session_id_of(file_name: &str) -> Option<&str> {
    file_name.strip_suffix(SESSION_SUFFIX).filter(|id| is_session_id(id))
}

/// The session file of `id` in the project's session folder `dir`: a top-level regular file named
/// `<id>.jsonl` (symlinks, folders and deny-listed files never match), found by a shallow scan.
pub(super) fn find_session(dir: &Path, id: &str) -> Result<PathBuf> {
    if !is_session_id(id) {
        return Err(BusinessError::InvalidInput("invalid session id".to_owned()));
    }

    let entries = fs::read_dir(dir).map_err(|_| not_found(id))?;

    for entry in entries.take(MAX_DIR_ENTRIES).flatten() {
        // `file_type` does not follow symlinks.
        let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        let name = entry.file_name();
        let matches = name.to_str().and_then(session_id_of) == Some(id);
        let path = entry.path();

        if is_file && matches && is_denied(&path).is_none() {
            return Ok(path);
        }
    }

    Err(not_found(id))
}

fn not_found(id: &str) -> BusinessError {
    BusinessError::NotFound(format!("session {id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ids_are_lowercase_uuids() {
        assert!(is_session_id("c1262da3-a66b-42b5-a9ee-32bb98f2045d"));
        assert!(!is_session_id("C1262DA3-A66B-42B5-A9EE-32BB98F2045D"));
        assert!(!is_session_id("c1262da3a66b42b5a9ee32bb98f2045d"));
        assert!(!is_session_id("c1262da3-a66b-42b5-a9ee-32bb98f2045"));
        assert!(!is_session_id("../1262da3-a66b-42b5-a9ee-32bb98f2045d"));
        assert!(!is_session_id("g1262da3-a66b-42b5-a9ee-32bb98f2045d"));
        assert!(!is_session_id(""));
    }
}
