//! `<app_config_dir>/hook-approvals.json`: `{ "version": 1, "approvals": { "<id>": "<sha256>" } }`.
//!
//! Written only by the app window (the MCP server reads it). A hook runs only while the SHA-256 of
//! its exact file bytes equals its entry, so any byte change (an agent rewriting the same values
//! included) makes it pending again.
//!
//! - Readers ([`Approvals::read`]): a missing or invalid file means nothing is approved, a file
//!   that cannot be read is an error (listings show every hook pending then).
//! - Writers ([`Approvals::load_for_update`]) never save over a file they could not read: that is
//!   an error. An invalid file (not JSON, oversized, another version) is renamed
//!   `hook-approvals.json.corrupt-<unix-seconds>` and replaced by an empty one, so every hook is
//!   pending until the user approves it again.
//!
//! Beside it, `<app_config_dir>/hook-approved/<id>.json` keeps a copy of the approved bytes, so the
//! Hooks view can show what changed in a pending edit ([`approved_snapshot`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::APPROVALS_FILE;
use super::format::{MAX_HOOK_FILE_BYTES, is_hook_id, parse_hook_file};
use super::sha256_hex;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically, unix_nanos};
use crate::model::Hook;
use crate::security::SafeReader;

/// The folder of approved copies in the app config folder.
pub const SNAPSHOT_FOLDER: &str = "hook-approved";

/// Largest approvals file read (64 hooks per scope, many projects).
const MAX_APPROVALS_BYTES: u64 = 4 * 1024 * 1024;

/// The approved hash of each hook.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Approvals {
    hashes: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct ApprovalsFile {
    version: u32,
    approvals: BTreeMap<String, String>,
}

/// The approvals file as read.
enum Stored {
    Missing,
    Invalid,
    Valid(Approvals),
}

impl Approvals {
    /// The approvals in `config_dir`, read only: empty when the file is missing or invalid, an
    /// error when it cannot be read. Entries with a malformed id or hash are left out.
    pub fn read(config_dir: &Path) -> Result<Self> {
        match read_stored(&config_dir.join(APPROVALS_FILE))? {
            Stored::Missing => Ok(Self::default()),
            Stored::Invalid => {
                tracing::warn!("the hook approvals file is invalid, every hook is pending");
                Ok(Self::default())
            }
            Stored::Valid(approvals) => Ok(approvals),
        }
    }

    /// The approvals in `config_dir`, for a writer that saves them again: an error when the file
    /// cannot be read (it must not be saved over), and an invalid file is set aside
    /// (`.corrupt-<unix-seconds>`) and counts as empty.
    pub fn load_for_update(config_dir: &Path) -> Result<Self> {
        let path = config_dir.join(APPROVALS_FILE);

        match read_stored(&path)? {
            Stored::Missing => Ok(Self::default()),
            Stored::Invalid => {
                tracing::warn!("the hook approvals file is invalid; set aside, every hook is pending");
                set_aside(&path)?;
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

        let path: PathBuf = config_dir.join(APPROVALS_FILE);
        refuse_link(&path)?;

        let file = ApprovalsFile {
            version: 1,
            approvals: self.hashes.clone(),
        };
        let mut bytes = serde_json::to_vec_pretty(&file)
            .map_err(|error| BusinessError::Internal(format!("encoding hook approvals: {error}")))?;
        bytes.push(b'\n');

        replace_atomically(&path, &bytes, "saving hook approvals")
    }
}

/// The approvals file at `path`. A regular file that is too large, not UTF-8 or not a version 1
/// approvals object is invalid; any other read failure is an error.
fn read_stored(path: &Path) -> Result<Stored> {
    let text = match SafeReader::read_to_string(path, MAX_APPROVALS_BYTES) {
        Ok(text) => text,
        Err(BusinessError::NotFound(_)) => return Ok(Stored::Missing),
        Err(BusinessError::InvalidInput(_)) if is_regular_file(path) => return Ok(Stored::Invalid),
        Err(error) => return Err(error),
    };

    let file = match serde_json::from_str::<ApprovalsFile>(&text) {
        Ok(file) if file.version == 1 => file,
        _ => return Ok(Stored::Invalid),
    };

    let hashes = file
        .approvals
        .into_iter()
        .filter(|(id, hash)| is_hook_id(id) && is_sha256(hash))
        .collect();

    Ok(Stored::Valid(Approvals { hashes }))
}

/// True when `path` is a regular file (not a link, folder or device).
pub(crate) fn is_regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
}

/// Renames the invalid file at `path` to `<name>.corrupt-<unix-seconds>`, kept for inspection.
pub(crate) fn set_aside(path: &Path) -> Result<()> {
    let mut aside = path.as_os_str().to_owned();
    aside.push(format!(".corrupt-{}", unix_nanos() / 1_000_000_000));

    match std::fs::rename(path, &aside) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(BusinessError::from_io("setting an invalid hooks file aside", error)),
    }
}

/// 64 lowercase hex characters.
pub(crate) fn is_sha256(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Keeps `bytes` as the approved copy of hook `id` (`0600`, replaced in one rename, the folder
/// `0700` on Unix). A failure is logged: only the diff in the Hooks view needs the copy.
pub(crate) fn save_snapshot(config_dir: &Path, id: &str, bytes: &[u8]) {
    let dir = config_dir.join(SNAPSHOT_FOLDER);
    let path = dir.join(format!("{id}.json"));

    let saved = create_private_dir(&dir, "creating the approved hooks folder")
        .and_then(|()| refuse_link(&path))
        .and_then(|()| replace_atomically(&path, bytes, "saving an approved hook"));

    if let Err(error) = saved {
        tracing::warn!(%error, "the approved copy of a hook was not kept");
    }
}

/// The ids of the approved copies kept in `config_dir`.
pub(crate) fn snapshot_ids(config_dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(config_dir.join(SNAPSHOT_FOLDER)) else {
        return Vec::new();
    };

    entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter_map(|name| {
            name.strip_suffix(".json")
                .filter(|id| is_hook_id(id))
                .map(str::to_owned)
        })
        .collect()
}

/// Drops the approved copy of hook `id`.
pub(crate) fn remove_snapshot(config_dir: &Path, id: &str) {
    let path = config_dir.join(SNAPSHOT_FOLDER).join(format!("{id}.json"));

    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(%error, "the approved copy of a hook was not removed"),
    }
}

/// The approved version of hook `id`: the kept copy in `config_dir`, when it still hashes to
/// `approved_sha256` and parses.
pub fn approved_snapshot(config_dir: &Path, id: &str, approved_sha256: &str) -> Option<Hook> {
    let bytes = approved_bytes(config_dir, id, approved_sha256)?;
    parse_hook_file(id, &bytes).ok()
}

/// The bytes of the kept copy of hook `id`, when they hash to `approved_sha256`.
pub(crate) fn approved_bytes(config_dir: &Path, id: &str, approved_sha256: &str) -> Option<Vec<u8>> {
    if !is_hook_id(id) {
        return None;
    }

    let path = config_dir.join(SNAPSHOT_FOLDER).join(format!("{id}.json"));
    let text = SafeReader::read_to_string(&path, MAX_HOOK_FILE_BYTES as u64).ok()?;

    (sha256_hex(text.as_bytes()) == approved_sha256).then(|| text.into_bytes())
}
