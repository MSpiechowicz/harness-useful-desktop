//! `<app_local_data_dir>/routine-sessions.json`: `{ "version": 1, "sessions": { "<harness>:<session
//! id>": { "routineId", "runId", "name" } } }`, the harness sessions routine runs started, so
//! Sessions can tag them "Routine" (see "Session tags" in `docs/routines-format.md`). The newest
//! [`MAX_SESSION_TAGS`] entries are kept, in the order they were written. Written by the app
//! window alone, when an agent run's session id is known.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::files::{load_json, read_json, write_json};
use crate::error::Result;
use crate::model::{HarnessId, SessionRoutineTag};

/// The file in the app's local data folder.
pub const SESSIONS_FILE: &str = "routine-sessions.json";

/// Entries kept.
pub const MAX_SESSION_TAGS: usize = 2000;

/// Largest file read.
const MAX_SESSIONS_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionsFile {
    version: u32,
    /// Oldest first (the JSON object keeps the order it was written in).
    sessions: Map<String, Value>,
}

/// The session tags of one local data folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTags {
    path: PathBuf,
}

/// The tags as read, to look sessions up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionTagIndex {
    tags: std::collections::HashMap<String, SessionRoutineTag>,
}

impl SessionTagIndex {
    /// The routine that started session `session_id` of `harness`.
    pub fn get(&self, harness: HarnessId, session_id: &str) -> Option<&SessionRoutineTag> {
        self.tags.get(&key(harness, session_id))
    }

    /// True when no session is tagged.
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }
}

impl SessionTags {
    /// The tags in `<local_data_dir>/routine-sessions.json`.
    pub fn new(local_data_dir: &Path) -> Self {
        Self {
            path: local_data_dir.join(SESSIONS_FILE),
        }
    }

    /// Every tag, read only (none when the file is missing or invalid).
    pub fn index(&self) -> SessionTagIndex {
        let tags = read_json::<SessionsFile>(&self.path, MAX_SESSIONS_FILE_BYTES, |file| file.version == 1)
            .map(|file| {
                file.sessions
                    .into_iter()
                    .filter_map(|(key, value)| Some((key, serde_json::from_value(value).ok()?)))
                    .collect()
            })
            .unwrap_or_default();

        SessionTagIndex { tags }
    }

    /// The routine that started session `session_id` of `harness`.
    pub fn lookup(&self, harness: HarnessId, session_id: &str) -> Option<SessionRoutineTag> {
        self.index().get(harness, session_id).cloned()
    }

    /// Tags session `session_id` of `harness` with `tag` (as the newest entry; the oldest are
    /// dropped past [`MAX_SESSION_TAGS`]). An invalid file is set aside and started again.
    pub fn record(&self, harness: HarnessId, session_id: &str, tag: &SessionRoutineTag) -> Result<()> {
        let file = load_json::<SessionsFile>(&self.path, MAX_SESSIONS_FILE_BYTES, |file| file.version == 1)?;
        let mut sessions = file.map(|file| file.sessions).unwrap_or_default();

        let key = key(harness, session_id);
        sessions.shift_remove(&key);
        sessions.insert(key, serde_json::to_value(tag).unwrap_or(Value::Null));

        let excess = sessions.len().saturating_sub(MAX_SESSION_TAGS);
        let sessions: Map<String, Value> = sessions.into_iter().skip(excess).collect();

        write_json(
            &self.path,
            &SessionsFile { version: 1, sessions },
            "the routine session tags",
        )
    }
}

/// `<harness>:<session id>`.
fn key(harness: HarnessId, session_id: &str) -> String {
    format!("{}:{session_id}", harness.binary_name())
}
