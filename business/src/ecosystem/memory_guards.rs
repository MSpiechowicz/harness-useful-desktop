//! The save guards Harness Memory applies to agents, kept in its data folder
//! ([`memory_data_dir`](super::memory_data_dir)) and written by this app only:
//!
//! - `guards/global.json` ([`GLOBAL_FILE`]): `{"version": 2, "notes": {"paused", "everyProject",
//!   "onRepos", "repoKeys"}, "scripts": {...}}`, the guards of every chat (`memory_guards_get`,
//!   `memory_guards_update`, one change at a time under a lock in this process and on
//!   `guards/global.lock` for other processes: [`update_global`]). Saving is opt-in: it is off
//!   until `everyProject` is on or the project's [`saving_root`] is in `onRepos` ([`repo_is_on`])
//!   and still has the project key its entry was pinned with in `repoKeys` (the key the server gave
//!   the folder when the user turned it on, `null` for one with no project: [`Pin`]). This app
//!   cannot compute keys, so its own rule is by path and an entry with no pin counts as dead;
//!   `changedRepos` (the entries whose key changed) is asked of the server by the commands.
//!   Version 1 (`offRepos`, saving on) becomes version 2 when the app starts, when any agent starts
//!   and when the guards are read ([`ensure_global`]): the pause stays, the list does not. This
//!   happens only on a computer that uses Harness Memory ([`guards_data_dir`]: the data folder exists
//!   or the server resolves), so a user without it gets no folder, no file and no failed start. A
//!   start fails when the move fails only if it has a memory chat or a readable version 1 file
//!   remains, with "Harness Memory settings could not be updated: <reason>" (`io`);
//! - `guards/opt-in` ([`OPT_IN_MARKER`]): `{"version":1}`, written before `global.json` by every
//!   [`ensure_global`]. While it exists, Harness Memory takes a missing `global.json` as unreadable
//!   (refuses) instead of "no rules", so a deleted file cannot turn saving back on;
//! - `guards/chats/<key>.json` ([`CHATS_DIR`]): `{"version": 1, "paused", "notes": {"on", "ask"},
//!   "scripts": {...}, "repoRoot"}`, the guards of one chat and the root of its project (its git
//!   root, else the folder: [`chat_repo_root`]), written when the chat starts, on every toggle
//!   (`memory_chat_guards_set`, newest revision only: [`ChatGuardWrites`]) and when it moves. The chat's harness gets the key
//!   as [`SESSION_VAR`] only once its file is written. Files untouched for [`CHAT_MAX_AGE`] go at
//!   startup ([`sweep_chats`]).
//!
//! The server reads them on every gated call: a missing `global.json` is no rules only while
//! `guards/opt-in` does not exist (an older desktop has chats and no file and no marker, while this
//! app treats a missing file as off and writes the marker and the file before any agent starts),
//! and with a session a missing or unreadable chat file refuses. Folders are `0700` and this user's, files `0600`, written in one rename, never through
//! a symbolic link; a file of another version is unreadable. The app creates the data folder when
//! it is missing (`0700`), as the server does.
//!
//! The hooks of one chat read both through [`read_memory_guards`](crate::hooks::read_memory_guards).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, ensure_private_dir, refuse_link, replace_atomically};
use crate::hooks::project_identity;
use crate::model::{KindGuard, MemoryChatGuards, MemoryGuards, MemoryGuardsPatch};
use crate::security::SafeReader;

use global::{GlobalFile, GlobalRead};
use saving_root::names_itself;

/// The variable that names a chat to Harness Memory (its key).
pub const SESSION_VAR: &str = "HARNESS_MEMORY_SESSION";

/// The guards folder in the data folder.
pub const GUARDS_DIR: &str = "guards";

/// The guards of every chat, in [`GUARDS_DIR`].
pub const GLOBAL_FILE: &str = "global.json";

/// The folder of each chat's guards, in [`GUARDS_DIR`].
pub const CHATS_DIR: &str = "chats";

/// The format of a chat's file.
const VERSION: u32 = 1;

/// The largest guards file read and written (256 KiB). A full list of long paths can pass it
/// ([`MAX_ON_REPOS`] of [`MAX_REPO_CHARS`] each), and a write over it is refused ([`write_global`]).
const MAX_FILE_BYTES: u64 = 256 * 1024;

