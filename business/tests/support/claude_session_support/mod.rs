//! A temporary home with the Claude Code session fixtures (`tests/fixtures/sessions/claude`,
//! built from the scrubbed recordings in `tests/fixtures/claude/sessions` plus one synthetic
//! session) in the session folder of the project `<home>/Projects/demo`.
//!
//! The fixtures record `/home/demo/Projects/demo` as the working directory and in tool paths;
//! installing points them at the test project. File times are fixed, newest first in [`NEWEST_FIRST`].
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};

use harness_business::harnesses::Claude;
use harness_business::model::{HarnessId, SessionRef};

use crate::common::DEMO_PROJECT;
use crate::session_support::{SessionHome, session_home, set_mtime};

/// The synthetic session: edits, a branch, a slash command, a split shell run, an image, subagent
/// and hidden lines, an `ai-title`.
pub const EDITS: &str = "5e1f0c2a-7b3d-4c8e-9f10-2a3b4c5d6e7f";

/// A renamed session (`custom-title` lines and the rename sidecar).
pub const RENAMED: &str = "e3e8d4ad-b257-4fdb-83a3-973cd8a3af62";

/// A session with a shell run injected as a user line.
pub const SHELL: &str = "d5ef9b67-4fda-4682-aac2-8f9f5966c2d5";

/// A fork of [`QUEUE`] cut after its first turn.
pub const FORK: &str = "44b8b4df-227b-4f61-98d1-c42cf465a870";

/// A session with a steering message and a follow-up, the parent of [`FORK`].
pub const QUEUE: &str = "e52c9fea-232a-48d5-a135-a564fe56012e";

/// A clone of [`COMPACTED`] made after its compaction.
pub const CLONE: &str = "acf5e683-2293-45ef-a9ac-d3016775fdbc";

/// A session compacted with `/compact`.
pub const COMPACTED: &str = "c1262da3-a66b-42b5-a9ee-32bb98f2045d";

/// A session with Read, Write and a declined Bash call.
pub const TOOLS: &str = "0a433744-8ef1-4d3c-861b-8ff5bc0e302c";

/// Every fixture session, newest first, with its file time.
pub const NEWEST_FIRST: [(&str, &str); 8] = [
    (EDITS, "2026-10-22T09:00:00Z"),
    (RENAMED, "2026-10-22T08:00:00Z"),
    (SHELL, "2026-10-22T07:50:00Z"),
    (FORK, "2026-10-22T07:45:00Z"),
    (QUEUE, "2026-10-22T07:40:00Z"),
    (CLONE, "2026-10-22T07:39:00Z"),
    (COMPACTED, "2026-10-22T07:38:00Z"),
    (TOOLS, "2026-10-22T07:36:00Z"),
];

/// A home with the fixtures installed.
pub struct ClaudeHome {
    pub home: SessionHome,
    /// The project's session folder.
    pub dir: PathBuf,
    /// The Claude config directory (`<home>/.claude`).
    pub config: PathBuf,
}

/// A home with the fixtures in the demo project's session folder.
pub fn claude_home() -> ClaudeHome {
    let home = session_home();
    let dir = home.install(&Claude, "claude");

    for (id, time) in NEWEST_FIRST {
        let path = dir.join(format!("{id}.jsonl"));
        replace_in(&path, DEMO_PROJECT, &json_text(&home.project));
        set_mtime(&path, time);
    }

    let config = home.home.join(".claude");

    ClaudeHome { home, dir, config }
}

impl ClaudeHome {
    /// The session file of `id`.
    pub fn file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.jsonl"))
    }
}

/// A Claude Code session reference.
pub fn claude(id: &str) -> SessionRef {
    SessionRef {
        harness: HarnessId::Claude,
        id: id.to_owned(),
    }
}

/// `path` as text inside a JSON string (backslashes escaped).
pub fn json_text(path: &Path) -> String {
    let quoted = serde_json::to_string(&path.to_string_lossy()).expect("path as JSON");

    quoted[1..quoted.len() - 1].to_owned()
}

/// Replaces every `from` in the file at `path` with `to`, keeping its modification time.
pub fn replace_in(path: &Path, from: &str, to: &str) {
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .expect("file time");
    let text = fs::read_to_string(path).expect("read fixture");

    fs::write(path, text.replace(from, to)).expect("write fixture");
    fs::File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_modified(modified))
        .expect("keep file time");
}

/// Appends `line` (one JSON object) to the file at `path`.
pub fn append_line(path: &Path, line: &serde_json::Value) {
    let mut text = fs::read_to_string(path).expect("read fixture");
    text.push_str(&line.to_string());
    text.push('\n');

    fs::write(path, text).expect("write fixture");
}
