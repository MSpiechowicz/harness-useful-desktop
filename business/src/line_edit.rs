//! Shared pieces of the line-based edits of user-owned text files (the OMP settings, the Harness
//! Skills model roles): the file as lines that keep their own terminators, and the guarded
//! read, edit and atomic write of one file.
//!
//! The edits never re-serialize a file. They change or add lines, so comments, order, quoting
//! and the line ending stay as the user wrote them.

use std::fs;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use sha2::{Digest, Sha256};

use crate::error::{BusinessError, Result};
use crate::fs_atomic::replace_atomically_keeping;
use crate::security::SafeReader;

/// The file as lines that keep their own terminators.
pub(crate) struct Lines {
    pub(crate) lines: Vec<String>,
    pub(crate) eol: &'static str,
}

impl Lines {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            lines: text.split_inclusive('\n').map(str::to_owned).collect(),
            eol: if text.contains("\r\n") { "\r\n" } else { "\n" },
        }
    }

    pub(crate) fn text(&self) -> String {
        self.lines.concat()
    }

    /// Replaces the text of line `index` and keeps its terminator (none for an unterminated last line).
    pub(crate) fn set_content(&mut self, index: usize, text: &str) {
        let line = &self.lines[index];
        let terminator = &line[content(line).len()..];

        self.lines[index] = format!("{text}{terminator}");
    }
}

/// A line without its terminator.
pub(crate) fn content(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

/// Blank lines and comments, which belong to no block.
pub(crate) fn is_skipped(line: &str) -> bool {
    let trimmed = line.trim();

    trimmed.is_empty() || trimmed.starts_with('#')
}

/// One edit at a time in this process: the read, the hash check and the rename of an edit are
/// not interleaved with another edit, so two edits never lose each other's change.
static EDIT_LOCK: Mutex<()> = Mutex::new(());

/// Reads the file at `path` (`what` names it in errors, `max_bytes` caps the read), applies `edit`
/// to its text and replaces the file atomically, keeping a symlink in place and the mode of the
/// file. A file with other hard links is refused (`Unsupported`): the replacement would unlink it.
///
/// With `within` (a project folder) the resolved file must lie inside it, checked again right
/// before the file is replaced (`Forbidden`). `before_write` runs after the edit is planned and
/// before the file is replaced, where a change made meanwhile by another program is caught: the
/// file must still hash like the text that was edited, otherwise nothing is written and the
/// result is `Conflict`.
pub(crate) fn edit_text_file(
    path: &Path,
    what: &str,
    max_bytes: u64,
    within: Option<&Path>,
    edit: impl FnOnce(&str) -> Result<String>,
    before_write: impl FnOnce(),
) -> Result<()> {
    let _one_at_a_time = EDIT_LOCK.lock().unwrap_or_else(PoisonError::into_inner);

    if !path.is_file() {
        return Err(BusinessError::NotFound(format!("{what} file")));
    }

    // A symlinked file (dotfiles) is edited in place, so the link stays.
    let target =
        dunce::canonicalize(path).map_err(|error| BusinessError::from_io(format!("resolving {what} file"), error))?;
    let root = within
        .map(|root| {
            dunce::canonicalize(root).map_err(|error| BusinessError::from_io("resolving the project folder", error))
        })
        .transpose()?;

    if let Some(root) = &root {
        confine(root, &target)?;
    }

    let metadata =
        fs::metadata(&target).map_err(|error| BusinessError::from_io(format!("reading {what} file"), error))?;

    if has_other_links(&metadata) {
        return Err(BusinessError::Unsupported(format!(
            "{what} file has other hard links, edit it by hand"
        )));
    }

    let text = SafeReader::read_to_string(&target, max_bytes)?;
    let before = Sha256::digest(text.as_bytes());
    let edited = edit(&text)?;

    replace_atomically_keeping(
        &target,
        edited.as_bytes(),
        &format!("saving {what}"),
        &metadata.permissions(),
        || {
            before_write();

            if let Some(root) = &root {
                confine(root, &target)?;
            }

            let now = SafeReader::read_to_string(&target, max_bytes)?;

            if Sha256::digest(now.as_bytes()) != before {
                return Err(BusinessError::Conflict(format!(
                    "{what} changed while it was being edited, try again"
                )));
            }

            Ok(())
        },
    )
}

/// `target` (the canonical file) must still resolve to itself and lie inside `root`: a symlink
/// put in the way since, in the file or in a folder above it, fails.
fn confine(root: &Path, target: &Path) -> Result<()> {
    let now =
        dunce::canonicalize(target).map_err(|error| BusinessError::from_io("resolving the file to edit", error))?;

    if now != target || !now.starts_with(root) {
        return Err(BusinessError::Forbidden(format!(
            "{} is outside {}",
            target.display(),
            root.display()
        )));
    }

    Ok(())
}

#[cfg(unix)]
fn has_other_links(metadata: &fs::Metadata) -> bool {
    std::os::unix::fs::MetadataExt::nlink(metadata) > 1
}

#[cfg(not(unix))]
fn has_other_links(_metadata: &fs::Metadata) -> bool {
    false
}
