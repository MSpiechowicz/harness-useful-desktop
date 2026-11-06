//! `guards/global.json` as the app reads and writes it: version 2 (saving is opt-in), version 1 (the
//! file before that, read as version 2 would hold it) and the move from one to the other
//! ([`ensure_global`]).
//!
//! Each kind of a version 2 file pins every entry of `onRepos` to the project key it had when the
//! user turned it on (`repoKeys`, `null` for a folder with no project). A version 2 file written
//! before the pins has none: it reads with `repoKeys: {}` (every entry unpinned, so turning
//! nothing on) and [`ensure_global`] writes it back with the empty object.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::lock::GlobalLock;
use super::{
    check_owner, global_path, read_file, read_global, read_listed, ready_guards_dir, refuse_link, unreadable,
    write_bytes, write_global,
};
use crate::ecosystem::{memory_data_dir, resolve_memory};
use crate::env::Env;
use crate::error::Result;
use crate::model::{GlobalKindGuard, MemoryGuards};

/// The marker in the guards folder that says saving is opt-in on this computer: `{"version":1}`.
/// Harness Memory refuses every save while it exists and `global.json` does not, instead of
/// treating the missing file as "no rules".
pub const OPT_IN_MARKER: &str = "opt-in";

/// The marker's content.
const MARKER_TEXT: &str = "{\"version\":1}";

/// The format of `guards/global.json` written and read: saving is opt-in.
const GLOBAL_VERSION: u32 = 2;

/// The format of `guards/global.json` before saving was opt-in.
const LEGACY_GLOBAL_VERSION: u32 = 1;

/// `guards/global.json` as written (version 2).
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GlobalFile {
    version: u32,
    notes: KindFile,
    scripts: KindFile,
}

/// One kind in `guards/global.json`: only what the server reads (it refuses unknown keys), so what
/// is computed on a read ([`GlobalKindGuard::dead_repos`], [`GlobalKindGuard::changed_repos`])
/// never reaches the file. `repoKeys` is read as optional (a file before the pins has none, see
/// [`GlobalRead::Unpinned`]) and always written.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KindFile {
    paused: bool,
    every_project: bool,
    on_repos: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repo_keys: Option<BTreeMap<String, Option<String>>>,
}

impl KindFile {
    fn of(guard: &GlobalKindGuard) -> Self {
        Self {
            paused: guard.paused,
            every_project: guard.every_project,
            on_repos: guard.on_repos.clone(),
            repo_keys: Some(guard.repo_keys.clone()),
        }
    }

    /// True when the file has the `repoKeys` of this kind.
    fn is_pinned(&self) -> bool {
        self.repo_keys.is_some()
    }

    fn guard(self) -> GlobalKindGuard {
        GlobalKindGuard {
            paused: self.paused,
            every_project: self.every_project,
            on_repos: self.on_repos,
            repo_keys: self.repo_keys.unwrap_or_default(),
            dead_repos: Vec::new(),
            changed_repos: Vec::new(),
        }
    }
}

/// The version of any `guards/global.json`.
#[derive(Deserialize)]
struct GlobalHeader {
    version: u32,
}

/// `guards/global.json` of version 1: only the pauses are read, its `offRepos` are dropped.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyGlobalFile {
    notes: LegacyKindGuard,
    scripts: LegacyKindGuard,
}

#[derive(Deserialize)]
struct LegacyKindGuard {
    paused: bool,
}

/// A `guards/global.json` that was read.
pub(super) enum GlobalRead {
    /// Version 2, as it stands.
    Current(MemoryGuards),
    /// Version 2 written before the pins: some kind has no `repoKeys`. Read with every entry
    /// unpinned, and written again by [`ensure_global`] with `repoKeys: {}`.
    Unpinned(MemoryGuards),
    /// Version 1, as version 2 would hold it: the pauses, saving off.
    Legacy(MemoryGuards),
}

impl GlobalFile {
    /// `guards` as a version 2 file.
    pub(super) fn of(guards: &MemoryGuards) -> Self {
        Self {
            version: GLOBAL_VERSION,
            notes: KindFile::of(&guards.notes),
            scripts: KindFile::of(&guards.scripts),
        }
    }
}

