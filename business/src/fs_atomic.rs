//! Pieces of an atomic file replacement (write a temporary sibling, flush it, rename it over the
//! target), shared by the prefs store, the MCP registration and the desktop shell's file saves.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{BusinessError, Result};

/// Replaces `target` with `bytes` in one rename: a new temporary sibling (`0600` on Unix), flushed
/// to disk first and removed again on any failure, so a failure never leaves a half-written file at
/// `target`. `context` names the rename in the error.
pub fn replace_atomically(target: &Path, bytes: &[u8], context: &str) -> Result<()> {
    let temp = temp_sibling(target);

    let written = write_synced(&temp, bytes)
        .and_then(|()| fs::rename(&temp, target).map_err(|error| BusinessError::from_io(context, error)));

    if written.is_err() {
        fs::remove_file(&temp).ok();
    }

    written
}

/// [`replace_atomically`], with `still_current` checked right before the rename (after the new
/// bytes are on disk): when it fails, nothing is renamed, the temporary file is removed and its
/// error is returned. It narrows the window in which a read-check-write loses a change made
/// meanwhile, but does not close it: a change between the check and the rename is replaced (no
/// compare-and-swap across processes), so callers must fail closed on their own.
pub(crate) fn replace_atomically_if(
    target: &Path,
    bytes: &[u8],
    context: &str,
    still_current: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let temp = temp_sibling(target);

    let written = write_synced(&temp, bytes)
        .and_then(|()| still_current())
        .and_then(|()| fs::rename(&temp, target).map_err(|error| BusinessError::from_io(context, error)));

    if written.is_err() {
        fs::remove_file(&temp).ok();
    }

    written
}

/// A fresh temporary path next to `target` (`.<name>.tmp-<pid>-<nanos>`), so the final rename
/// stays on one filesystem.
pub(crate) fn temp_sibling(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temp = format!(".{name}.tmp-{}-{}", std::process::id(), unix_nanos());

    match target.parent() {
        Some(parent) => parent.join(temp),
        None => PathBuf::from(temp),
    }
}

/// Creates `path` (it must not exist; mode `0600` on Unix), writes `bytes` and flushes them to disk.
pub(crate) fn write_synced(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);

    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);

    let mut file = options
        .open(path)
        .map_err(|error| BusinessError::from_io("creating a temporary file", error))?;

    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| BusinessError::from_io("writing a temporary file", error))
}

/// Creates `dir` and its missing parents (`0700` on Unix); an existing folder is fine. `context`
/// names the creation in the error.
pub(crate) fn create_private_dir(dir: &Path, context: &str) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);

    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);

    builder
        .create(dir)
        .map_err(|error| BusinessError::from_io(context, error))
}

/// Nanoseconds since the Unix epoch (0 when the clock is before it).
pub(crate) fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos())
}

/// Refuses (`invalidInput`) a symbolic link at `path`, so nothing is written through one; a
/// missing path is fine.
pub(crate) fn refuse_link(path: &Path) -> Result<()> {
    let is_link = std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink());

    if is_link {
        return Err(BusinessError::InvalidInput(format!(
            "{} is a symbolic link; the app does not write through it",
            path.file_name().map(|name| name.to_string_lossy()).unwrap_or_default()
        )));
    }

    Ok(())
}
