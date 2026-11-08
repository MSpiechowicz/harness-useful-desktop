//! Copying what Harness Memory kept on this computer while the team database was unreachable.
//!
//! With the offline fallback the server saves a note or script of a scope set to `remote` on this
//! computer when the team database cannot be opened, and keeps a change to a team note it cannot
//! make on a copy. Both wait in its data folder ([`memory_data_dir`]) until the team database is
//! back, and every server copies and applies them at its next call. The app does not wait for one:
//!
//! - `pending-sync.json` ([`PENDING_SYNC_FILE`]) lists the notes and scripts saved offline
//!   (`{"notes": [...], "scripts": [...]}`), `pending-updates.json` ([`PENDING_UPDATES_FILE`]) the
//!   changes to team notes (`{"updates": [...]}`). The app only counts their entries
//!   ([`waiting_entries`]): a file that is not a plain file (a symbolic link is not counted), is
//!   larger than [`MAX_WAITING_BYTES`] (the most the server reads) or does not parse waits for
//!   nothing, as it does for the server, and is reported as unreadable;
//! - [`MemorySync::begin`] hands out a turn at most every [`SYNC_INTERVAL`], and never while
//!   another is running or while the Project memory switch is off (`memory_enabled`: nothing is
//!   looked at or run then, and the first turn after it is turned on is due at once). [`SyncTurn::run`] looks at the files and, when at least one entry waits
//!   and the installed server lists `sync-command`, runs `harness-memory sync --json` within
//!   [`SYNC_TIMEOUT`]. It prints `{"online": bool, "waiting": {"notes": N, "scripts": N,
//!   "updates": N}}` (exit 0 also while the database is unreachable): what still waits after one
//!   attempt. A failed run is logged and the next turn tries again.
//!
//! Nothing runs while Harness Memory is not installed or its data folder is missing. Logs carry
//! counts only, never a title, a body or the server's own words.

use std::ffi::OsString;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::Value;

use super::memory_cli::{CliCall, LIST_OUTPUT_BYTES, run_json};
use super::memory_embedder::feature_server;
use super::memory_features::{FeaturesSupport, MemoryFeatures};
use super::memory_requests::memory_data_dir;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::process::ProcessRunner;
use crate::security::SafeReader;

/// The notes and scripts saved offline, in the data folder.
pub const PENDING_SYNC_FILE: &str = "pending-sync.json";

/// The changes to team notes made offline, in the data folder.
pub const PENDING_UPDATES_FILE: &str = "pending-updates.json";

/// How often the app looks for work that waits (60 s, counted from the end of the last look).
pub const SYNC_INTERVAL: Duration = Duration::from_secs(60);

/// How long `sync --json` may take (300 s: every waiting entry is written to a team database that
/// may be far away).
pub const SYNC_TIMEOUT: Duration = Duration::from_secs(300);

/// The largest waiting file read (4 MiB: the most the server reads of either).
pub const MAX_WAITING_BYTES: u64 = 4 * 1024 * 1024;

/// How many entries wait in the data folder, and how many of its files cannot be read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Waiting {
    /// Notes and scripts saved offline, and changes to team notes, together.
    pub entries: usize,
    /// Files that exist but are not read: a link, a folder, too large or not understood.
    pub unreadable: usize,
}

/// `pending-sync.json`, as far as the app reads it.
#[derive(Deserialize)]
struct OfflineSaves {
    #[serde(default)]
    notes: Vec<IgnoredAny>,
    #[serde(default)]
    scripts: Vec<IgnoredAny>,
}

/// `pending-updates.json`, as far as the app reads it.
#[derive(Deserialize)]
struct QueuedUpdates {
    #[serde(default)]
    updates: Vec<IgnoredAny>,
}

/// What waits in `data_dir`: the entries of both files (see the module docs).
pub fn waiting_entries(data_dir: &Path) -> Waiting {
    let saves = count_in(&data_dir.join(PENDING_SYNC_FILE), |text| {
        let saves: OfflineSaves = serde_json::from_str(text).ok()?;

        Some(saves.notes.len() + saves.scripts.len())
    });

    let updates = count_in(&data_dir.join(PENDING_UPDATES_FILE), |text| {
        let updates: QueuedUpdates = serde_json::from_str(text).ok()?;

        Some(updates.updates.len())
    });

    Waiting {
        entries: saves.unwrap_or(0) + updates.unwrap_or(0),
        unreadable: usize::from(saves.is_none()) + usize::from(updates.is_none()),
    }
}

/// The entries `count` finds in the file at `path`: none for a missing or empty file, `None` for
/// one that exists but cannot be counted.
fn count_in(path: &Path, count: impl FnOnce(&str) -> Option<usize>) -> Option<usize> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Some(0),
        Err(_) => return None,
    };

    if !metadata.is_file() || metadata.len() > MAX_WAITING_BYTES {
        return None;
    }

    if metadata.len() == 0 {
        return Some(0);
    }

    let text = SafeReader::read_to_string(path, MAX_WAITING_BYTES).ok()?;

    if text.trim().is_empty() {
        return Some(0);
    }

    count(&text)
}

/// What `sync --json` answered: whether the team database was reachable, and what still waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncReport {
    /// False when the team database could not be reached.
    pub online: bool,
    /// Notes saved offline that wait to be copied.
    pub notes: u64,
    /// Scripts saved offline that wait to be copied.
    pub scripts: u64,
    /// Changes to team notes that wait to be applied.
    pub updates: u64,
}

