//! Reading one registry file of the orphan sweep ([`super::sweep_orphans`]): opened without
//! following a link and checked on what was opened.

use std::fs;
use std::io::Read;
use std::path::Path;

use super::{MAX_FILE_BYTES, MAX_RECORDED_CHILDREN, RegistryFile, VERSION, private_file};

/// What reading one registry file gave.
pub(super) enum ReadRecord {
    Record(RegistryFile),
    /// Another user's file, or one group or others may open: left alone, unread.
    Untrusted,
    /// Not a regular file (a link included), too large or not a registry file of [`VERSION`].
    Corrupt,
    /// Could not be opened or read this time (too many open files, an I/O error): kept for the
    /// next start, never removed.
    Unavailable,
}

/// True when opening failed because `path` is a link (`O_NOFOLLOW` gives `ELOOP`).
fn opened_a_link(error: &std::io::Error) -> bool {
    #[cfg(unix)]
    return error.raw_os_error() == Some(nix::libc::ELOOP);

    #[cfg(not(unix))]
    return error.kind() == std::io::ErrorKind::InvalidInput;
}

/// The registry file at `path` (see [`ReadRecord`]), opened without following a link (on Unix)
/// and checked on what was opened, so a file swapped in after a check by path is never read.
pub(super) fn read_record(path: &Path) -> ReadRecord {
    let (file, metadata) = match open_record(path) {
        Ok(opened) => opened,
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return ReadRecord::Untrusted,
        Err(error) if opened_a_link(&error) => return ReadRecord::Corrupt,
        Err(_) => return ReadRecord::Unavailable,
    };

    if !metadata.is_file() {
        return ReadRecord::Corrupt;
    }

    if !private_file(&metadata) {
        return ReadRecord::Untrusted;
    }

    if metadata.len() > MAX_FILE_BYTES {
        return ReadRecord::Corrupt;
    }

    let mut bytes = Vec::new();

    if file.take(MAX_FILE_BYTES).read_to_end(&mut bytes).is_err() {
        return ReadRecord::Unavailable;
    }

    let record = serde_json::from_slice::<RegistryFile>(&bytes).ok();
    let valid = |record: &RegistryFile| record.version == VERSION && record.children.len() <= MAX_RECORDED_CHILDREN;

    record.filter(valid).map_or(ReadRecord::Corrupt, ReadRecord::Record)
}

/// Opens `path` for reading, with the metadata of what was opened: on Unix never through a link,
/// nor waiting for a writer when it is a FIFO.
fn open_record(path: &Path) -> std::io::Result<(fs::File, fs::Metadata)> {
    let mut options = fs::OpenOptions::new();

    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::custom_flags(
        &mut options,
        (nix::fcntl::OFlag::O_NOFOLLOW | nix::fcntl::OFlag::O_NONBLOCK).bits(),
    );

    let file = options.read(true).open(path)?;

    file.metadata().map(|metadata| (file, metadata))
}
