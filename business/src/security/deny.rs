//! The never-read list and [`SafeReader`].
//!
//! **Every file read in the business layer goes through [`SafeReader`].** It refuses deny-listed
//! files (databases, logs, keys, credential stores, blob caches), checks the canonical path and
//! every symlink hop, refuses non-regular files and caps the size, so a credential store can never
//! be opened by mistake — not even through a symlink.

use std::fmt;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};

use crate::error::{BusinessError, Result};

/// Extensions (lowercase, without dot) that are never read.
const DENIED_EXTENSIONS: &[&str] = &[
    "db",
    "db-wal",
    "db-shm",
    "db-journal",
    "sqlite",
    "sqlite3",
    "log",
    "pem",
    "key",
    "p12",
    "pfx",
];

/// Exact file names (lowercase) that are never read.
const DENIED_NAMES: &[&str] = &[
    "auth.json",
    "mcp-auth.json",
    "install-id",
    "auth-token",
    "models-store.json",
    "gpu_cache.json",
    "credentials",
    "credentials.json",
    ".env",
];

/// File-name prefixes (lowercase) that are never read.
const DENIED_PREFIXES: &[&str] = &[".env.", "id_rsa", "id_ed25519"];

/// Directory names (lowercase) whose contents are never read.
const DENIED_DIRS: &[&str] = &["terminal-sessions", "url-search"];

/// Maximum symlink hops followed when checking link targets.
const MAX_LINK_HOPS: usize = 40;

/// Why a path is on the deny-list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyReason {
    /// The extension marks a database, log or key file.
    Extension(String),
    /// The file name is a known credential or private store.
    FileName(String),
    /// The path lies inside a private directory.
    Directory(String),
}

impl fmt::Display for DenyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Extension(extension) => write!(f, "*.{extension} files are never read"),
            Self::FileName(name) => write!(f, "{name} is never read"),
            Self::Directory(name) => write!(f, "files under {name}/ are never read"),
        }
    }
}

/// Checks `path` (as given, without touching the filesystem) against the deny-list.
/// Names are compared case-insensitively.
pub fn is_denied(path: &Path) -> Option<DenyReason> {
    let components: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect();

    let (file_name, parents) = components.split_last()?;

    if let Some(reason) = denied_file_name(file_name) {
        return Some(reason);
    }

    for (index, dir) in parents.iter().enumerate() {
        if DENIED_DIRS.contains(&dir.as_str()) {
            return Some(DenyReason::Directory(dir.clone()));
        }

        // Agent blob stores (`~/.omp/agent/blobs/`).
        if dir == "blobs" && parents[..index].iter().any(|ancestor| ancestor == "agent") {
            return Some(DenyReason::Directory(dir.clone()));
        }
    }

    None
}

fn denied_file_name(name: &str) -> Option<DenyReason> {
    if DENIED_NAMES.contains(&name) || DENIED_PREFIXES.iter().any(|prefix| name.starts_with(prefix)) {
        return Some(DenyReason::FileName(name.to_owned()));
    }

    let extension = Path::new(name).extension()?.to_str()?;
    DENIED_EXTENSIONS
        .contains(&extension)
        .then(|| DenyReason::Extension(extension.to_owned()))
}

/// Guarded file reads. See the module docs.
#[derive(Debug, Clone, Copy, Default)]
pub struct SafeReader;

impl SafeReader {
    /// Default size cap for [`SafeReader::read_to_string`] (2 MiB).
    pub const DEFAULT_MAX_BYTES: u64 = 2 * 1024 * 1024;

    /// Reads a whole UTF-8 file of at most `max_bytes` bytes.
    ///
    /// Errors: `Forbidden` for deny-listed paths (including via symlinks), `InvalidInput` for
    /// non-regular files, oversized files and invalid UTF-8, `NotFound` for missing files.
    pub fn read_to_string(path: &Path, max_bytes: u64) -> Result<String> {
        let (file, metadata) = open_checked(path)?;

        if metadata.len() > max_bytes {
            return Err(too_large(path, max_bytes));
        }

        let mut bytes = Vec::new();
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| BusinessError::from_io(format!("reading {}", path.display()), error))?;

        // The file may have grown between the size check and the read.
        if bytes.len() as u64 > max_bytes {
            return Err(too_large(path, max_bytes));
        }

