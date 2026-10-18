//! `<hooks root>/team/`: one envelope per team hook and `projects.json`.
//!
//! - `<id>.json` holds `{"version": 1, "id", "repo", "sha256", "author", "createdAt",
//!   "updatedAt", "body"}`, the row as the server answered it. Only names that are exactly
//!   `<id>.json` are read, only regular files (no symbolic links), and nothing while group or
//!   others can write to the folder.
//! - `projects.json` holds `{"<project root>": {"key": "git:…", "shareable": true}}`: the
//!   identity the server gave each project root a list was asked for, so a chat in that root gets
//!   the team hooks of its repository without asking the server.
//! - The folder is `0700` and every file `0600`, each replaced in one rename. The app window is
//!   the only writer, under its hooks write lock.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::sync::TeamFetch;
use super::{PROJECTS_FILE, ProjectKey, TEAM_FOLDER, TeamRow};
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically};
use crate::hooks::approvals::{is_sha256, json_ids};
use crate::hooks::format::{MAX_HOOK_FILE_BYTES, is_hook_id};
use crate::hooks::store::HookStore;
use crate::security::SafeReader;

/// Largest envelope read: a 16 KiB body, JSON-escaped, and its fields.
const MAX_ENVELOPE_BYTES: u64 = 8 * MAX_HOOK_FILE_BYTES as u64;

/// Largest `projects.json` read.
const MAX_PROJECTS_BYTES: u64 = 256 * 1024;

/// The only envelope version.
const ENVELOPE_VERSION: u32 = 1;

/// The team folder of one hooks root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamFolder {
    dir: PathBuf,
}

/// One `<id>.json` envelope as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamFile {
    /// The file name stem.
    pub id: String,
    /// The row it holds, or why it cannot be used.
    pub row: std::result::Result<TeamRow, String>,
}

/// What a refresh changed in the folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncCounts {
    /// Envelopes written (new or changed).
    pub written: usize,
    /// The ids of the envelopes removed (no longer listed).
    pub removed: Vec<String>,
}

impl SyncCounts {
    /// True when the folder changed.
    pub fn changed(&self) -> bool {
        self.written > 0 || !self.removed.is_empty()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    version: u32,
    #[serde(flatten)]
    row: TeamRow,
}

impl TeamFolder {
    /// The team folder of `store`'s hooks root (`<root>/team`).
    pub fn of(store: &HookStore) -> Self {
        Self::new(store.root().join(TEAM_FOLDER))
    }

    /// The team folder at `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// True when the folder may be read: a real folder (not a link) other users cannot change.
    fn readable(&self) -> bool {
        let Ok(metadata) = std::fs::symlink_metadata(&self.dir) else {
            return false;
        };

        metadata.is_dir() && !others_may_write(&metadata)
    }

    /// The id of every `<id>.json` entry, whatever its type (for keeping approvals).
    pub fn ids(&self) -> BTreeSet<String> {
        json_ids(&self.dir).into_iter().collect()
    }

    /// Every envelope, by id. Nothing while the folder is missing or not safe to read.
    pub fn read(&self) -> Vec<TeamFile> {
        if !self.readable() {
            return Vec::new();
        }

        self.ids().into_iter().filter_map(|id| self.read_file(&id)).collect()
    }

    /// The envelope of team hook `id`, when there is a regular file for it.
    pub fn read_one(&self, id: &str) -> Option<TeamFile> {
        if !is_hook_id(id) || !self.readable() {
            return None;
        }

        self.read_file(id)
    }

    fn read_file(&self, id: &str) -> Option<TeamFile> {
        let path = self.file_path(id);

        if !std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
            return None;
        }

        let row = SafeReader::read_to_string(&path, MAX_ENVELOPE_BYTES)
            .map_err(|_| "the team copy could not be read".to_owned())
            .and_then(|text| parse_envelope(id, &text));

        Some(TeamFile { id: id.to_owned(), row })
    }

    /// The row of team hook `id` (`NotFound` when there is no usable copy).
    pub fn row(&self, id: &str) -> Result<TeamRow> {
        self.read_one(id)
            .and_then(|file| file.row.ok())
            .ok_or_else(|| BusinessError::NotFound("the team hook".to_owned()))
    }

    /// Writes the envelope of `row` (creating the folder); a row whose id or hash is malformed is
    /// refused. True when the file changed.
    pub fn write_row(&self, row: &TeamRow) -> Result<bool> {
        if !is_hook_id(&row.id) || !is_sha256(&row.sha256) {
            return Err(BusinessError::Protocol(
                "harness-memory answered a team hook without a valid id or hash".to_owned(),
            ));
        }

        self.ensure_dir()?;

        let path = self.file_path(&row.id);
        refuse_link(&path)?;

        if self.read_file(&row.id).is_some_and(|file| file.row.as_ref() == Ok(row)) {
            return Ok(false);
        }

        let envelope = Envelope {
            version: ENVELOPE_VERSION,
            row: row.clone(),
        };

        replace_atomically(&path, &json_bytes(&envelope)?, "saving a team hook")?;
        Ok(true)
    }

