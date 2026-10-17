//! Which user MCP files the app registered its own MCP server in, and the program it wrote there
//! (`desktop-mcp-registration.json` in the app's config folder): a server the user removed is not
//! added back on the next launch, and an entry still naming an older program of the app (a moved
//! AppImage) is recognised as the app's and repointed.
//!
//! The file holds `{"version": 1, "registered": [{"harness": "omp", "file": "~/.omp/agent/mcp.json",
//! "command": "/usr/bin/harness-desktop"}]}`, never file contents. It is read with a 64 KiB cap and
//! replaced atomically. A record that cannot be used is renamed
//! `desktop-mcp-registration.json.corrupt-<unix-seconds>` and rebuilt: for that launch every file
//! counts as registered, so nothing is added, and every file checked is recorded.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::desktop_register::DesktopRegistration;
use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{replace_atomically, unix_nanos};
use crate::model::HarnessId;
use crate::pi_family::tilde;
use crate::security::SafeReader;

/// File name inside the app's config folder.
pub const DESKTOP_RECORD_FILE: &str = "desktop-mcp-registration.json";

/// Size cap for reading the record.
const RECORD_CAP: u64 = 64 * 1024;

/// One MCP file the server was registered in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopRecordEntry {
    pub harness: HarnessId,
    /// Home-relative path of the MCP file.
    pub file: String,
    /// The program the app last wrote into the entry, when it wrote one.
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    registered: Vec<DesktopRecordEntry>,
}

/// The record in one config folder, as loaded (see the module docs).
#[derive(Debug)]
pub struct DesktopRecord {
    path: PathBuf,
    entries: Vec<DesktopRecordEntry>,
    rebuilding: bool,
    dirty: bool,
}

impl DesktopRecord {
    /// Loads the record in `config_dir`: empty when there is none, rebuilding when it cannot be
    /// used. Never fails.
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(DESKTOP_RECORD_FILE);

        let stored = match SafeReader::read_to_string(&path, RECORD_CAP) {
            Err(BusinessError::NotFound(_)) => Ok(Vec::new()),
            Err(_) => Err(()),
            Ok(text) => match serde_json::from_str::<Stored>(&text) {
                Ok(stored) if stored.version == 1 => Ok(stored.registered),
                _ => Err(()),
            },
        };

        let rebuilding = stored.is_err();

        if rebuilding {
            tracing::warn!("unusable desktop MCP registration record; rebuilding it");
            set_aside(&path);
        }

        Self {
            path,
            entries: stored.unwrap_or_default(),
            rebuilding,
            dirty: rebuilding,
        }
    }

    /// The files recorded so far.
    pub fn entries(&self) -> &[DesktopRecordEntry] {
        &self.entries
    }

    /// Whether the record stands in for one that could not be used.
    pub fn rebuilding(&self) -> bool {
        self.rebuilding
    }

    /// Whether the server counts as registered in `path` before: recorded, or any file while the
    /// record is rebuilt.
    pub fn is_recorded(&self, env: &Env, path: &Path) -> bool {
        self.rebuilding || self.entry(env, path).is_some()
    }

    /// The program the app last wrote into `path`.
    pub fn command_in(&self, env: &Env, path: &Path) -> Option<&str> {
        self.entry(env, path)?.command.as_deref()
    }

    /// Records `path` when `outcome` left the server in it (any outcome while the record is
    /// rebuilt), with `command` when the app wrote it.
    pub fn note(&mut self, env: &Env, harness: HarnessId, path: &Path, outcome: DesktopRegistration, command: &Path) {
        if !self.rebuilding && !outcome.is_registered() {
            return;
        }

        let file = tilde(env, path);
        let written = outcome.wrote().then(|| command.to_string_lossy().into_owned());

        match self.entries.iter_mut().find(|entry| entry.file == file) {
            Some(entry) => {
                if written.is_some() && entry.command != written {
                    entry.command = written;
                    self.dirty = true;
                }
            }
            None => {
                self.entries.push(DesktopRecordEntry {
                    harness,
                    file,
                    command: written,
                });
                self.dirty = true;
            }
        }
    }

    /// Writes the record when it changed since it was loaded (the folder is created when missing).
    pub fn save(&self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let stored = Stored {
            version: 1,
            registered: self.entries.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&stored)
            .map_err(|error| BusinessError::Internal(format!("encoding the desktop MCP record: {error}")))?;

        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|error| BusinessError::from_io("creating the app's config folder", error))?;
        }

        replace_atomically(&self.path, &bytes, "saving the desktop MCP record")
    }

    fn entry(&self, env: &Env, path: &Path) -> Option<&DesktopRecordEntry> {
        let file = tilde(env, path);

        self.entries.iter().find(|entry| entry.file == file)
    }
}

/// Keeps an unusable record for inspection, beside the new one.
fn set_aside(path: &Path) {
    let mut aside = path.as_os_str().to_owned();
    aside.push(format!(".corrupt-{}", unix_nanos() / 1_000_000_000));

    match std::fs::rename(path, &aside) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(%error, "cannot move the desktop MCP record aside"),
    }
}