/// The most repositories turned on per kind.
pub const MAX_ON_REPOS: usize = 256;

/// The longest repository path kept, in characters.
const MAX_REPO_CHARS: usize = 4096;

/// Whether `key` is a chat key: a lowercase UUID (`8-4-4-4-12` lowercase hex digits).
pub fn is_chat_key(key: &str) -> bool {
    let groups: Vec<&str> = key.split('-').collect();
    let lengths = [8, 4, 4, 4, 12];

    groups.len() == lengths.len()
        && groups.iter().zip(lengths).all(|(group, length)| {
            group.len() == length && group.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        })
}

/// `invalidInput` unless `key` is a chat key ([`is_chat_key`]).
pub fn check_chat_key(key: &str) -> Result<()> {
    if is_chat_key(key) {
        return Ok(());
    }

    Err(BusinessError::InvalidInput(
        "a memory chat key is a lowercase UUID".to_owned(),
    ))
}

/// `guards/chats/<key>.json` as written.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatFile {
    version: u32,
    paused: bool,
    notes: KindGuard,
    scripts: KindGuard,
    #[serde(default)]
    repo_root: Option<String>,
}

/// The guards of every chat in `data_dir`: saving off while the file is missing. A file of version
/// 1 reads as [`ensure_global`] would write it (the pauses, saving off) with `migrationPending`
/// set, and is not changed. `protocol` for a file that cannot be read (a symbolic link, too large,
/// of another version), which the server takes as "refuse every save".
///
/// Each kind's `dead_repos` are the entries that turn nothing on ([`names_itself`], or no pin),
/// computed here with one canonicalisation per entry and never written ([`GlobalFile`]).
pub fn read_global(data_dir: &Path) -> Result<MemoryGuards> {
    read_listed(data_dir).map(with_dead_repos)
}

/// [`read_global`] without `dead_repos`: for what only reads or rewrites the entries.
pub(crate) fn read_listed(data_dir: &Path) -> Result<MemoryGuards> {
    let Some(text) = read_file(&global_path(data_dir))? else {
        return Ok(MemoryGuards::default());
    };

    Ok(GlobalRead::parse(&text)?.guards())
}

/// `guards` with each kind's `dead_repos` filled in: the entries that name nothing or have no pin.
fn with_dead_repos(mut guards: MemoryGuards) -> MemoryGuards {
    for kind in [&mut guards.notes, &mut guards.scripts] {
        kind.dead_repos = kind
            .on_repos
            .iter()
            .filter(|entry| !names_itself(Path::new(entry)) || !kind.repo_keys.contains_key(*entry))
            .cloned()
            .collect();
    }

    guards
}

/// Writes the guards of every chat to `data_dir` and returns them as written: the repositories as
/// given (a change turns one on as its saving root, see [`change`]; the ones listed stay as they
/// are), without duplicates, sorted. `invalidInput` for a repository that is not an absolute path,
/// too long, more than [`MAX_ON_REPOS`] of them, or a file that would pass [`MAX_FILE_BYTES`] (the
/// file is left as it is). The caller holds the [`GlobalLock`].
pub(crate) fn write_global(data_dir: &Path, guards: &MemoryGuards) -> Result<MemoryGuards> {
    let guards = MemoryGuards {
        notes: change::listed_kind(&guards.notes)?,
        scripts: change::listed_kind(&guards.scripts)?,
        migration_pending: false,
    };

    let bytes = encoded(&GlobalFile::of(&guards))?;

    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(BusinessError::InvalidInput(format!(
            "the repositories turned on take more than {} KiB of settings: turn some off",
            MAX_FILE_BYTES / 1024
        )));
    }

    let guards_dir = ready_guards_dir(data_dir)?;
    write_bytes(&guards_dir.join(GLOBAL_FILE), &bytes)?;

    tracing::info!(
        notes_paused = guards.notes.paused,
        notes_every_project = guards.notes.every_project,
        notes_on = guards.notes.on_repos.len(),
        scripts_paused = guards.scripts.paused,
        scripts_every_project = guards.scripts.every_project,
        scripts_on = guards.scripts.on_repos.len(),
        "memory save guards saved"
    );

    Ok(guards)
}