    /// Removes the envelope of team hook `id`; a missing one is fine. True when there was one.
    pub fn remove(&self, id: &str) -> Result<bool> {
        if !is_hook_id(id) {
            return Ok(false);
        }

        match std::fs::remove_file(self.file_path(id)) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(BusinessError::from_io("removing a team hook", error)),
        }
    }

    /// The repository key of each project root, from `projects.json` (empty when it is missing or
    /// invalid).
    pub fn projects(&self) -> BTreeMap<String, ProjectKey> {
        if !self.readable() {
            return BTreeMap::new();
        }

        SafeReader::read_to_string(&self.dir.join(PROJECTS_FILE), MAX_PROJECTS_BYTES)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Replaces `projects.json` with `projects` (creating the folder).
    pub fn save_projects(&self, projects: &BTreeMap<String, ProjectKey>) -> Result<()> {
        self.ensure_dir()?;

        let path = self.dir.join(PROJECTS_FILE);
        refuse_link(&path)?;

        if &self.projects() == projects {
            return Ok(());
        }

        replace_atomically(&path, &json_bytes(projects)?, "saving the team projects")
    }

    /// Writes what one refresh round listed (see [`TeamFetch`]), scope by scope:
    ///
    /// - `projects.json` holds the roots whose query answered, plus the skipped and failed roots
    ///   it already knew (their folders or the server may come back). Refused roots and roots the
    ///   round was not given are dropped.
    /// - A global envelope it did not list (or one that cannot be read) is removed when the global
    ///   query read every page. An envelope of repository key `R` it did not list is removed when
    ///   a query of a root with key `R` read every page, or when no root in `projects.json` has
    ///   `R` any more. So an unmounted drive or a failing query keeps its repository's copies.
    ///
    /// Use [`apply_round`](super::apply_round) to drop the approvals of what it removes too.
    pub fn apply(&self, fetch: &TeamFetch) -> Result<SyncCounts> {
        self.ensure_dir()?;

        let mut counts = SyncCounts::default();

        for row in fetch.rows.values() {
            if self.write_row(row)? {
                counts.written += 1;
            }
        }

        let saved = self.projects();
        let mut projects: BTreeMap<String, ProjectKey> = fetch
            .skipped
            .iter()
            .chain(&fetch.failed)
            .filter_map(|root| saved.get(root).map(|key| (root.clone(), key.clone())))
            .collect();
        projects.extend(fetch.projects.clone());

        let known: BTreeSet<&str> = projects.values().map(|key| key.key.as_str()).collect();

        for file in self.read() {
            if fetch.rows.contains_key(&file.id) {
                continue;
            }

            let gone = match file.row.as_ref().map(|row| row.repo.as_deref()) {
                Err(_) | Ok(None) => fetch.global_listed,
                Ok(Some(repo)) => fetch.listed_keys.contains(repo) || !known.contains(repo),
            };

            if gone && self.remove(&file.id)? {
                counts.removed.push(file.id);
            }
        }

        self.save_projects(&projects)?;

        Ok(counts)
    }

    /// Deletes the folder with every copy (sharing is off, or the server has no team hooks).
    /// True when there was one.
    pub fn clear(&self) -> Result<bool> {
        match std::fs::symlink_metadata(&self.dir) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(BusinessError::from_io("reading the team hooks folder", error)),
            // A link is removed, never followed.
            Ok(metadata) if !metadata.is_dir() => std::fs::remove_file(&self.dir)
                .map(|()| true)
                .map_err(|error| BusinessError::from_io("removing the team hooks folder", error)),
            Ok(_) => std::fs::remove_dir_all(&self.dir)
                .map(|()| true)
                .map_err(|error| BusinessError::from_io("removing the team hooks folder", error)),
        }
    }

    /// Creates the folder (`0700` on Unix) when missing; `Forbidden` when it is a link or other
    /// users can change it.
    fn ensure_dir(&self) -> Result<()> {
        refuse_link(&self.dir)?;
        create_private_dir(&self.dir, "creating the team hooks folder")?;

        if !self.readable() {
            return Err(BusinessError::Forbidden(
                "other users can change the team hooks folder".to_owned(),
            ));
        }

        Ok(())
    }
}

/// The row in the envelope `text` of file `id`, or why it cannot be used.
fn parse_envelope(id: &str, text: &str) -> std::result::Result<TeamRow, String> {
    let envelope: Envelope = serde_json::from_str(text).map_err(|_| "the team copy is not readable".to_owned())?;

    if envelope.version != ENVELOPE_VERSION {
        return Err("the team copy has another version".to_owned());
    }

    if envelope.row.id != id {
        return Err("the team copy's id does not match its file name".to_owned());
    }

    Ok(envelope.row)
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| BusinessError::Internal(format!("encoding team hooks: {error}")))?;
    bytes.push(b'\n');

    Ok(bytes)
}

#[cfg(unix)]
fn others_may_write(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o022 != 0
}

#[cfg(not(unix))]
fn others_may_write(_metadata: &std::fs::Metadata) -> bool {
    false
}
