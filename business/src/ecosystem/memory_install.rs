//! The app's stable copy of the bundled `harness-memory` server (`install_memory_binary`).
//!
//! The harnesses' MCP entries name this copy by absolute path, so it must outlive app updates and
//! moves of the app bundle. It is replaced only when its bytes differ from the bundled server's:
//! a fresh file is written next to it (read and written, never `fs::copy`, so no macOS extended
//! attribute such as the quarantine flag comes along), made executable, flushed and renamed over it.
//! On Windows a running copy cannot be replaced, so it is first renamed aside
//! (`harness-memory.old-<nanos>.exe`); such leftovers are removed on the next install on Windows
//! (elsewhere nothing is renamed aside, so nothing else in the folder is touched).

use std::fs;
use std::path::{Path, PathBuf};

use super::catalog::MEMORY_BINARY;
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{temp_sibling, unix_nanos, write_synced};

/// The file name of the server on this OS.
pub fn memory_binary_name() -> String {
    format!("{MEMORY_BINARY}{}", std::env::consts::EXE_SUFFIX)
}

/// Where the stable copy is and whether this install replaced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledBinary {
    pub path: PathBuf,
    /// False when the copy already held the same bytes (nothing was written).
    pub changed: bool,
}

/// Installs `source` as `dir/harness-memory[.exe]` unless that file already holds the same bytes
/// (see the module docs); `dir` is created when missing.
///
/// Errors: `NotFound` when `source` is missing, `InvalidInput` when it is not a regular file or the
/// destination is a symlink or anything but a regular file, `Io` when a write fails (the old copy
/// is then left as it was).
pub fn install_memory_binary(source: &Path, dir: &Path) -> Result<InstalledBinary> {
    let bytes = read_source(source)?;
    let path = dir.join(memory_binary_name());

    fs::create_dir_all(dir).map_err(|error| BusinessError::from_io("creating the memory server's folder", error))?;

    if cfg!(windows) {
        remove_leftovers(dir);
    }

    if same_bytes(&path, &bytes)? {
        return Ok(InstalledBinary { path, changed: false });
    }

    let temp = temp_sibling(&path);
    let replaced = write_executable(&temp, &bytes).and_then(|()| rename_over(&temp, &path));

    if replaced.is_err() {
        fs::remove_file(&temp).ok();
    }

    replaced.map(|()| InstalledBinary { path, changed: true })
}

fn read_source(source: &Path) -> Result<Vec<u8>> {
    let metadata = match fs::metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(BusinessError::NotFound("the bundled harness-memory server".to_owned()));
        }
        Err(error) => {
            return Err(BusinessError::from_io(
                "reading the bundled harness-memory server",
                error,
            ));
        }
    };

    if !metadata.is_file() {
        return Err(BusinessError::InvalidInput(
            "the bundled harness-memory server is not a file".to_owned(),
        ));
    }

    fs::read(source).map_err(|error| BusinessError::from_io("reading the bundled harness-memory server", error))
}

/// Whether `path` is a regular file holding `bytes`; `InvalidInput` for a symlink or anything but a
/// regular file.
fn same_bytes(path: &Path, bytes: &[u8]) -> Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(BusinessError::from_io(
                "checking the installed harness-memory server",
                error,
            ));
        }
    };

    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BusinessError::InvalidInput(
            "the installed harness-memory server is not a regular file".to_owned(),
        ));
    }

    if metadata.len() != bytes.len() as u64 {
        return Ok(false);
    }

    // An unreadable copy is replaced like a different one.
    Ok(fs::read(path).is_ok_and(|installed| installed == bytes))
}

fn write_executable(temp: &Path, bytes: &[u8]) -> Result<()> {
    write_synced(temp, bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(temp, fs::Permissions::from_mode(0o755))
            .map_err(|error| BusinessError::from_io("making the memory server executable", error))?;
    }

    Ok(())
}

fn rename_over(temp: &Path, path: &Path) -> Result<()> {
    let Err(error) = fs::rename(temp, path) else {
        return Ok(());
    };

    // Windows refuses to replace a running executable but lets it be renamed.
    if cfg!(windows) && path.exists() {
        let aside = path.with_file_name(format!("{MEMORY_BINARY}.old-{}.exe", unix_nanos()));

        if fs::rename(path, &aside).is_ok() {
            return move_in_or_restore(temp, path, &aside)
                .map_err(|error| BusinessError::from_io("installing the memory server", error));
        }
    }

    Err(BusinessError::from_io("installing the memory server", error))
}

/// Renames `temp` to `path`, whose old copy was renamed to `aside`; when that fails, the old copy is
/// put back (best effort) so the harnesses' entries still find a server.
fn move_in_or_restore(temp: &Path, path: &Path, aside: &Path) -> std::io::Result<()> {
    let Err(error) = fs::rename(temp, path) else {
        return Ok(());
    };

    if let Err(restore) = fs::rename(aside, path) {
        tracing::warn!(error = %restore, "cannot put the previous memory server back");
    }

    Err(error)
}

/// Removes copies renamed aside by earlier installs on Windows (`harness-memory.old-*.exe`); one
/// still running stays until the next time.
fn remove_leftovers(dir: &Path) {
    let prefix = format!("{MEMORY_BINARY}.old-");
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let is_leftover = name.starts_with(&prefix) && name.ends_with(".exe");

        if is_leftover && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            fs::remove_file(entry.path()).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_move_in_puts_the_old_copy_back() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path();

        let path = dir.join("harness-memory");
        let aside = dir.join("harness-memory.old-1");
        fs::write(&aside, "old copy").unwrap();

        let moved = move_in_or_restore(&dir.join("missing-temp"), &path, &aside);

        assert!(moved.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "old copy");
        assert!(!aside.exists());
    }
}
