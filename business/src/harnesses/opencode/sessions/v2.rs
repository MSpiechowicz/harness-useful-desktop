//! The OpenCode 2 session commands. Each runs with `--standalone`: without it OpenCode 2 starts its
//! background service (`opencode serve --service`) and leaves it running. OpenCode 2 has no
//! `--pure`.
//!
//! - List: `opencode session list --standalone --format json -n <count>`, run in the project. The
//!   JSON is the same as OpenCode 1's (`[{id, title?, updated, created, projectId, directory}]`,
//!   times in Unix milliseconds, `[]` when empty) and is filtered by directory the same way.
//! - Transcript and edited files: `opencode session export --standalone <id>` (see
//!   [`super::transcript_v2`]).
//! - Delete: `opencode session delete --standalone <id>` (deletes its child sessions too). A
//!   missing session fails with `Session not found: <id>`, which is `NotFound`.
//!
//! Besides the shared spawn settings, the one-shot server behind each command skips the project
//! config (under OpenCode 2's own name too), the model catalog download and the file watcher
//! (`ONE_SHOT_ENV_V2`).

use super::Command;

/// Sessions one list asks for at most. OpenCode 2 lists 100 by default, and the list holds the
/// sessions of the whole OpenCode project, which may span more directories than the app's project.
const LIST_COUNT: &str = "5000";

/// The OpenCode 2 argv of `command`.
pub(super) fn args(command: Command<'_>) -> Vec<&str> {
    match command {
        Command::List => vec!["session", "list", "--standalone", "--format", "json", "-n", LIST_COUNT],
        Command::Export(id) => vec!["session", "export", "--standalone", id],
        Command::Delete(id) => vec!["session", "delete", "--standalone", id],
    }
}
