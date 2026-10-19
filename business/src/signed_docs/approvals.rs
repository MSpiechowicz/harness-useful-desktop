//! `<app_config_dir>/<kind>-approvals.json` ([`DocKind::APPROVALS_FILE`]):
//! `{ "version": 1, "approvals": { "<id>": "<sha256>" } }`. A kind that keeps the approvals of
//! team copies apart ([`DocKind::SEPARATE_TEAM_APPROVALS`]) keys them `team:<id>` in the same map.
//!
//! Written only by the app window (the MCP server reads it). A document is used only while the
//! SHA-256 of its exact file bytes equals its entry, so any byte change (an agent rewriting the
//! same values included) makes it pending again.
//!
//! - Readers ([`Approvals::read`]): a missing or invalid file means nothing is approved, a file
//!   that cannot be read is an error (listings show every document pending then).
//! - Writers ([`Approvals::load_for_update`]) never save over a file they could not read: that is
//!   an error. An invalid file (not JSON, oversized, another version) is renamed
//!   `<name>.corrupt-<unix-seconds>` and replaced by an empty one, so every document is pending
//!   until the user approves it again.

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::json_file::{Stored, read_stored};
use super::{DocKind, is_approval_key, is_sha256, pretty_json};
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{refuse_link, replace_atomically, unix_nanos};
use crate::model::HookApproval;

/// Largest approvals file read (64 documents per scope, many projects).
const MAX_APPROVALS_BYTES: u64 = 4 * 1024 * 1024;

/// The approved hash of each document of kind `K`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Approvals<K: DocKind> {
    hashes: BTreeMap<String, String>,
    kind: PhantomData<K>,
}

#[derive(Serialize, Deserialize)]
struct ApprovalsFile {
    version: u32,
    approvals: BTreeMap<String, String>,
}

impl<K: DocKind> Approvals<K> {
    /// The approvals in `config_dir`, read only: empty when the file is missing or invalid, an
    /// error when it cannot be read. Entries with a malformed id or hash are left out.
    pub fn read(config_dir: &Path) -> Result<Self> {
        match read_approvals::<K>(&config_dir.join(K::APPROVALS_FILE))? {
            Stored::Missing => Ok(Self::default()),
            Stored::Invalid => {
                tracing::warn!(
                    "the {} approvals file is invalid, every {} is pending",
                    K::NOUN,
                    K::NOUN
                );
                Ok(Self::default())
            }
            Stored::Valid(approvals) => Ok(approvals),
        }
    }

    /// The approvals in `config_dir`, for a writer that saves them again: an error when the file
    /// cannot be read (it must not be saved over), and an invalid file is set aside
    /// (`.corrupt-<unix-seconds>`) and counts as empty.
    pub fn load_for_update(config_dir: &Path) -> Result<Self> {
        let path = config_dir.join(K::APPROVALS_FILE);

        match read_approvals::<K>(&path)? {
            Stored::Missing => Ok(Self::default()),
            Stored::Invalid => {
                tracing::warn!(
                    "the {} approvals file is invalid; set aside, every {} is pending",
                    K::NOUN,
                    K::NOUN
                );
                set_aside::<K>(&path)?;
                Ok(Self::default())
            }
            Stored::Valid(approvals) => Ok(approvals),
        }
    }

    /// The approved hash of `id`.
    pub fn get(&self, id: &str) -> Option<&str> {
        self.hashes.get(id).map(String::as_str)
    }

    /// The ids that have an approval.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.hashes.keys().map(String::as_str)
    }

    /// True when `sha256` is the approved hash of `id`.
    pub fn is_approved(&self, id: &str, sha256: &str) -> bool {
        self.get(id) == Some(sha256)
    }

    /// How the approvals relate to the file `id` whose bytes hash to `sha256`.
    pub fn approval(&self, id: &str, sha256: &str) -> HookApproval {
        match self.get(id) {
            Some(approved) if approved == sha256 => HookApproval::Approved,
            Some(_) => HookApproval::Changed,
            None => HookApproval::Never,
        }
    }

    /// Approves `sha256` for `id` (in memory).
    pub fn insert(&mut self, id: &str, sha256: &str) {
        self.hashes.insert(id.to_owned(), sha256.to_owned());
    }

    /// Drops the approval of `id` (in memory); true when there was one.
    pub fn remove(&mut self, id: &str) -> bool {
        self.hashes.remove(id).is_some()
    }

    /// Writes the approvals to `config_dir` (created when missing; the file `0600`, replaced in
    /// one rename, never through a symbolic link).
    pub fn save(&self, config_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(config_dir)
            .map_err(|error| BusinessError::from_io("creating the app config folder", error))?;

        let path = config_dir.join(K::APPROVALS_FILE);
        refuse_link(&path)?;

        let file = ApprovalsFile {
            version: 1,
            approvals: self.hashes.clone(),
        };
        let bytes = pretty_json(&file, &format!("{} approvals", K::NOUN))?;

        replace_atomically(&path, &bytes, &format!("saving {} approvals", K::NOUN))
    }
}

/// The approvals file at `path`. A regular file that is too large, not UTF-8 or not a version 1
/// approvals object is invalid; any other read failure is an error. Entries with a malformed key or
/// hash are left out.
fn read_approvals<K: DocKind>(path: &Path) -> Result<Stored<Approvals<K>>> {
    let file = match read_stored::<ApprovalsFile>(path, MAX_APPROVALS_BYTES, |file| file.version == 1)? {
        Stored::Missing => return Ok(Stored::Missing),
        Stored::Invalid => return Ok(Stored::Invalid),
        Stored::Valid(file) => file,
    };

    let hashes = file
        .approvals
        .into_iter()
        .filter(|(key, hash)| is_approval_key(key) && is_sha256(hash))
        .collect();

    Ok(Stored::Valid(Approvals {
        hashes,
        kind: PhantomData,
    }))
}

/// True when `path` is a regular file (not a link, folder or device).
pub(crate) fn is_regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
}

/// Renames the invalid file at `path` to `<name>.corrupt-<unix-seconds>`, kept for inspection.
pub(crate) fn set_aside<K: DocKind>(path: &Path) -> Result<()> {
    let mut aside = path.as_os_str().to_owned();
    aside.push(format!(".corrupt-{}", unix_nanos() / 1_000_000_000));

    match std::fs::rename(path, &aside) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(BusinessError::from_io(
            format!("setting an invalid {} file aside", K::PLURAL),
            error,
        )),
    }
}
