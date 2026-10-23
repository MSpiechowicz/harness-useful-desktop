//! A thread of `thread/list` or `thread/read` as the Sessions view shows it (`super`).

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use crate::agent::rfc3339;
use crate::codex_rpc::wire::results::Thread;
use crate::model::{HarnessId, MAX_FIRST_PROMPT_CHARS, SessionRef, SessionSummary, SessionTitleSource};
use crate::opencode_server::mapper::project_dir;
use crate::pi_family::sessions::is_safe_session_id;

/// A listed thread as a session with its update time (seconds), when it is a saved thread of
/// `project` (not ephemeral, not a subagent's).
pub(super) fn summary_of(thread: Thread, project: &Path) -> Option<(u64, SessionSummary)> {
    if !is_safe_session_id(&thread.id) || thread.ephemeral || thread.parent_thread_id.is_some() {
        return None;
    }

    project_dir(&thread.cwd, project)?;

    let updated = thread.updated_at.or(thread.created_at).unwrap_or(0);
    let created = thread.created_at.unwrap_or(updated);
    let title = title_of(&thread);

    let summary = SessionSummary {
        title_source: title.as_ref().map(|_| SessionTitleSource::User),
        title,
        first_prompt: prompt_of(&thread),
        created_at: time_text(created),
        updated_at: time_text(updated),
        size_bytes: 0,
        model: thread.model,
        parent_session: thread.forked_from_id.filter(|parent| is_safe_session_id(parent)),
        empty_copy: false,
        routine: None,
        cwd: thread.cwd,
        session: SessionRef {
            harness: HarnessId::Codex,
            id: thread.id,
        },
    };

    Some((updated, summary))
}

/// The name the user gave the thread.
pub(super) fn title_of(thread: &Thread) -> Option<String> {
    let name = thread.name.as_deref()?.trim();

    (!name.is_empty()).then(|| name.to_owned())
}

/// The thread's preview (its first prompt), cut to [`MAX_FIRST_PROMPT_CHARS`].
pub(super) fn prompt_of(thread: &Thread) -> Option<String> {
    let preview = thread.preview.trim();

    (!preview.is_empty()).then(|| preview.chars().take(MAX_FIRST_PROMPT_CHARS).collect())
}

/// Seconds since the epoch as RFC 3339.
fn time_text(seconds: u64) -> String {
    rfc3339(UNIX_EPOCH + Duration::from_secs(seconds))
}
