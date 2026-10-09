//! Atomic, self-healing storage of [`AppPrefs`].
//!
//! The file holds only the fields of [`AppPrefs`] — never secrets or chat content.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{BusinessError, Result};
use crate::model::{AppPrefs, BinaryOverride};
use crate::security::SafeReader;

/// File name inside the store directory.
pub const PREFS_FILE: &str = "prefs.json";

/// Maximum number of remembered recent projects.
pub const MAX_RECENT_PROJECTS: usize = 12;

/// Size cap for reading the prefs file.
const MAX_PREFS_BYTES: u64 = 256 * 1024;

/// Reads and writes `prefs.json` in one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefsStore {
    dir: PathBuf,
}

impl PrefsStore {
    /// A store rooted at `dir` (created on first save).
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Path of the prefs file.
    pub fn path(&self) -> PathBuf {
        self.dir.join(PREFS_FILE)
    }

    /// Loads normalized prefs. Never fails and never panics:
    /// a missing or unreadable file yields defaults; a corrupt file (invalid JSON, invalid UTF-8,
    /// oversized) is renamed to `prefs.json.corrupt-<unix-seconds>` and defaults are returned.
    pub fn load(&self) -> AppPrefs {
        let path = self.path();

        let text = match SafeReader::read_to_string(&path, MAX_PREFS_BYTES) {
            Ok(text) => text,
            Err(BusinessError::NotFound(_)) => return AppPrefs::default(),
            Err(BusinessError::InvalidInput(reason)) => {
                tracing::warn!(%reason, "unreadable prefs file");
                self.quarantine(&path);
                return AppPrefs::default();
            }
            Err(error) => {
                tracing::warn!(%error, "cannot read prefs; using defaults");
                return AppPrefs::default();
            }
        };

        match serde_json::from_str::<AppPrefs>(&text) {
            Ok(prefs) => normalize(prefs),
            Err(error) => {
                // serde_json errors carry a position, not file contents.
                tracing::warn!(%error, "corrupt prefs file");
                self.quarantine(&path);
                AppPrefs::default()
            }
        }
    }

    /// Normalizes `prefs`, writes them atomically and returns what was stored.
    ///
    /// The write goes to a temporary file in the same directory, is flushed with `sync_all` and
    /// then renamed over `prefs.json`. On Unix the directory is `0700` and the file `0600`.
    pub fn save(&self, prefs: &AppPrefs) -> Result<AppPrefs> {
        let prefs = normalize(prefs.clone());
        let json = serde_json::to_vec_pretty(&prefs)
            .map_err(|error| BusinessError::Internal(format!("serializing prefs: {error}")))?;

        self.ensure_dir()?;

        let temp = self
            .dir
            .join(format!(".{PREFS_FILE}.tmp-{}-{}", std::process::id(), unix_nanos()));
        let written = write_synced(&temp, &json)
            .and_then(|()| fs::rename(&temp, self.path()).map_err(|error| io_error("replacing prefs file", error)));

        if written.is_err() {
            fs::remove_file(&temp).ok();
        }

        written.map(|()| prefs)
    }

    fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.dir).map_err(|error| io_error("creating prefs directory", error))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(&self.dir, fs::Permissions::from_mode(0o700))
                .map_err(|error| io_error("restricting prefs directory", error))?;
        }

        Ok(())
    }

    /// Moves a corrupt file aside so the next save does not destroy it.
    fn quarantine(&self, path: &Path) {
        let backup = self.dir.join(format!("{PREFS_FILE}.corrupt-{}", unix_seconds()));

        if let Err(error) = fs::rename(path, &backup) {
            tracing::warn!(%error, "cannot move corrupt prefs aside");
        }
    }
}

/// Applies the storage invariants:
/// - `version` is the current schema version;
/// - `recentProjects`: absolute paths only, duplicates removed (first occurrence kept), at most
///   [`MAX_RECENT_PROJECTS`], and empty when `rememberRecentProjects` is off;
/// - `activeProject`: absolute or `None`;
/// - `binaryOverrides`: absolute paths only, one per harness (the last entry wins, keeping its
///   position among the survivors).
pub fn normalize(mut prefs: AppPrefs) -> AppPrefs {
    prefs.version = AppPrefs::VERSION;

    prefs.recent_projects = if prefs.remember_recent_projects {
        let mut seen = Vec::<String>::new();

        for project in prefs.recent_projects {
            if is_absolute(&project) && !seen.contains(&project) {
                seen.push(project);
            }
        }

        seen.truncate(MAX_RECENT_PROJECTS);
        seen
    } else {
        Vec::new()
    };

    prefs.active_project = prefs.active_project.filter(|project| is_absolute(project));
    prefs.binary_overrides = normalize_overrides(prefs.binary_overrides);

    prefs
}

fn normalize_overrides(overrides: Vec<BinaryOverride>) -> Vec<BinaryOverride> {
    let mut kept: Vec<BinaryOverride> = Vec::new();

    for entry in overrides.into_iter().rev() {
        let duplicate = kept.iter().any(|existing| existing.harness == entry.harness);

        if is_absolute(&entry.path) && !duplicate {
            kept.push(entry);
        }
    }

    kept.reverse();
    kept
}

fn is_absolute(path: &str) -> bool {
    !path.is_empty() && Path::new(path).is_absolute()
}

fn write_synced(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);

    let mut file = options
        .open(path)
        .map_err(|error| io_error("creating temporary prefs file", error))?;

    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| io_error("writing prefs file", error))
}

fn io_error(context: &str, error: std::io::Error) -> BusinessError {
    BusinessError::from_io(context, error)
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos())
}
