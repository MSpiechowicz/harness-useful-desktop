//! Atomic, self-healing storage of [`AppPrefs`].
//!
//! The file holds only the fields of [`AppPrefs`] — never secrets or chat content.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{BusinessError, Result};
use crate::model::{AppPrefs, BinaryOverride, ChatDefaults, DEFAULT_LANGUAGE, LANGUAGES};
use crate::security::SafeReader;

/// File name inside the store directory.
pub const PREFS_FILE: &str = "prefs.json";

/// Maximum number of remembered recent projects.
pub const MAX_RECENT_PROJECTS: usize = 12;

/// Size cap for reading the prefs file.
const MAX_PREFS_BYTES: u64 = 256 * 1024;

/// Where the prefs returned by [`PrefsStore::load_checked`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefsOrigin {
    /// Read from `prefs.json`.
    Stored,
    /// Defaults: nothing was ever stored.
    Missing,
    /// Defaults standing in for a file that exists, existed as corrupt, or could not be read.
    Unusable,
}

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
        self.load_checked().0
    }

    /// [`load`](Self::load), also telling whether the prefs are the stored ones, defaults for a
    /// store that never held a file, or defaults standing in for a file that could not be used.
    ///
    /// A file moved aside as corrupt (by this or an earlier load) counts as unusable until the
    /// next save writes a new `prefs.json`, so a choice the user made is never taken as "unset".
    pub fn load_checked(&self) -> (AppPrefs, PrefsOrigin) {
        let path = self.path();

        let text = match SafeReader::read_to_string(&path, MAX_PREFS_BYTES) {
            Ok(text) => text,
            Err(BusinessError::NotFound(_)) => {
                let origin = if self.has_quarantined() {
                    PrefsOrigin::Unusable
                } else {
                    PrefsOrigin::Missing
                };

                return (AppPrefs::default(), origin);
            }
            Err(BusinessError::InvalidInput(reason)) => {
                tracing::warn!(%reason, "unreadable prefs file");
                self.quarantine(&path);
                return (AppPrefs::default(), PrefsOrigin::Unusable);
            }
            Err(error) => {
                tracing::warn!(%error, "cannot read prefs; using defaults");
                return (AppPrefs::default(), PrefsOrigin::Unusable);
            }
        };

        match serde_json::from_str::<AppPrefs>(&text) {
            Ok(prefs) => (normalize(prefs), PrefsOrigin::Stored),
            Err(error) => {
                // serde_json errors carry a position, not file contents.
                tracing::warn!(%error, "corrupt prefs file");
                self.quarantine(&path);
                (AppPrefs::default(), PrefsOrigin::Unusable)
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

    /// Whether a corrupt file was moved aside (see [`quarantine`](Self::quarantine)).
    fn has_quarantined(&self) -> bool {
        let prefix = format!("{PREFS_FILE}.corrupt-");
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return false;
        };

        entries
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
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
/// - `language`: trimmed and ASCII-lowercased when that is one of [`LANGUAGES`], otherwise
///   [`DEFAULT_LANGUAGE`] (also for anything longer than 16 bytes);
/// - `recentProjects`: absolute paths only, duplicates removed (first occurrence kept), at most
///   [`MAX_RECENT_PROJECTS`], and empty when `rememberRecentProjects` is off;
/// - `activeProject`: absolute or `None`;
/// - `binaryOverrides`: absolute paths only, one per harness (the last entry wins, keeping its
///   position among the survivors);
/// - `chatDefaults`: one per harness (the last entry wins), each value at most
///   [`MAX_CHAT_DEFAULT_LEN`] bytes and free of control characters (anything else is dropped).
pub fn normalize(mut prefs: AppPrefs) -> AppPrefs {
    prefs.version = AppPrefs::VERSION;
    prefs.language = normalize_language(&prefs.language);

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
    prefs.chat_defaults = normalize_chat_defaults(prefs.chat_defaults);

    prefs
}

/// Longest stored `language` value, surrounding whitespace included, that can still be one of
/// [`LANGUAGES`]; anything longer becomes [`DEFAULT_LANGUAGE`] without being copied.
const MAX_LANGUAGE_LEN: usize = 16;

fn normalize_language(language: &str) -> String {
    if language.len() > MAX_LANGUAGE_LEN {
        return DEFAULT_LANGUAGE.to_owned();
    }

    let code = language.trim().to_ascii_lowercase();

    if LANGUAGES.contains(&code.as_str()) {
        code
    } else {
        DEFAULT_LANGUAGE.to_owned()
    }
}

/// Longest model provider, model id or thinking level kept in `chatDefaults`.
pub const MAX_CHAT_DEFAULT_LEN: usize = 200;

fn normalize_chat_defaults(defaults: Vec<ChatDefaults>) -> Vec<ChatDefaults> {
    let clean = |value: Option<String>| {
        value.filter(|value| {
            !value.is_empty() && value.len() <= MAX_CHAT_DEFAULT_LEN && !value.chars().any(char::is_control)
        })
    };
    let mut kept: Vec<ChatDefaults> = Vec::new();

    for entry in defaults.into_iter().rev() {
        if kept.iter().any(|existing| existing.harness == entry.harness) {
            continue;
        }

        let (provider, model_id) = match (clean(entry.provider), clean(entry.model_id)) {
            (Some(provider), Some(model_id)) => (Some(provider), Some(model_id)),
            _ => (None, None),
        };

        kept.push(ChatDefaults {
            harness: entry.harness,
            provider,
            model_id,
            thinking_level: clean(entry.thinking_level),
        });
    }

    kept.reverse();
    kept
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
