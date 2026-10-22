//! [`SafeReader`]: the guarded reads of the deny module (see its docs), and the one opt-in read of
//! Claude Code's global file ([`SafeReader::read_claude_global_file`]).

use std::fs::{File, Metadata, OpenOptions};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::{CLAUDE_GLOBAL_FILE, DenyReason, MAX_LINK_HOPS, is_denied, is_denied_under_private_root};
use crate::error::{BusinessError, Result};

/// The global files of Claude Code ([`crate::harnesses::claude::global_config_file`] registers each
/// path it returns). Paths are only ever added.
static CLAUDE_GLOBAL_FILES: RwLock<Vec<PathBuf>> = RwLock::new(Vec::new());

/// Registers `path` as a Claude Code global file, the only file
/// [`SafeReader::read_claude_global_file`] opens. Registering it again changes nothing.
pub(crate) fn add_claude_global_file(path: &Path) {
    let mut registered = CLAUDE_GLOBAL_FILES
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    if !registered.iter().any(|known| known == path) {
        registered.push(path.to_path_buf());
    }
}

/// True when `canonical` is the canonical path of a registered global file.
fn is_claude_global_file(canonical: &Path) -> bool {
    let registered = CLAUDE_GLOBAL_FILES
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    registered
        .iter()
        .any(|known| dunce::canonicalize(known).is_ok_and(|known| known == canonical))
}

/// Which deny reasons a read lifts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Lift<'a> {
    /// None: every deny reason refuses.
    Nothing,
    /// The name of Claude Code's global file, for [`SafeReader::read_claude_global_file`] only.
    ClaudeGlobalName,
    /// The rules that match an ancestor's name, for the part of a path above this canonical project
    /// folder ([`SafeReader::open_in_project`]): below it the project-relative path meets every
    /// rule and the full path the private names of a Claude Code config directory or a Codex home.
    AboveProject(&'a Path),
}