/// Applies `patch` to the guards of every chat saved in `data_dir` now and returns them as written
/// (see [`write_global`]): pause or resume the kind, turn it on for every project or only for the
/// listed ones (`everyProject`), turn a repository on (`addRepo`, any folder in it) or off again
/// (`removeRepo`, as listed or any folder in it). One change at a time, in this process and across
/// processes (a lock on `guards/global.lock`, waited for 5 seconds), so changes made at once all
/// apply. `added` is the [`Pin`] of the folder `addRepo` names, asked of the server before this
/// (`invalidInput` when `addRepo` comes without it: no entry is written without its pin); the entry
/// is written with it, or its pin replaced when the repository was on already. `protocol` while the
/// file saved now cannot be read (nothing is written over it) or while another process keeps the
/// lock, `invalidInput` as [`write_global`].
pub fn update_global(data_dir: &Path, patch: &MemoryGuardsPatch, added: Option<Pin>) -> Result<MemoryGuards> {
    let _writing = GlobalLock::take(data_dir)?;

    change::apply(data_dir, patch, added).map(with_dead_repos)
}

/// The project key a repository had when the user turned it on, as Harness Memory computes it
/// ([`project_key`](super::memory_project_key)): `None` for a folder with no project, such as the
/// home folder. Saving there stops when the repository's key becomes another one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin(pub Option<String>);

/// The guards of chat `key` in `data_dir`; `None` while its file is missing. `invalidInput` for a
/// key that is not one, `protocol` for a file that cannot be read.
pub fn read_chat(data_dir: &Path, key: &str) -> Result<Option<MemoryChatGuards>> {
    Ok(read_chat_file(data_dir, key)?.map(|file| MemoryChatGuards {
        paused: file.paused,
        notes: file.notes,
        scripts: file.scripts,
    }))
}

/// The project root chat `key`'s file in `data_dir` names; `None` while it is missing or names none.
/// As [`read_chat`] otherwise.
pub fn read_chat_repo(data_dir: &Path, key: &str) -> Result<Option<String>> {
    Ok(read_chat_file(data_dir, key)?.and_then(|file| file.repo_root))
}

fn read_chat_file(data_dir: &Path, key: &str) -> Result<Option<ChatFile>> {
    check_chat_key(key)?;

    let Some(text) = read_file(&chat_path(data_dir, key))? else {
        return Ok(None);
    };

    match serde_json::from_str::<ChatFile>(&text) {
        Ok(file) if file.version == VERSION => Ok(Some(file)),
        _ => Err(unreadable()),
    }
}

/// Writes the guards of chat `key` to `data_dir`, with `repo_root`, the root of its project
/// ([`chat_repo_root`]). `invalidInput` for a key that is not one.
pub fn write_chat(data_dir: &Path, key: &str, guards: &MemoryChatGuards, repo_root: Option<&str>) -> Result<()> {
    check_chat_key(key)?;

    let chats = ready_guards_dir(data_dir)?.join(CHATS_DIR);
    ready_folder(&chats)?;

    let file = ChatFile {
        version: VERSION,
        paused: guards.paused,
        notes: guards.notes,
        scripts: guards.scripts,
        repo_root: repo_root.map(str::to_owned),
    };

    write_file(&chats.join(format!("{key}.json")), &file)
}

/// The repository of a chat in `project`, as its chat file names it: the project root the hooks
/// resolve ([`project_identity`]), its git root or, outside a repository, the folder itself (for a
/// worktree its own root: the server maps it to the main repository, as [`saving_root`] does).
/// `None` for a folder that is gone or a path that is not text.
pub fn chat_repo_root(project: &Path) -> Option<String> {
    let root = project_identity(project).ok()?;

    root.into_os_string().into_string().ok()
}

fn global_path(data_dir: &Path) -> PathBuf {
    data_dir.join(GUARDS_DIR).join(GLOBAL_FILE)
}

fn chat_path(data_dir: &Path, key: &str) -> PathBuf {
    data_dir.join(GUARDS_DIR).join(CHATS_DIR).join(format!("{key}.json"))
}

