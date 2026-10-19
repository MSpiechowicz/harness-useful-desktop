//! A kind's state file in the app config folder ([`DocKind::STATE_FILE`], e.g.
//! `hooks-state.json`): a version 1 JSON object only the app window writes. What it holds is the
//! kind's ([`DocState`]); this module reads, sets aside and saves it.
//!
//! A writer ([`load_state`]) never saves over a file it could not read. An invalid file is set
//! aside (`.corrupt-<unix-seconds>`) and rebuilt by the kind (saved at once). A reader
//! ([`read_state`]) never writes: it sees an invalid or unreadable file as empty.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::approvals::{is_regular_file, set_aside};
use super::{DocKind, pretty_json};
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{refuse_link, replace_atomically};
use crate::security::SafeReader;

/// Largest state file read.
const MAX_STATE_BYTES: u64 = 256 * 1024;

/// What a kind keeps in its state file.
pub trait DocState: Serialize + DeserializeOwned {
    /// The state of a new file: version 1, nothing in it.
    fn empty() -> Self;

    /// The file's `version` (only 1 is read).
    fn version(&self) -> u32;

    /// The state that replaces an invalid file (saved at once): [`DocState::empty`] unless the
    /// kind keeps more.
    fn rebuilt() -> Self {
        Self::empty()
    }
}

/// The team entries a kind's state keeps: `sharedFrom`, the local version each shared document
/// had, and the kind's own entries per team document.
pub trait TeamState: DocState {
    /// `sharedFrom`.
    fn shared(&self) -> &BTreeMap<String, String>;

    /// `sharedFrom`, to change.
    fn shared_mut(&mut self) -> &mut BTreeMap<String, String>;

    /// Forgets the kind's own entries of team document `id` (it left the team).
    fn forget_own(&mut self, id: &str);

    /// The SHA-256 the local document `id` had when the user shared it.
    fn shared_from(&self, id: &str) -> Option<&str> {
        self.shared().get(id).map(String::as_str)
    }

    /// Records that the local document `id` was shared at version `sha256`.
    fn set_shared_from(&mut self, id: &str, sha256: &str) {
        self.shared_mut().insert(id.to_owned(), sha256.to_owned());
    }

    /// Forgets every team entry of document `id` (it left the team).
    fn forget_team(&mut self, id: &str) {
        self.shared_mut().remove(id);
        self.forget_own(id);
    }
}

/// The state for a writer, rebuilt as the kind says ([`DocState::rebuilt`]) when invalid.
pub fn load_kind_state<K: DocKind, S: DocState>(config_dir: &Path) -> Result<S> {
    load_state::<K, S>(config_dir, S::rebuilt)
}

/// The state, read only: empty when the file is missing, invalid or cannot be read.
pub fn read_state<K: DocKind, S: DocState>(config_dir: &Path) -> S {
    let text = match SafeReader::read_to_string(&config_dir.join(K::STATE_FILE), MAX_STATE_BYTES) {
        Ok(text) => text,
        Err(BusinessError::NotFound(_)) => return S::empty(),
        Err(_) => {
            tracing::warn!("the {} state file cannot be read", K::PLURAL);
            return S::empty();
        }
    };

    serde_json::from_str::<S>(&text)
        .ok()
        .filter(|state| state.version() == 1)
        .unwrap_or_else(S::empty)
}

/// The state, for a writer that saves it again (see the module docs). An invalid file is set
/// aside and replaced by `rebuilt()`, saved at once.
pub fn load_state<K: DocKind, S: DocState>(config_dir: &Path, rebuilt: impl FnOnce() -> S) -> Result<S> {
    let path = config_dir.join(K::STATE_FILE);

    let text = match SafeReader::read_to_string(&path, MAX_STATE_BYTES) {
        Ok(text) => Some(text),
        Err(BusinessError::NotFound(_)) => return Ok(S::empty()),
        Err(BusinessError::InvalidInput(_)) if is_regular_file(&path) => None,
        Err(error) => return Err(error),
    };

    let state = text
        .and_then(|text| serde_json::from_str::<S>(&text).ok())
        .filter(|state| state.version() == 1);

    if let Some(state) = state {
        return Ok(state);
    }

    tracing::warn!("the {} state file is invalid; set aside and rebuilt", K::PLURAL);
    set_aside::<K>(&path)?;

    let rebuilt = rebuilt();

    save_state::<K, S>(config_dir, &rebuilt)?;
    Ok(rebuilt)
}

/// Writes the state to `config_dir` (created when missing; `0600`, one rename, never through a
/// symbolic link).
pub fn save_state<K: DocKind, S: Serialize>(config_dir: &Path, state: &S) -> Result<()> {
    std::fs::create_dir_all(config_dir)
        .map_err(|error| BusinessError::from_io("creating the app config folder", error))?;

    let path = config_dir.join(K::STATE_FILE);
    refuse_link(&path)?;

    let bytes = pretty_json(state, &format!("the {} state", K::PLURAL))?;

    replace_atomically(&path, &bytes, &format!("saving the {} state", K::PLURAL))
}