impl GlobalRead {
    pub(super) fn parse(text: &str) -> Result<Self> {
        let header: GlobalHeader = serde_json::from_str(text).map_err(|_| unreadable())?;

        match header.version {
            GLOBAL_VERSION => {
                let file: GlobalFile = serde_json::from_str(text).map_err(|_| unreadable())?;
                let pinned = file.notes.is_pinned() && file.scripts.is_pinned();

                let mut guards = MemoryGuards {
                    notes: file.notes.guard(),
                    scripts: file.scripts.guard(),
                    migration_pending: false,
                };

                if pinned {
                    return Ok(Self::Current(guards));
                }

                // Entries from before pins turn nothing on at the server, so they are dropped and
                // not shown as on: the user turns those projects on again, which pins them.
                guards.notes.on_repos.clear();
                guards.scripts.on_repos.clear();

                Ok(Self::Unpinned(guards))
            }
            LEGACY_GLOBAL_VERSION => {
                let file: LegacyGlobalFile = serde_json::from_str(text).map_err(|_| unreadable())?;

                Ok(Self::Legacy(MemoryGuards {
                    notes: GlobalKindGuard {
                        paused: file.notes.paused,
                        ..GlobalKindGuard::default()
                    },
                    scripts: GlobalKindGuard {
                        paused: file.scripts.paused,
                        ..GlobalKindGuard::default()
                    },
                    migration_pending: true,
                }))
            }
            _ => Err(unreadable()),
        }
    }

    pub(super) fn guards(self) -> MemoryGuards {
        match self {
            Self::Current(guards) | Self::Unpinned(guards) | Self::Legacy(guards) => guards,
        }
    }
}

/// Brings the opt-in guards in `data_dir` to the current form, once, before anything reads or
/// changes them (when the app starts, when any agent starts and when the guards are read), under
/// the same lock as [`update_global`](super::update_global):
///
/// - the marker `guards/opt-in` is written first, whatever `global.json` is (see
///   [`OPT_IN_MARKER`]);
/// - `global.json` missing: written with saving off for every project and no repository on;
/// - version 1: written as version 2 with the same pauses, saving off (its `offRepos` are dropped:
///   they listed where saving was off, and saving is off everywhere now);
/// - version 2: left as it is, byte for byte, unless a kind has no `repoKeys` (a file from before
///   the pins): written again with `repoKeys: {}` and the same entries, which stay unpinned and
///   turn nothing on until the user turns them on again;
/// - a file that cannot be read (not JSON, a symbolic link, another version): left as it is, with
///   a warning. The server refuses every save against it, so nothing is saved by mistake.
///
/// A state that is already current is found without the lock ([`is_current`]): the guards are read
/// on every look at them, and a second desktop that holds the lock for a while must not hold those
/// reads. Anything else takes the lock and looks again under it.
///
/// Callers run it only where Harness Memory is in use ([`guards_data_dir`]): it creates the data
/// folder when it is missing.
///
/// An error only when the marker or the file could not be written (a data folder of another user,
/// one other users can write to, a disk that refuses, a symbolic link where the marker belongs) or
/// the lock stays taken by another process.
pub fn ensure_global(data_dir: &Path) -> Result<()> {
    if is_current(data_dir) {
        return Ok(());
    }

    let _writing = GlobalLock::take(data_dir)?;

    write_marker(&ready_guards_dir(data_dir)?)?;

    let path = global_path(data_dir);
    let text = match read_file(&path) {
        Ok(text) => text,
        Err(error) => {
            tracing::warn!(code = ?error.code(), "the memory save guards file is left as it is: it cannot be read");
            return Ok(());
        }
    };

    let guards = match text.as_deref().map(GlobalRead::parse) {
        None => MemoryGuards::default(),
        Some(Ok(GlobalRead::Current(_))) => return Ok(()),
        Some(Ok(GlobalRead::Unpinned(guards) | GlobalRead::Legacy(guards))) => guards,
        Some(Err(error)) => {
            tracing::warn!(code = ?error.code(), "the memory save guards file is left as it is: it cannot be read");
            return Ok(());
        }
    };

    write_global(data_dir, &guards)?;

    Ok(())
}