impl SyncReport {
    /// What still waits, all of it.
    pub fn waiting(self) -> u64 {
        self.notes + self.scripts + self.updates
    }

    /// The report in `data`, or `protocol` when it has no `online`. Other fields are ignored, and a
    /// count the server leaves out is 0.
    fn read(data: &Value) -> Result<Self> {
        let Some(online) = data.get("online").and_then(Value::as_bool) else {
            return Err(BusinessError::Protocol(
                "harness-memory sync answered without `online`".to_owned(),
            ));
        };

        let count = |field: &str| {
            data.get("waiting")
                .and_then(|waiting| waiting.get(field))
                .and_then(Value::as_u64)
                .unwrap_or(0)
        };

        Ok(Self {
            online,
            notes: count("notes"),
            scripts: count("scripts"),
            updates: count("updates"),
        })
    }
}

/// How a [`SyncTurn`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOutcome {
    /// Nothing waits, or Harness Memory is not installed or has no data folder: nothing ran.
    Idle,
    /// Entries wait, but the installed server does not list `sync-command`: nothing ran.
    Unsupported,
    /// `sync --json` ran: how many entries waited before, and what the server says waits after.
    Synced {
        /// The entries counted in the files before the run.
        before: usize,
        /// The answer.
        report: SyncReport,
    },
    /// `sync --json` could not run or answered nothing readable (logged): the next turn tries again.
    Failed,
}

/// When the app last looked for waiting work, and whether it is looking now. See the module docs.
#[derive(Debug, Default)]
pub struct MemorySync {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    running: bool,
    finished: Option<Instant>,
    /// Whether unreadable files were logged (once, until the files can be read again).
    warned: bool,
}

impl MemorySync {
    /// Nothing run yet: the first turn is due at once.
    pub fn new() -> Self {
        Self::default()
    }

    /// A turn when `project_memory` (the Project memory switch) is on, none is running and the last
    /// one ended at least [`SYNC_INTERVAL`] before `now`; `None` otherwise. A refusal for the
    /// switch leaves the interval alone, so the turn is due as soon as the switch is on.
    pub fn begin(self: &Arc<Self>, now: Instant, project_memory: bool) -> Option<SyncTurn> {
        if !project_memory {
            return None;
        }

        let mut state = self.lock();

        if state.running {
            return None;
        }

        let waiting_for_interval = state
            .finished
            .is_some_and(|finished| now.saturating_duration_since(finished) < SYNC_INTERVAL);

        if waiting_for_interval {
            return None;
        }

        state.running = true;

        Some(SyncTurn { owner: self.clone() })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The right to look for waiting work and run `sync --json` once. Dropping it (the turn ended, or
/// its task was cancelled) lets the next one begin after [`SYNC_INTERVAL`].
#[derive(Debug)]
pub struct SyncTurn {
    owner: Arc<MemorySync>,
}

impl Drop for SyncTurn {
    fn drop(&mut self) {
        let mut state = self.owner.lock();

        state.running = false;
        state.finished = Some(Instant::now());
    }
}

impl SyncTurn {
    /// One look at the data folder of `env` and, when entries wait and the installed server
    /// lists `sync-command` (`features`), one run of `sync --json` with `runner`.
    pub async fn run(self, env: &Env, runner: &dyn ProcessRunner, features: &MemoryFeatures) -> SyncOutcome {
        let Some(data_dir) = memory_data_dir(env).filter(|dir| dir.is_dir()) else {
            return SyncOutcome::Idle;
        };

        let waiting = waiting_entries(&data_dir);
        self.note_unreadable(waiting.unreadable);

        if waiting.entries == 0 {
            return SyncOutcome::Idle;
        }

        let server = match feature_server(env, runner, features, FeaturesSupport::sync_command, "syncing").await {
            Ok(server) => server,
            Err(BusinessError::NotFound(_)) => return SyncOutcome::Idle,
            Err(BusinessError::Unsupported(_)) => return SyncOutcome::Unsupported,
            Err(error) => {
                tracing::warn!(code = ?error.code(), "memory sync could not ask the installed server");
                return SyncOutcome::Failed;
            }
        };

        let call = CliCall {
            name: "sync",
            args: ["sync", "--json"].map(OsString::from).into(),
            timeout: SYNC_TIMEOUT,
            output_bytes: LIST_OUTPUT_BYTES,
            stdin: None,
        };

        let answer = run_json(env, runner, &server, call)
            .await
            .and_then(|data| SyncReport::read(&data));

        match answer {
            Ok(report) => {
                tracing::info!(
                    before = waiting.entries,
                    online = report.online,
                    notes = report.notes,
                    scripts = report.scripts,
                    updates = report.updates,
                    "memory sync ran"
                );

                SyncOutcome::Synced {
                    before: waiting.entries,
                    report,
                }
            }
            Err(error) => {
                tracing::warn!(code = ?error.code(), "memory sync failed, it is tried again later");
                SyncOutcome::Failed
            }
        }
    }

    /// Logs once that `unreadable` waiting files are not read; forgets it when they can be.
    fn note_unreadable(&self, unreadable: usize) {
        let mut state = self.owner.lock();

        if unreadable == 0 {
            state.warned = false;
            return;
        }

        if !state.warned {
            state.warned = true;
            tracing::warn!(
                unreadable,
                "memory files of work waiting for the team database are not read"
            );
        }
    }
}
