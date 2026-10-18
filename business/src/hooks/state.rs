//! `<app_config_dir>/hooks-state.json`: `{ "version": 1, "seeded": [...], "teamDisabled": [...],
//! "sharedFrom": {...} }`, written by the app window only.
//!
//! - `seeded`: the Harness family hooks launch seeding created (or found) once (see
//!   [`super::family`]).
//! - `teamDisabled`: the team hooks the user turned off on this machine (never written to the
//!   team).
//! - `sharedFrom`: for each hook the user shared, the SHA-256 of the local file at the time, so
//!   the team copy can say the local original changed since ("Update team copy").
//!
//! A writer ([`load_state`]) never saves over a file it could not read. An invalid file is set
//! aside (`.corrupt-<unix-seconds>`) and rebuilt with every catalog seed listed (saved at once), so
//! a hook the user deleted is not created again at launch, and with no team entries. A reader
//! ([`read_state`]) never writes: it sees an invalid or unreadable file as empty.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::STATE_FILE;
use super::approvals::{is_regular_file, set_aside};
use super::family::FAMILY;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{refuse_link, replace_atomically};
use crate::model::HookSeed;
use crate::security::SafeReader;

/// Largest state file read.
const MAX_STATE_BYTES: u64 = 256 * 1024;

/// See the module docs.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HooksState {
    version: u32,
    seeded: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    team_disabled: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    shared_from: BTreeMap<String, String>,
}

impl HooksState {
    fn empty() -> Self {
        Self {
            version: 1,
            ..Self::default()
        }
    }

    /// True when launch seeding created (or found) `seed` once.
    pub fn has(&self, seed: HookSeed) -> bool {
        self.seeded.iter().any(|name| name == seed.as_str())
    }

    /// Records that `seed` was seeded.
    pub fn record(&mut self, seed: HookSeed) {
        if !self.has(seed) {
            self.seeded.push(seed.as_str().to_owned());
        }
    }

    /// True when the user turned team hook `id` off on this machine.
    pub fn team_disabled(&self, id: &str) -> bool {
        self.team_disabled.contains(id)
    }

    /// Turns team hook `id` on or off on this machine.
    pub fn set_team_disabled(&mut self, id: &str, disabled: bool) {
        if disabled {
            self.team_disabled.insert(id.to_owned());
        } else {
            self.team_disabled.remove(id);
        }
    }

    /// The SHA-256 the local hook `id` had when the user shared it.
    pub fn shared_from(&self, id: &str) -> Option<&str> {
        self.shared_from.get(id).map(String::as_str)
    }

    /// Records that local hook `id` was shared at version `sha256`.
    pub fn set_shared_from(&mut self, id: &str, sha256: &str) {
        self.shared_from.insert(id.to_owned(), sha256.to_owned());
    }

    /// Forgets the team entries of hook `id` (it left the team).
    pub fn forget_team(&mut self, id: &str) {
        self.shared_from.remove(id);
        self.team_disabled.remove(id);
    }
}

/// The state, read only: empty when the file is missing, invalid or cannot be read.
pub(crate) fn read_state(config_dir: &Path) -> HooksState {
    let text = match SafeReader::read_to_string(&config_dir.join(STATE_FILE), MAX_STATE_BYTES) {
        Ok(text) => text,
        Err(BusinessError::NotFound(_)) => return HooksState::empty(),
        Err(_) => {
            tracing::warn!("the hooks state file cannot be read");
            return HooksState::empty();
        }
    };

    serde_json::from_str::<HooksState>(&text)
        .ok()
        .filter(|state| state.version == 1)
        .unwrap_or_else(HooksState::empty)
}

/// The state, for a writer that saves it again (see the module docs).
pub(crate) fn load_state(config_dir: &Path) -> Result<HooksState> {
    let path = config_dir.join(STATE_FILE);

    let text = match SafeReader::read_to_string(&path, MAX_STATE_BYTES) {
        Ok(text) => Some(text),
        Err(BusinessError::NotFound(_)) => return Ok(HooksState::empty()),
        Err(BusinessError::InvalidInput(_)) if is_regular_file(&path) => None,
        Err(error) => return Err(error),
    };

    let state = text
        .and_then(|text| serde_json::from_str::<HooksState>(&text).ok())
        .filter(|state| state.version == 1);

    if let Some(state) = state {
        return Ok(state);
    }

    tracing::warn!("the hooks state file is invalid; set aside and rebuilt");
    set_aside(&path)?;

    let mut rebuilt = HooksState::empty();
    for entry in &FAMILY {
        rebuilt.record(entry.seed);
    }

    // Saved at once: a missing record would let the next launch seed deleted hooks again.
    save_state(config_dir, &rebuilt)?;
    Ok(rebuilt)
}

/// Writes the state to `config_dir` (created when missing; `0600`, one rename, never through a
/// symbolic link).
pub(crate) fn save_state(config_dir: &Path, state: &HooksState) -> Result<()> {
    std::fs::create_dir_all(config_dir)
        .map_err(|error| BusinessError::from_io("creating the app config folder", error))?;

    let path = config_dir.join(STATE_FILE);
    refuse_link(&path)?;

    let mut bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| BusinessError::Internal(format!("encoding the hooks state: {error}")))?;
    bytes.push(b'\n');

    replace_atomically(&path, &bytes, "saving the hooks state")
}
