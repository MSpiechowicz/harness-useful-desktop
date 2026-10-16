//! Which user MCP files the app registered the memory server in (`memory-registration.json` in the
//! app's config folder), so a server the user removed is not added back on the next launch.
//!
//! The file holds `{"version": 1, "registered": [{"harness": "omp", "file": "~/.omp/agent/mcp.json"}]}`,
//! one entry per MCP file (home-relative), never file contents. It is read with a 64 KiB cap and
//! replaced atomically. A file that cannot be used is moved aside
//! (`memory-registration.json.corrupt-<unix-seconds>`) and rebuilt: for that launch every file counts
//! as registered, so nothing is added, and every file checked is recorded, so an entry the user
//! removed is not added on later launches either.
//! Turning project memory on clears the record, so the server is added back everywhere.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::memory_register::MemoryRegistration;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::replace_atomically;
use crate::model::HarnessId;
use crate::pi_family::tilde;
use crate::security::SafeReader;

/// File name inside the app's config folder.
pub const MEMORY_RECORD_FILE: &str = "memory-registration.json";

/// Size cap for reading the record.
const MAX_RECORD_BYTES: u64 = 64 * 1024;

/// Schema version written by this build.
const VERSION: u32 = 1;

/// One MCP file the server was registered in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedFile {
    pub harness: HarnessId,
    /// Home-relative path of the MCP file.
    pub file: String,
}

#[derive(Serialize, Deserialize)]
struct RecordDocument {
    version: u32,
    registered: Vec<RecordedFile>,
}

/// The record of one config folder, as loaded (see the module docs).
#[derive(Debug)]
pub struct MemoryRecord {
    dir: PathBuf,
    registered: Vec<RecordedFile>,
    rebuilding: bool,
    changed: bool,
}

impl MemoryRecord {
    /// Loads the record in `dir`: empty when there is none, rebuilding when it cannot be used.
    /// Never fails.
    pub fn load(dir: &Path) -> Self {
        let path = dir.join(MEMORY_RECORD_FILE);
        let empty = |rebuilding| Self {
            dir: dir.to_path_buf(),
            registered: Vec::new(),
            rebuilding,
            changed: rebuilding,
        };

        let text = match SafeReader::read_to_string(&path, MAX_RECORD_BYTES) {
            Ok(text) => text,
            Err(BusinessError::NotFound(_)) => return empty(false),
            Err(error) => {
                tracing::warn!(%error, "unusable memory registration record; rebuilding it");
                quarantine(dir, &path);
                return empty(true);
            }
        };

        match serde_json::from_str::<RecordDocument>(&text) {
            Ok(document) if document.version == VERSION => Self {
                registered: document.registered,
                ..empty(false)
            },
            _ => {
                tracing::warn!("corrupt memory registration record; rebuilding it");
                quarantine(dir, &path);
                empty(true)
            }
        }
    }

    /// Removes the record in `dir`, if any.
    pub fn clear(dir: &Path) -> Result<()> {
        match fs::remove_file(dir.join(MEMORY_RECORD_FILE)) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(BusinessError::from_io("removing the memory registration record", error))
            }
            _ => Ok(()),
        }
    }

    /// Whether the record stands in for one that could not be used.
    pub fn rebuilding(&self) -> bool {
        self.rebuilding
    }

    /// The files recorded so far.
    pub fn registered(&self) -> &[RecordedFile] {
        &self.registered
    }

    /// Whether the server counts as registered in `path` before: recorded, or any file while the
    /// record is rebuilt.
    pub fn is_recorded(&self, env: &Env, path: &Path) -> bool {
        let file = tilde(env, path);

        self.rebuilding || self.registered.iter().any(|entry| entry.file == file)
    }

    /// Records `path` when `outcome` left the server in it, or whatever the outcome while the record
    /// is rebuilt.
    pub fn note(&mut self, env: &Env, harness: HarnessId, path: &Path, outcome: MemoryRegistration) {
        let file = tilde(env, path);
        let known = self.registered.iter().any(|entry| entry.file == file);
        let keep = self.rebuilding || outcome.is_registered();

        if keep && !known {
            self.registered.push(RecordedFile { harness, file });
            self.changed = true;
        }
    }

    /// Writes the record when it changed since it was loaded (the folder is created when missing).
    pub fn save(&self) -> Result<()> {
        if !self.changed {
            return Ok(());
        }

        let document = RecordDocument {
            version: VERSION,
            registered: self.registered.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&document)
            .map_err(|error| BusinessError::Internal(format!("serializing the memory record: {error}")))?;

        fs::create_dir_all(&self.dir)
            .map_err(|error| BusinessError::from_io("creating the app's config folder", error))?;

        replace_atomically(
            &self.dir.join(MEMORY_RECORD_FILE),
            &bytes,
            "replacing the memory registration record",
        )
    }
}

/// Moves an unusable record aside so it is kept for inspection.
fn quarantine(dir: &Path, path: &Path) {
    let seconds = crate::fs_atomic::unix_nanos() / 1_000_000_000;
    let backup = dir.join(format!("{MEMORY_RECORD_FILE}.corrupt-{seconds}"));

    if let Err(error) = fs::rename(path, &backup) {
        tracing::warn!(%error, "cannot move the memory registration record aside");
    }
}