        String::from_utf8(bytes)
            .map_err(|_| BusinessError::InvalidInput(format!("{} is not valid UTF-8", path.display())))
    }

    /// Reads up to `n` lines from the start of a file, splitting on `\n` only (Unicode line
    /// separators stay inside lines) and stripping one trailing `\r` per line.
    ///
    /// A line longer than `max_line_bytes` fails with `InvalidInput`; fewer than `n` lines are
    /// returned at end of file. Same access checks as [`SafeReader::read_to_string`].
    pub fn read_first_lines(path: &Path, n: usize, max_line_bytes: usize) -> Result<Vec<String>> {
        let (file, _) = open_checked(path)?;
        let mut reader = BufReader::new(file);
        let mut lines = Vec::with_capacity(n);
        let mut buffer = Vec::new();

        while lines.len() < n {
            buffer.clear();

            // Room for the content plus a `\r\n` terminator; anything longer is over the limit.
            let limit = max_line_bytes as u64 + 2;
            let read = (&mut reader)
                .take(limit)
                .read_until(b'\n', &mut buffer)
                .map_err(|error| BusinessError::from_io(format!("reading {}", path.display()), error))?;

            if read == 0 {
                break;
            }

            if buffer.last() == Some(&b'\n') {
                buffer.pop();
            }

            if buffer.last() == Some(&b'\r') {
                buffer.pop();
            }

            if buffer.len() > max_line_bytes {
                return Err(BusinessError::InvalidInput(format!(
                    "{} has a line longer than {max_line_bytes} bytes",
                    path.display()
                )));
            }

            let line = String::from_utf8(std::mem::take(&mut buffer))
                .map_err(|_| BusinessError::InvalidInput(format!("{} is not valid UTF-8", path.display())))?;
            lines.push(line);
        }

        Ok(lines)
    }
}

fn too_large(path: &Path, max_bytes: u64) -> BusinessError {
    BusinessError::InvalidInput(format!("{} is larger than {max_bytes} bytes", path.display()))
}

fn forbidden(path: &Path, reason: &DenyReason) -> BusinessError {
    BusinessError::Forbidden(format!("{}: {reason}", path.display()))
}

/// Applies every check and opens the canonical path for reading.
fn open_checked(path: &Path) -> Result<(File, Metadata)> {
    if let Some(reason) = is_denied(path) {
        return Err(forbidden(path, &reason));
    }

    check_link_chain(path)?;

    let canonical = dunce::canonicalize(path)
        .map_err(|error| BusinessError::from_io(format!("resolving {}", path.display()), error))?;

    if let Some(reason) = is_denied(&canonical) {
        return Err(forbidden(path, &reason));
    }

    // Checked before opening so a FIFO or device is never opened (opening a FIFO can block).
    let metadata = std::fs::metadata(&canonical).map_err(|error| inspect_error(path, error))?;
    ensure_regular(path, &metadata)?;

    let file =
        open_read(&canonical).map_err(|error| BusinessError::from_io(format!("opening {}", path.display()), error))?;

    // Re-checked on the open handle: the path may have been swapped after the first check.
    let metadata = file.metadata().map_err(|error| inspect_error(path, error))?;
    ensure_regular(path, &metadata)?;

    Ok((file, metadata))
}

fn inspect_error(path: &Path, error: std::io::Error) -> BusinessError {
    BusinessError::from_io(format!("inspecting {}", path.display()), error)
}

fn ensure_regular(path: &Path, metadata: &Metadata) -> Result<()> {
    if metadata.is_file() {
        return Ok(());
    }

    Err(BusinessError::InvalidInput(format!(
        "{} is not a regular file",
        path.display()
    )))
}

/// Follows a symlink chain hop by hop and refuses it when any hop names a denied file.
fn check_link_chain(path: &Path) -> Result<()> {
    let mut current = path.to_path_buf();

    for _ in 0..MAX_LINK_HOPS {
        let is_link = std::fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink());
        if !is_link {
            return Ok(());
        }

        let target = std::fs::read_link(&current)
            .map_err(|error| BusinessError::from_io(format!("resolving {}", path.display()), error))?;
        current = resolve_link_target(&current, target);

        if let Some(reason) = is_denied(&current) {
            return Err(forbidden(path, &reason));
        }
    }

    Err(BusinessError::InvalidInput(format!(
        "{} has too many symlink hops",
        path.display()
    )))
}

fn resolve_link_target(link: &Path, target: PathBuf) -> PathBuf {
    if target.is_absolute() {
        return target;
    }

    match link.parent() {
        Some(parent) => parent.join(target),
        None => target,
    }
}

#[cfg(unix)]
fn open_read(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;

    // Non-blocking open so a FIFO swapped in after the type check cannot hang the reader.
    OpenOptions::new()
        .read(true)
        .custom_flags(nix::fcntl::OFlag::O_NONBLOCK.bits())
        .open(path)
}

#[cfg(not(unix))]
fn open_read(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).open(path)
}
