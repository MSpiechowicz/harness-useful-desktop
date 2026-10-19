//! `<app_config_dir>/routines-state.json`: `{ "version": 1, "teamEnabled": { "<id>": "<project
//! root>" | null }, "sharedFrom": { "<id>": "<sha256>" } }`, written by the app window only.
//!
//! - `teamEnabled`: the team routines turned on on this machine, with the folder a team project
//!   routine runs in (`null` for a global one). Team routines arrive off: an id that is not here
//!   never runs.
//! - `sharedFrom`: for each routine the user shared, the SHA-256 of the local file at the time, so
//!   the team copy can say the local original changed since ("Update team copy").
//!
//! A writer ([`load_state`]) never saves over a file it could not read. An invalid file is set
//! aside (`.corrupt-<unix-seconds>`) and rebuilt empty, so every team routine is off again. A
//! reader ([`read_state`]) never writes: it sees an invalid or unreadable file as empty. The file
//! handling is [`crate::signed_docs::state`]'s.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::RoutineKind;
use crate::error::Result;
use crate::signed_docs::state::{self, DocState, TeamState};

/// See the module docs.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutinesState {
    version: u32,
    #[serde(default)]
    team_enabled: BTreeMap<String, Option<String>>,
    #[serde(default)]
    shared_from: BTreeMap<String, String>,
}

impl DocState for RoutinesState {
    fn empty() -> Self {
        Self {
            version: 1,
            ..Self::default()
        }
    }

    fn version(&self) -> u32 {
        self.version
    }
}

impl TeamState for RoutinesState {
    /// A team routine that left is off again.
    fn forget_own(&mut self, id: &str) {
        self.team_enabled.remove(id);
    }

    fn shared(&self) -> &BTreeMap<String, String> {
        &self.shared_from
    }

    fn shared_mut(&mut self) -> &mut BTreeMap<String, String> {
        &mut self.shared_from
    }
}

impl RoutinesState {
    /// Whether team routine `id` is turned on here: `None` when it is off, `Some(None)` for a
    /// global one, `Some(Some(root))` for a project one running in `root`.
    pub fn team_enabled(&self, id: &str) -> Option<Option<&str>> {
        self.team_enabled.get(id).map(Option::as_deref)
    }

    /// Turns team routine `id` on (`Some(folder)`: `None` for a global one, the chosen project root
    /// for a project one) or off (`None`).
    pub fn set_team_enabled(&mut self, id: &str, on: Option<Option<String>>) {
        match on {
            Some(folder) => self.team_enabled.insert(id.to_owned(), folder),
            None => self.team_enabled.remove(id),
        };
    }

    /// The ids turned on here.
    pub fn enabled_ids(&self) -> impl Iterator<Item = &str> {
        self.team_enabled.keys().map(String::as_str)
    }

    /// The project roots team routines run in here (for the team refresh, which must list them).
    pub fn team_folders(&self) -> impl Iterator<Item = &str> {
        self.team_enabled.values().filter_map(Option::as_deref)
    }
}

/// The state, read only: empty when the file is missing, invalid or cannot be read.
pub fn read_state(config_dir: &Path) -> RoutinesState {
    state::read_state::<RoutineKind, RoutinesState>(config_dir)
}

/// The state, for a writer that saves it again (see the module docs).
pub fn load_state(config_dir: &Path) -> Result<RoutinesState> {
    state::load_kind_state::<RoutineKind, RoutinesState>(config_dir)
}

/// Writes the state to `config_dir` (created when missing; `0600`, one rename, never through a
/// symbolic link).
pub fn save_state(config_dir: &Path, state: &RoutinesState) -> Result<()> {
    state::save_state::<RoutineKind, RoutinesState>(config_dir, state)
}