/// Whether [`ensure_global`] would change nothing, found without the lock: both folders are ready
/// as [`ready_guards_dir`] leaves them (not links, this user's, the guards folder `0700`), the marker is
/// the plain file it writes and `global.json` reads as version 2. Anything else, an unreadable
/// file included, is not current and takes the slow path.
fn is_current(data_dir: &Path) -> bool {
    let guards = data_dir.join(super::GUARDS_DIR);

    if !folder_is_ready(data_dir, false) || !folder_is_ready(&guards, true) {
        return false;
    }

    if !marker_is_current(&guards.join(OPT_IN_MARKER)) {
        return false;
    }

    let Ok(Some(text)) = read_file(&global_path(data_dir)) else {
        return false;
    };

    matches!(GlobalRead::parse(&text), Ok(GlobalRead::Current(_)))
}

/// Whether `dir` passes the checks of [`ready_guards_dir`] with nothing left to create or narrow:
/// a folder that is not a link and belongs to this user, and when `private` also `0700` (Unix).
fn folder_is_ready(dir: &Path, private: bool) -> bool {
    if refuse_link(dir).is_err() || check_owner(dir).is_err() {
        return false;
    }

    #[cfg(unix)]
    if private {
        use std::os::unix::fs::PermissionsExt;

        return std::fs::metadata(dir).is_ok_and(|metadata| metadata.permissions().mode() & 0o777 == 0o700);
    }

    #[cfg(not(unix))]
    let _ = private;

    true
}

/// Whether `path` is a plain file with the marker's content.
fn marker_is_current(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .and_then(|_| std::fs::read(path).ok())
        .as_deref()
        == Some(MARKER_TEXT.as_bytes())
}

/// Harness Memory's data folder when this computer uses Harness Memory: the folder exists or the
/// server resolves ([`resolve_memory`]). `None` otherwise (or while the folder is unknown), and
/// then the app creates no folder and no file for the guards: a user without Harness Memory never
/// gets one, nor a failure.
pub fn guards_data_dir(env: &Env) -> Option<PathBuf> {
    let data_dir = memory_data_dir(env)?;

    in_use(env, &data_dir).then_some(data_dir)
}

fn in_use(env: &Env, data_dir: &Path) -> bool {
    data_dir.exists() || resolve_memory(env).is_some()
}

/// True when `data_dir` holds a version 1 guards file that reads (a deny list, which the server
/// would keep applying with saving on).
pub fn version_one_remains(data_dir: &Path) -> bool {
    read_listed(data_dir).is_ok_and(|guards| guards.migration_pending)
}

/// True when `data_dir` holds neither `guards/global.json` nor the marker ([`OPT_IN_MARKER`]), as
/// the server finds them: it then applies no rules at all. [`ensure_global`] writes the marker
/// first, so this is what a failure before that leaves (a data folder of another user or that
/// other users can write to, a symbolic link in its place, a lock that cannot be taken).
pub fn rules_missing(data_dir: &Path) -> bool {
    let missing = |name: &str| {
        std::fs::symlink_metadata(data_dir.join(super::GUARDS_DIR).join(name))
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    };

    missing(super::GLOBAL_FILE) && missing(OPT_IN_MARKER)
}

/// [`read_global`], after [`ensure_global`] tried again: a version 1 file that could not be moved
/// before (a disk that refused, a folder that was read-only) is moved as soon as it can be, when
/// the guards are shown. A failure of the move is logged and the guards are read as they are
/// (`migrationPending` stays set for a version 1 file). Nothing is created while this computer
/// does not use Harness Memory ([`guards_data_dir`]).
pub fn read_global_healed(env: &Env, data_dir: &Path) -> Result<MemoryGuards> {
    if in_use(env, data_dir)
        && let Err(error) = ensure_global(data_dir)
    {
        tracing::warn!(code = ?error.code(), "the memory save guards could not be brought up to date");
    }

    read_global(data_dir)
}

/// Writes the marker in `guards` (`0600`, in one rename, never through a symbolic link) unless it
/// is already a plain file with that content.
fn write_marker(guards: &Path) -> Result<()> {
    let path = guards.join(OPT_IN_MARKER);

    if marker_is_current(&path) {
        return Ok(());
    }

    write_bytes(&path, MARKER_TEXT.as_bytes())
}
