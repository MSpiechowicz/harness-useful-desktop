//! `<root>/team/`: one envelope per team document and `projects.json`.
//!
//! - `<id>.json` holds the row as the server answered it (see [`super::envelope`]). Only names
//!   that are exactly `<id>.json` are read, only regular files (no symbolic links) the user owns,
//!   and nothing while another user owns the folder or the root, or group or others can write to
//!   either.
//! - `projects.json` holds `{"<project root>": {"key": "git:…", "shareable": true}}`: the
//!   identity the server gave each project root a list was asked for, so a chat in that root gets
//!   the team documents of its repository without asking the server.
//! - The folder is `0700` and every file `0600`, each replaced in one rename. The app window is
//!   the only writer, under its write lock for the kind.

use std::collections::{BTreeMap, BTreeSet};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use super::envelope::{envelope_bytes, parse_envelope};
use super::sync::TeamFetch;
use super::{PROJECTS_FILE, ProjectKey, TEAM_FOLDER, TeamRow};
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, refuse_link, replace_atomically};
use crate::security::SafeReader;
use crate::signed_docs::snapshots::json_ids;
use crate::signed_docs::{
    DocKind, DocStore, is_doc_id, is_sha256, owned_by_current_user, pretty_json, sha256_hex, unsafe_folder, unsafe_root,
};

/// Largest `projects.json` read.
const MAX_PROJECTS_BYTES: u64 = 256 * 1024;

/// The team folder of one root of kind `K`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamFolder<K: DocKind> {
    dir: PathBuf,
    kind: PhantomData<K>,
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

impl<K: DocKind> TeamFolder<K> {
    /// The team folder of `store`'s root (`<root>/team`).
    pub fn of(store: &DocStore<K>) -> Self {
        Self::new(store.root().join(TEAM_FOLDER))
    }

    /// The team folder at `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            kind: PhantomData,
        }
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// True when the folder may be read: a real folder (not a link) the user owns and other users
    /// cannot change, in a root that is the same.
    fn readable(&self) -> bool {
        let Ok(metadata) = std::fs::symlink_metadata(&self.dir) else {
            return false;
        };

        let root_safe = self.dir.parent().is_some_and(|root| unsafe_root::<K>(root).is_none());

        metadata.is_dir() && unsafe_folder::<K>(&metadata).is_none() && root_safe
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

    /// The envelope of team document `id`, when there is a regular file for it.
    pub fn read_one(&self, id: &str) -> Option<TeamFile> {
        if !is_doc_id(id) || !self.readable() {
            return None;
        }

        self.read_file(id)
    }

    fn read_file(&self, id: &str) -> Option<TeamFile> {
        let path = self.file_path(id);

        let metadata = std::fs::symlink_metadata(&path).ok()?;

        if !metadata.is_file() {
            return None;
        }

        if !owned_by_current_user(&metadata) {
            let row = Err("the remote copy belongs to another user".to_owned());
            return Some(TeamFile { id: id.to_owned(), row });
        }

        // A body at the size limit, JSON-escaped, and the row's other fields.
        let max_envelope_bytes = 8 * K::MAX_FILE_BYTES as u64;

        let row = SafeReader::read_to_string(&path, max_envelope_bytes)
            .map_err(|_| "the remote copy could not be read".to_owned())
            .and_then(|text| parse_envelope(id, &text));

        Some(TeamFile { id: id.to_owned(), row })
    }

    /// The row of team document `id` (`NotFound` when there is no usable copy).
    pub fn row(&self, id: &str) -> Result<TeamRow> {
        self.read_one(id)
            .and_then(|file| file.row.ok())
            .ok_or_else(|| BusinessError::NotFound(format!("the remote {}", K::NOUN)))
    }

    /// The row of team document `id` while its body is version `sha256` (the one the user was
    /// shown). `Conflict` when the copy changed since, `NotFound` without one.
    pub fn row_at(&self, id: &str, sha256: &str) -> Result<TeamRow> {
        let row = self.row(id)?;

        if row.sha256 != sha256 || sha256_hex(row.body.as_bytes()) != sha256 {
            return Err(BusinessError::Conflict(format!(
                "the remote {} changed since it was shown",
                K::NOUN
            )));
        }

        Ok(row)
    }

    /// Writes the envelope of `row` (creating the folder); a row whose id or hash is malformed is
    /// refused. True when the file changed.
    pub fn write_row(&self, row: &TeamRow) -> Result<bool> {
        if !is_doc_id(&row.id) || !is_sha256(&row.sha256) {
            return Err(BusinessError::Protocol(format!(
                "harness-memory answered a remote {} without a valid id or hash",
                K::NOUN
            )));
        }

        self.ensure_dir()?;

        let path = self.file_path(&row.id);
        refuse_link(&path)?;

        if self.read_file(&row.id).is_some_and(|file| file.row.as_ref() == Ok(row)) {
            return Ok(false);
        }

        replace_atomically(
            &path,
            &envelope_bytes::<K>(row)?,
            &format!("saving a remote {}", K::NOUN),
        )?;
        Ok(true)
    }

    /// Removes the envelope of team document `id`; a missing one is fine. True when there was one.
    pub fn remove(&self, id: &str) -> Result<bool> {
        if !is_doc_id(id) {
            return Ok(false);
        }

        match std::fs::remove_file(self.file_path(id)) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(BusinessError::from_io(format!("removing a remote {}", K::NOUN), error)),
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

        let bytes = pretty_json(projects, &format!("remote {}", K::PLURAL))?;

        replace_atomically(&path, &bytes, "saving the remote projects")
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

    /// Deletes the folder with every copy (sharing is off, or the server has no team documents of
    /// this kind). True when there was one.
    pub fn clear(&self) -> Result<bool> {
        let removing = || format!("removing the remote {} folder", K::PLURAL);

        match std::fs::symlink_metadata(&self.dir) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(BusinessError::from_io(
                format!("reading the remote {} folder", K::PLURAL),
                error,
            )),
            // A link is removed, never followed.
            Ok(metadata) if !metadata.is_dir() => std::fs::remove_file(&self.dir)
                .map(|()| true)
                .map_err(|error| BusinessError::from_io(removing(), error)),
            Ok(_) => std::fs::remove_dir_all(&self.dir)
                .map(|()| true)
                .map_err(|error| BusinessError::from_io(removing(), error)),
        }
    }

    /// Creates the folder (`0700` on Unix) when missing; `Forbidden` when it is a link or other
    /// users can change it.
    fn ensure_dir(&self) -> Result<()> {
        refuse_link(&self.dir)?;
        create_private_dir(&self.dir, &format!("creating the remote {} folder", K::PLURAL))?;

        if !self.readable() {
            return Err(BusinessError::Forbidden(format!(
                "other users can change the remote {} folder",
                K::PLURAL
            )));
        }

        Ok(())
    }
}
