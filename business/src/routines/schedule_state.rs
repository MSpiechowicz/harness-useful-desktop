//! `<app_local_data_dir>/routine-schedule.json`: `{ "version": 1, "routines": { "<id>": {
//! "sha256", "armedAt", "lastSlot" } } }`, what the scheduler handled (see "Catch-up and the
//! schedule file" in `docs/routines-format.md`). Written by the app window only.
//!
//! - `armedAt`: when this machine first saw these exact bytes runnable, so a routine never runs for
//!   a time before it existed in this form. A byte change, enabling or approval arms it again.
//! - `lastSlot`: the latest slot handled (run, skipped or caught up).
//!
//! An invalid or unreadable file is set aside (`.corrupt-<unix-seconds>`) and rebuilt: every
//! routine is armed again when the scheduler next looks at it ([`super::plan::due`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use super::files::{load_json, write_json};
use crate::error::Result;

/// The file in the app's local data folder.
pub const SCHEDULE_FILE: &str = "routine-schedule.json";

/// Largest schedule file read.
const MAX_SCHEDULE_BYTES: u64 = 1024 * 1024;

/// What the scheduler keeps per routine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleEntry {
    /// The SHA-256 of the routine bytes it was armed for.
    pub sha256: String,
    /// When these bytes were first seen runnable.
    pub armed_at: Timestamp,
    /// The latest slot handled.
    pub last_slot: Option<Timestamp>,
}

/// See the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleState {
    version: u32,
    #[serde(default)]
    routines: BTreeMap<String, ScheduleEntry>,
}

impl Default for ScheduleState {
    fn default() -> Self {
        Self {
            version: 1,
            routines: BTreeMap::new(),
        }
    }
}

impl ScheduleState {
    /// `<local_data_dir>/routine-schedule.json`.
    pub fn path(local_data_dir: &Path) -> PathBuf {
        local_data_dir.join(SCHEDULE_FILE)
    }

    /// The state in `path`: empty when the file is missing, and when it is invalid (then set aside,
    /// so every routine is armed again). An error when it cannot be read.
    pub fn load(path: &Path) -> Result<Self> {
        let state = load_json::<Self>(path, MAX_SCHEDULE_BYTES, |state| state.version == 1)?;

        Ok(state.unwrap_or_default())
    }

    /// Writes the state to `path` (`0600`, one rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        write_json(path, self, "the routine schedule")
    }

    /// The entry of routine `id`.
    pub fn get(&self, id: &str) -> Option<&ScheduleEntry> {
        self.routines.get(id)
    }

    /// The entry of routine `id`, to change.
    pub(crate) fn get_mut(&mut self, id: &str) -> Option<&mut ScheduleEntry> {
        self.routines.get_mut(id)
    }

    /// Arms routine `id` for the bytes `sha256` at `now` (to the second), with no slot handled.
    pub fn arm(&mut self, id: &str, sha256: &str, now: Timestamp) {
        let armed_at = Timestamp::from_second(now.as_second()).unwrap_or(now);

        let entry = ScheduleEntry {
            sha256: sha256.to_owned(),
            armed_at,
            last_slot: None,
        };

        self.routines.insert(id.to_owned(), entry);
    }

    /// Forgets routine `id` (deleted, or no longer runnable: it is armed again when it is).
    pub fn forget(&mut self, id: &str) -> bool {
        self.routines.remove(id).is_some()
    }

    /// Forgets every routine `keep` refuses.
    pub(crate) fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.routines.retain(|id, _| keep(id));
    }
}
