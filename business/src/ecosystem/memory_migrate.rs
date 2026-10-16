//! Retires the stable copy of the bundled server that app builds up to 0.16 kept on macOS and
//! Windows (`<local data>/bin/harness-memory[.exe]`, [`legacy_memory_copy`]), now that the server
//! is installed on its own (`migrate_legacy_memory`).
//!
//! - Re-point: an owned entry (see [`super::memory_register`]; only in a harness's user MCP file)
//!   whose `command` is that copy is changed to run the installed server. Only once one is installed: until then every entry stays
//!   as it is and the copy keeps serving it.
//! - Remove: the copy is deleted once every checked MCP file was read and none of its servers runs
//!   it (a user's own entry keeps it). A symlink or anything but a regular file is never removed,
//!   nor a copy reached through a symlinked folder (`bin` or the data folder itself); failures are
//!   logged and the next launch tries again.
//! - Same file: when the copy's path leads to the installed server itself (a folder linked to the
//!   installer's), nothing is re-pointed or removed.
//!
//! On Linux the copy was `~/.local/bin/harness-memory`, where the installer puts the server now,
//! so there is nothing to retire there.

use std::fs;
use std::path::{Path, PathBuf};

use super::memory_register::{names_command, repoint_memory_mcp};
use crate::app_dirs::{legacy_memory_copy, local_data_dir};
use crate::env::Env;

/// What [`migrate_legacy_memory`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyMigration {
    /// The owned entries changed to run the installed server.
    pub repointed: usize,
    /// What became of the old copy.
    pub copy: LegacyCopy,
}

/// The fate of the old copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyCopy {
    /// There is none (on Linux, or it is gone).
    Absent,
    /// Kept: the server is not installed, so nothing was re-pointed.
    NotInstalled,
    /// Kept: a server entry still runs it, a file could not be read, or it is the installed server
    /// itself.
    InUse,
    /// Kept: it is a symlink, not a regular file, or in a symlinked folder.
    Refused,
    /// Kept: removing it failed (on Windows, while a harness still runs it).
    Failed,
    /// Deleted.
    Removed,
}

/// Re-points the owned entries in the user MCP files of `targets` (each with the files the harness
/// also reads, which are only checked) from the old copy to `server`, and deletes the copy when
/// nothing runs it any more (see the module docs). `server` is the resolved `harness-memory`;
/// `None` when it is not installed.
pub fn migrate_legacy_memory(env: &Env, targets: &[(PathBuf, Vec<PathBuf>)], server: Option<&Path>) -> LegacyMigration {
    let Some(legacy) = legacy_memory_copy(env) else {
        return kept(LegacyCopy::Absent);
    };

    let Some(server) = server.filter(|server| *server != legacy) else {
        return kept(LegacyCopy::NotInstalled);
    };

    if same_file(&legacy, server) {
        tracing::info!("the old memory server copy is the installed server; left alone");
        return kept(LegacyCopy::InUse);
    }

    let repointed = targets
        .iter()
        .filter(|(path, _)| {
            let changed = repoint_memory_mcp(path, &legacy, server) == Ok(true);

            if changed {
                tracing::info!(
                    file = %path.display(),
                    from = %legacy.display(),
                    to = %server.display(),
                    "project memory: the app's entry now runs the installed server"
                );
            }

            changed
        })
        .count();

    // A file that cannot be read may still name the copy, so it counts as a use.
    let in_use = targets
        .iter()
        .flat_map(|(path, also_read)| std::iter::once(path).chain(also_read))
        .any(|file| names_command(file, &legacy).unwrap_or(true));

    let copy = if in_use {
        LegacyCopy::InUse
    } else {
        remove_copy(env, &legacy, server)
    };

    LegacyMigration { repointed, copy }
}

fn kept(copy: LegacyCopy) -> LegacyMigration {
    LegacyMigration { repointed: 0, copy }
}

/// Deletes the regular file at `path`, then its folder when that is left empty. Nothing is deleted
/// when a folder on the way from `<local data>` is a symlink, or when `path` is `server`'s file.
fn remove_copy(env: &Env, path: &Path, server: &Path) -> LegacyCopy {
    if linked_folder(env, path) {
        tracing::warn!("the old memory server copy is in a symlinked folder; left alone");
        return LegacyCopy::Refused;
    }

    if same_file(path, server) {
        tracing::info!("the old memory server copy is the installed server; left alone");
        return LegacyCopy::InUse;
    }

    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return LegacyCopy::Absent,
        Err(error) => {
            tracing::warn!(%error, "cannot check the old memory server copy");
            return LegacyCopy::Failed;
        }
    };

    if metadata.file_type().is_symlink() || !metadata.is_file() {
        tracing::warn!("the old memory server copy is not a regular file; left alone");
        return LegacyCopy::Refused;
    }

    if let Err(error) = fs::remove_file(path) {
        tracing::warn!(%error, "cannot remove the old memory server copy");
        return LegacyCopy::Failed;
    }

    // Only succeeds when nothing else is in the app's `bin` folder.
    if let Some(dir) = path.parent() {
        fs::remove_dir(dir).ok();
    }

    LegacyCopy::Removed
}

/// Whether a folder between `<local data>` (included) and `path` is a symlink, or cannot be checked.
fn linked_folder(env: &Env, path: &Path) -> bool {
    let Some(root) = local_data_dir(env) else {
        return true;
    };

    path.ancestors()
        .skip(1)
        .take_while(|folder| folder.starts_with(&root))
        .any(|folder| match fs::symlink_metadata(folder) {
            Ok(metadata) => metadata.file_type().is_symlink(),
            Err(error) => error.kind() != std::io::ErrorKind::NotFound,
        })
}

/// Whether `a` and `b` lead to the same existing file (through any symlink on the way).
#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

/// Whether `a` and `b` lead to the same existing file (through any symlink or junction on the way).
#[cfg(not(unix))]
fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
