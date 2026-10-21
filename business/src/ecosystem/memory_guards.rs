//! The save guards Harness Memory applies to agents, kept in its data folder
//! ([`memory_data_dir`](super::memory_data_dir)) and written by this app only:
//!
//! - `guards/global.json` ([`GLOBAL_FILE`]): `{"version": 1, "notes": {"paused", "offRepos"},
//!   "scripts": {...}}`, the guards of every chat (`memory_guards_get`, `memory_guards_update`,
//!   one change at a time under a lock: [`update_global`]);
//! - `guards/chats/<key>.json` ([`CHATS_DIR`]): `{"version": 1, "paused", "notes": {"on", "ask"},
//!   "scripts": {...}, "repoRoot"}`, the guards of one chat and the root of its project (its git
//!   root, else the folder: [`chat_repo_root`]), written when the chat starts, on every toggle
//!   (`memory_chat_guards_set`, newest revision only: [`ChatGuardWrites`]) and when it moves. The chat's harness gets the key
//!   as [`SESSION_VAR`] only once its file is written. Files untouched for [`CHAT_MAX_AGE`] go at
//!   startup ([`sweep_chats`]).
//!
//! The server reads them on every gated call: a missing file is no guards, an unreadable one
//! refuses. Folders are `0700` and this user's, files `0600`, written in one rename, never through
//! a symbolic link; a file of another version is unreadable. The app creates the data folder when
//! it is missing (`0700`), as the server does.
//!
//! The hooks of one chat read both through [`read_memory_guards`](crate::hooks::read_memory_guards).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};

use crate::error::{BusinessError, Result};
use crate::fs_atomic::{create_private_dir, ensure_private_dir, refuse_link, replace_atomically};
use crate::hooks::project_identity;
use crate::model::{GlobalKindGuard, KindGuard, MemoryChatGuards, MemoryGuardKind, MemoryGuards, MemoryGuardsPatch};
use crate::security::SafeReader;

/// The variable that names a chat to Harness Memory (its key).
pub const SESSION_VAR: &str = "HARNESS_MEMORY_SESSION";

/// The guards folder in the data folder.
pub const GUARDS_DIR: &str = "guards";

/// The guards of every chat, in [`GUARDS_DIR`].
pub const GLOBAL_FILE: &str = "global.json";

/// The folder of each chat's guards, in [`GUARDS_DIR`].
pub const CHATS_DIR: &str = "chats";

/// The file format written and read.
const VERSION: u32 = 1;

/// The largest guards file read (256 KiB: far more than [`MAX_OFF_REPOS`] paths).
const MAX_FILE_BYTES: u64 = 256 * 1024;

/// The most repositories turned off per kind.
pub const MAX_OFF_REPOS: usize = 256;

/// The longest repository path kept, in characters.
const MAX_REPO_CHARS: usize = 4096;

/// One change of `guards/global.json` at a time in this process, so two changes made at once both
/// apply ([`update_global`]).
static GLOBAL_WRITES: Mutex<()> = Mutex::new(());

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

/// `guards/global.json` as written.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GlobalFile {
    version: u32,
    notes: GlobalKindGuard,
    scripts: GlobalKindGuard,
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

/// The guards of every chat in `data_dir`: none while the file is missing. `protocol` for a file
/// that cannot be read (a symbolic link, too large, not this version), which the server takes as
/// "refuse every save".
pub fn read_global(data_dir: &Path) -> Result<MemoryGuards> {
    let Some(text) = read_file(&global_path(data_dir))? else {
        return Ok(MemoryGuards::default());
    };

    match serde_json::from_str::<GlobalFile>(&text) {
        Ok(file) if file.version == VERSION => Ok(MemoryGuards {
            notes: file.notes,
            scripts: file.scripts,
        }),
        _ => Err(unreadable()),
    }
}

