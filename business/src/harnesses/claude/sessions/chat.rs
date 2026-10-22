//! What the Claude Code chat engine reads from a session file: the fork point of a message and the
//! messages of the transcript, the same active path the Sessions view shows ([`transcript`]).
//!
//! - **Fork point:** a fork keeps the turn of the message it starts from: the user's prompt with
//!   everything up to the next prompt (`--resume-session-at` takes the last `user` or `assistant`
//!   line of it, never a command's output). Claude Code only finds lines after the session's last
//!   compaction boundary, so a message before it is refused, as is a message that is not on the
//!   active path. The point is a line `uuid` the file names, so it must be a session UUID before it
//!   reaches a command line.
//! - **Messages:** the transcript, cut after the last message of a fork point when one is named (a
//!   fork whose own file Claude Code has not written yet shows what it copied).

use std::path::{Path, PathBuf};

use super::entries::{Index, Kind};
use super::transcript::transcript;
use super::{find_project_session, is_session_id};
use crate::error::{BusinessError, Result};
use crate::model::ChatMessage;

/// Entries indexed at most when locating a fork point.
const MAX_INDEXED_ENTRIES: usize = 500_000;

/// The session file of session `id` in `dir`, the session folder of `project`; a session of another
/// folder with the same slug is not found ([`find_project_session`]).
pub fn session_file(dir: &Path, project: &Path, id: &str) -> Result<PathBuf> {
    find_project_session(dir, project, id)
}

/// The `uuid` a fork from message `entry_id` of the session file at `path` resumes at (see the
/// module docs).
pub fn fork_point(path: &Path, entry_id: &str) -> Result<String> {
    let index = Index::read(path, MAX_INDEXED_ENTRIES)?;
    let active = index.active_path();

    let Some(start) = active.iter().position(|&node| index.nodes[node].id == entry_id) else {
        return Err(BusinessError::NotFound(format!("message {entry_id} in this chat")));
    };

    let mut point = None;

    for (offset, &position) in active[start..].iter().enumerate() {
        let node = &index.nodes[position];

        if offset > 0 && starts_turn(node.kind) {
            break;
        }

        if node.kind != Kind::LocalOutput {
            point = Some(node);
        }
    }

    let Some(point) = point else {
        return Err(BusinessError::InvalidInput(
            "a chat forks at a message, not at a command's output".to_owned(),
        ));
    };

    let last_line = point.lines.last().copied().unwrap_or_default();

    if index.last_boundary.is_some_and(|boundary| last_line < boundary) {
        return Err(BusinessError::InvalidInput(
            "Claude Code forks only after the chat's last compaction. Pick a later message".to_owned(),
        ));
    }

    if !is_session_id(&point.id) {
        return Err(BusinessError::Protocol(
            "the session file names its message with an unexpected id".to_owned(),
        ));
    }

    Ok(point.id.clone())
}

/// The transcript of the session file at `path`, oldest first, up to and with the last message of
/// `cut` when one is named (and found).
/// Only the kept messages are copied out of the cached transcript.
pub fn chat_messages(path: &Path, cut: Option<&str>) -> Result<Vec<ChatMessage>> {
    let messages = transcript(path)?;

    let end = cut
        .and_then(|cut| messages.iter().rposition(|message| message.id.as_deref() == Some(cut)))
        .map_or(messages.len(), |end| end + 1);

    Ok(messages[..end].to_vec())
}

/// True for an entry that begins a turn of its own.
fn starts_turn(kind: Kind) -> bool {
    matches!(kind, Kind::User | Kind::Command | Kind::Bash | Kind::Compaction)
}