fn unreadable() -> BusinessError {
    BusinessError::Protocol("the Harness Memory guards file cannot be read".to_owned())
}

/// The text of `path`; `None` while it is missing. `protocol` for a symbolic link or a file that
/// cannot be read.
fn read_file(path: &Path) -> Result<Option<String>> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(unreadable()),
        Ok(metadata) if !metadata.is_file() => return Err(unreadable()),
        Ok(_) => {}
    }

    SafeReader::read_to_string(path, MAX_FILE_BYTES)
        .map(Some)
        .map_err(|_| unreadable())
}

/// `<data_dir>/guards`, created when missing, the data folder too (both `0700`).
fn ready_guards_dir(data_dir: &Path) -> Result<PathBuf> {
    refuse_link(data_dir)?;

    if !data_dir.exists() {
        create_private_dir(data_dir, "creating the Harness Memory data folder")?;
    }

    check_owner(data_dir)?;

    let guards = data_dir.join(GUARDS_DIR);
    ready_folder(&guards)?;

    Ok(guards)
}

/// Creates `dir` (`0700`) or narrows it to `0700`; refuses a symbolic link or a folder of another
/// user.
fn ready_folder(dir: &Path) -> Result<()> {
    refuse_link(dir)?;
    ensure_private_dir(dir, "creating the Harness Memory guards folder")?;
    check_owner(dir)
}

/// `forbidden` for a folder that belongs to another user, `invalidInput` naming the fix for one
/// that other users can write to (Unix).
#[cfg(unix)]
fn check_owner(dir: &Path) -> Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let metadata = std::fs::symlink_metadata(dir)
        .map_err(|error| BusinessError::from_io("reading the Harness Memory data folder", error))?;

    if !metadata.is_dir() {
        return Err(BusinessError::InvalidInput(format!(
            "{} is not a folder",
            dir.display()
        )));
    }

    if metadata.uid() != nix::unistd::geteuid().as_raw() {
        return Err(BusinessError::Forbidden(format!(
            "{} belongs to another user",
            dir.display()
        )));
    }

    if metadata.permissions().mode() & 0o022 != 0 {
        return Err(open_folder(dir));
    }

    Ok(())
}

/// The error for folder `dir` that other users can write to: `invalidInput` (its message reaches
/// the user, a `forbidden` one would not) naming the folder and the command that fixes it.
#[cfg(unix)]
fn open_folder(dir: &Path) -> BusinessError {
    let shown = dir.display().to_string();

    BusinessError::InvalidInput(format!(
        "other users can write to the Harness Memory folder {shown}. Run chmod 700 {} and try again",
        shell_quoted(&shown)
    ))
}

/// `text` as one word of a POSIX shell command line: as it is when it holds only safe characters,
/// else in single quotes.
#[cfg(unix)]
fn shell_quoted(text: &str) -> String {
    let safe = |c: char| c.is_ascii_alphanumeric() || "/._-+@%:,~".contains(c);

    if !text.is_empty() && text.chars().all(safe) {
        return text.to_owned();
    }

    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(not(unix))]
fn check_owner(_dir: &Path) -> Result<()> {
    Ok(())
}

/// `value` as the JSON a guards file holds.
fn encoded<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| BusinessError::Internal(format!("encoding the memory guards failed: {error}")))?;
    bytes.push(b'\n');

    Ok(bytes)
}

/// Writes `value` as JSON to `path` in one rename (`0600`), never through a symbolic link.
fn write_file<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    write_bytes(path, &encoded(value)?)
}

/// Writes `bytes` to `path` in one rename (`0600`), never through a symbolic link.
fn write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    refuse_link(path)?;

    replace_atomically(path, bytes, "saving the Harness Memory guards")
}

mod change;
mod changed;
mod chats;
mod global;
mod lock;
mod saving_root;

pub use changed::{checkable_repos, with_changed_repos};
pub use chats::{CHAT_MAX_AGE, ChatGuardWrites, sweep_chats};
pub use global::{
    OPT_IN_MARKER, ensure_global, guards_data_dir, read_global_healed, rules_missing, version_one_remains,
};
pub use saving_root::{repo_is_on, saving_root, saving_root_listed, saving_root_of_folder};

use lock::GlobalLock;

#[cfg(test)]
mod tests;
