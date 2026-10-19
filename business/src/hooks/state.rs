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
//! ([`read_state`]) never writes: it sees an invalid or unreadable file as empty. The file handling
//! is [`crate::signed_docs::state`]'s.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::HookKind;
use super::family::FAMILY;
use crate::error::Result;
use crate::model::HookSeed;
use crate::signed_docs::state::{self, DocState, TeamState};

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

impl DocState for HooksState {
    fn empty() -> Self {
        Self {
            version: 1,
            ..Self::default()
        }
    }

    fn version(&self) -> u32 {
        self.version
    }

    /// Every catalog seed listed, so a hook the user deleted is not created again at launch.
    fn rebuilt() -> Self {
        let mut rebuilt = Self::empty();
        for entry in &FAMILY {
            rebuilt.record(entry.seed);
        }

        rebuilt
    }
}

impl TeamState for HooksState {
    fn shared(&self) -> &BTreeMap<String, String> {
        &self.shared_from
    }

    fn shared_mut(&mut self) -> &mut BTreeMap<String, String> {
        &mut self.shared_from
    }

    fn forget_own(&mut self, id: &str) {
        self.team_disabled.remove(id);
    }
}

impl HooksState {
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
}

/// The state, read only: empty when the file is missing, invalid or cannot be read.
pub(crate) fn read_state(config_dir: &Path) -> HooksState {
    state::read_state::<HookKind, HooksState>(config_dir)
}

/// The state, for a writer that saves it again (see the module docs).
pub(crate) fn load_state(config_dir: &Path) -> Result<HooksState> {
    // Rebuilt with every seed and saved at once: a missing record would let the next launch seed
    // deleted hooks again.
    state::load_kind_state::<HookKind, HooksState>(config_dir)
}

/// Writes the state to `config_dir` (created when missing; `0600`, one rename, never through a
/// symbolic link).
pub(crate) fn save_state(config_dir: &Path, state: &HooksState) -> Result<()> {
    state::save_state::<HookKind, HooksState>(config_dir, state)
}