/// Writes the guards of every chat to `data_dir` and returns them as written: each repository as
/// its project root (see [`project_identity`]; a folder that is gone stays as given), without
/// duplicates, sorted. `invalidInput` for a repository that is not an absolute path, too long, or
/// more than [`MAX_OFF_REPOS`] of them.
pub(crate) fn write_global(data_dir: &Path, guards: &MemoryGuards) -> Result<MemoryGuards> {
    let guards = MemoryGuards {
        notes: canonical_kind(&guards.notes)?,
        scripts: canonical_kind(&guards.scripts)?,
    };

    let file = GlobalFile {
        version: VERSION,
        notes: guards.notes.clone(),
        scripts: guards.scripts.clone(),
    };

    let guards_dir = ready_guards_dir(data_dir)?;
    write_file(&guards_dir.join(GLOBAL_FILE), &file)?;

    tracing::info!(
        notes_paused = guards.notes.paused,
        notes_off = guards.notes.off_repos.len(),
        scripts_paused = guards.scripts.paused,
        scripts_off = guards.scripts.off_repos.len(),
        "memory save guards saved"
    );

    Ok(guards)
}

/// Applies `patch` to the guards of every chat saved in `data_dir` now and returns them as written
/// (see [`write_global`]): pause or resume the kind, turn a repository off (`addRepo`, any folder
/// in it) or on again (`removeRepo`, as listed or any folder in it). One change at a time in this
/// process, so changes made at once all apply. `protocol` while the file saved now cannot be read
/// (nothing is written over it), `invalidInput` as [`write_global`].
pub fn update_global(data_dir: &Path, patch: &MemoryGuardsPatch) -> Result<MemoryGuards> {
    let _writing = GLOBAL_WRITES.lock().unwrap_or_else(PoisonError::into_inner);

    let mut guards = read_global(data_dir)?;
    let guard = match patch.kind {
        MemoryGuardKind::Notes => &mut guards.notes,
        MemoryGuardKind::Scripts => &mut guards.scripts,
    };

    if let Some(paused) = patch.paused {
        guard.paused = paused;
    }

    if let Some(repo) = &patch.remove_repo {
        let root = canonical_repo(repo)?;
        guard.off_repos.retain(|off| off != repo && *off != root);
    }

    if let Some(repo) = &patch.add_repo {
        guard.off_repos.push(canonical_repo(repo)?);
    }

    write_global(data_dir, &guards)
}

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
/// resolve ([`project_identity`]), its git root or, outside a repository, the folder itself, the
/// same string `addRepo` turns off. `None` for a folder that is gone or a path that is not text.
pub fn chat_repo_root(project: &Path) -> Option<String> {
    let root = project_identity(project).ok()?;

    root.into_os_string().into_string().ok()
}

/// `guard` with its repositories as written (see [`write_global`]).
fn canonical_kind(guard: &GlobalKindGuard) -> Result<GlobalKindGuard> {
    if guard.off_repos.len() > MAX_OFF_REPOS {
        return Err(BusinessError::InvalidInput(format!(
            "at most {MAX_OFF_REPOS} repositories can be turned off"
        )));
    }

    let mut repos = Vec::new();

    for repo in &guard.off_repos {
        let repo = canonical_repo(repo)?;

        if !repos.contains(&repo) {
            repos.push(repo);
        }
    }

    repos.sort();

    Ok(GlobalKindGuard {
        paused: guard.paused,
        off_repos: repos,
    })
}

/// `repo` as its project root; as given when the folder is gone.
fn canonical_repo(repo: &str) -> Result<String> {
    let path = Path::new(repo);

    if !path.is_absolute() || repo.chars().count() > MAX_REPO_CHARS || repo.chars().any(char::is_control) {
        return Err(BusinessError::InvalidInput(
            "a repository is an absolute folder path".to_owned(),
        ));
    }

    let root = match project_identity(path) {
        Ok(root) => root,
        Err(BusinessError::NotFound(_)) => path.to_path_buf(),
        Err(error) => return Err(error),
    };

    root.into_os_string()
        .into_string()
        .map_err(|_| BusinessError::InvalidInput("a repository path is not valid text".to_owned()))
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

/// Writes `value` as JSON to `path` in one rename (`0600`), never through a symbolic link.
fn write_file<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    refuse_link(path)?;

    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| BusinessError::Internal(format!("encoding the memory guards failed: {error}")))?;
    bytes.push(b'\n');

    replace_atomically(path, &bytes, "saving the Harness Memory guards")
}

mod chats;

pub use chats::{CHAT_MAX_AGE, ChatGuardWrites, sweep_chats};

#[cfg(test)]
mod tests;