impl Lift<'_> {
    fn denies(self, path: &Path) -> Option<DenyReason> {
        if let Self::AboveProject(project) = self
            && let Ok(relative) = path.strip_prefix(project)
        {
            return is_denied(relative).or_else(|| is_denied_under_private_root(path));
        }

        let reason = is_denied(path)?;
        let lifted = self == Self::ClaudeGlobalName && reason == DenyReason::FileName(CLAUDE_GLOBAL_FILE.to_owned());

        (!lifted).then_some(reason)
    }
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
        let (file, metadata) = open_checked(path, Lift::Nothing)?;

        read_opened(path, file, &metadata, max_bytes)
    }

    /// Reads Claude Code's global file (`~/.claude.json`, `$CLAUDE_CONFIG_DIR/.claude.json`), whose
    /// name every other read refuses: it holds the account, device ids and every project's state.
    /// Only its allow-listed readers call this. `path` must resolve to a file registered as a global
    /// file (see [`add_claude_global_file`]); every other check of
    /// [`SafeReader::read_to_string`] still applies, and the backup copies stay refused.
    ///
    /// The opened file must still be the registered one (the same device and inode). Claude Code
    /// replaces the file while it runs, so a file replaced during the open is opened once more.
    ///
    /// Errors: as [`SafeReader::read_to_string`], and `Forbidden` when `path` resolves to anything
    /// but a registered global file.
    pub fn read_claude_global_file(path: &Path, max_bytes: u64) -> Result<String> {
        let mut attempts = 2;

        loop {
            attempts -= 1;

            let canonical = dunce::canonicalize(path)
                .map_err(|error| BusinessError::from_io(format!("resolving {}", path.display()), error))?;

            if !is_claude_global_file(&canonical) {
                return Err(BusinessError::Forbidden(format!(
                    "{} is not Claude Code's global file",
                    path.display()
                )));
            }

            let (file, metadata) = open_checked(path, Lift::ClaudeGlobalName)?;

            // Re-checked on the open handle: the path may have been swapped after the first check.
            if is_same_file(&canonical, &metadata) {
                return read_opened(path, file, &metadata, max_bytes);
            }

            if attempts == 0 {
                return Err(BusinessError::Forbidden(format!(
                    "{} changed while it was opened",
                    path.display()
                )));
            }
        }
    }

    /// Opens a file for a bounded streaming read (session transcripts, untracked files). The
    /// caller caps how much it reads. Same access checks as [`SafeReader::read_to_string`].
    pub fn open(path: &Path) -> Result<File> {
        open_checked(path, Lift::Nothing).map(|(file, _)| file)
    }

    /// Opens a file of the project folder `project` (a repository's working tree) like
    /// [`SafeReader::open`], except that the folders above the project do not count: a project
    /// cloned below a folder named `projects` or `opencode` is read like any other. Below the
    /// project the relative path meets the whole list, a Claude Code config directory inside it
    /// keeps its private files, and a symlink hop that leaves the project meets the whole list.
    pub fn open_in_project(project: &Path, path: &Path) -> Result<File> {
        let project = dunce::canonicalize(project)
            .map_err(|error| BusinessError::from_io(format!("resolving {}", project.display()), error))?;

        open_checked(path, Lift::AboveProject(&project)).map(|(file, _)| file)
    }

    /// Reads up to `n` lines from the start of a file, splitting on `\n` only (Unicode line
    /// separators stay inside lines) and stripping one trailing `\r` per line.
    ///
    /// A line longer than `max_line_bytes` fails with `InvalidInput`; fewer than `n` lines are
    /// returned at end of file. Same access checks as [`SafeReader::read_to_string`].
    pub fn read_first_lines(path: &Path, n: usize, max_line_bytes: usize) -> Result<Vec<String>> {
        let (file, _) = open_checked(path, Lift::Nothing)?;
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

/// The whole UTF-8 content of the opened `file`, at most `max_bytes` bytes.
fn read_opened(path: &Path, file: File, metadata: &Metadata, max_bytes: u64) -> Result<String> {
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

    String::from_utf8(bytes).map_err(|_| BusinessError::InvalidInput(format!("{} is not valid UTF-8", path.display())))
}

fn too_large(path: &Path, max_bytes: u64) -> BusinessError {
    BusinessError::InvalidInput(format!("{} is larger than {max_bytes} bytes", path.display()))
}

fn forbidden(path: &Path, reason: &DenyReason) -> BusinessError {
    BusinessError::Forbidden(format!("{}: {reason}", path.display()))
}

/// Applies every check (but the deny reasons `lift` lifts) and opens the canonical path for reading.
fn open_checked(path: &Path, lift: Lift<'_>) -> Result<(File, Metadata)> {
    if let Some(reason) = lift.denies(path) {
        return Err(forbidden(path, &reason));
    }

    check_link_chain(path, lift)?;

    let canonical = dunce::canonicalize(path)
        .map_err(|error| BusinessError::from_io(format!("resolving {}", path.display()), error))?;

    if let Some(reason) = lift.denies(&canonical) {
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

/// True when the open handle's `opened` metadata is the file at `canonical` now: the same device
/// and inode on Unix, the same canonical path elsewhere.
#[cfg(unix)]
fn is_same_file(canonical: &Path, opened: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    std::fs::metadata(canonical).is_ok_and(|now| now.dev() == opened.dev() && now.ino() == opened.ino())
        && dunce::canonicalize(canonical).is_ok_and(|now| now == canonical)
}

#[cfg(not(unix))]
fn is_same_file(canonical: &Path, _opened: &Metadata) -> bool {
    dunce::canonicalize(canonical).is_ok_and(|now| now == canonical)
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
fn check_link_chain(path: &Path, lift: Lift<'_>) -> Result<()> {
    let mut current = path.to_path_buf();

    for _ in 0..MAX_LINK_HOPS {
        let is_link = std::fs::symlink_metadata(&current).is_ok_and(|metadata| metadata.file_type().is_symlink());
        if !is_link {
            return Ok(());
        }

        let target = std::fs::read_link(&current)
            .map_err(|error| BusinessError::from_io(format!("resolving {}", path.display()), error))?;
        current = resolve_link_target(&current, target);

        if let Some(reason) = lift.denies(&current) {
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
