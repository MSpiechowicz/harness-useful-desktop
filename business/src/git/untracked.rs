//! Untracked files as all-added diffs, read by the business layer itself (git shows nothing).
//!
//! Each file is read through [`SafeReader`] after checking that it resolves inside the project.
//! Deny-listed files (`.env`, keys, databases, logs, ...) and files that resolve outside the
//! project are listed without content (`truncated`). Files over [`MAX_UNTRACKED_FILE_BYTES`] are
//! listed without content too; a NUL byte in the first 8 KiB marks a file binary. A symlink is
//! shown as its target path, like git shows it once added.

use std::fs;
use std::io::Read;
use std::path::Path;

use super::diff_parse::{DiffBudget, empty_file};
use crate::model::{DiffLine, DiffLineKind, FileDiff, GitFileStatus, Hunk, MAX_UNTRACKED_FILE_BYTES};
use crate::security::{SafeReader, canonical_within, is_denied};

/// Bytes inspected for a NUL byte.
const BINARY_SNIFF_BYTES: usize = 8 * 1024;

/// The diff of untracked file `relative` (project-relative, `/` separators).
pub fn untracked_diff(project: &Path, relative: &str, budget: &mut DiffBudget) -> FileDiff {
    let mut file = empty_file(relative.to_owned(), GitFileStatus::Untracked, false);
    let full = project.join(relative);

    let Ok(metadata) = fs::symlink_metadata(&full) else {
        file.truncated = true;
        return file;
    };

    let content = if metadata.file_type().is_symlink() {
        fs::read_link(&full)
            .ok()
            .map(|target| target.to_string_lossy().into_owned().into_bytes())
    } else if is_denied(Path::new(relative)).is_some() || metadata.len() > MAX_UNTRACKED_FILE_BYTES {
        None
    } else {
        read_contained(project, &full)
    };

    let Some(content) = content else {
        file.truncated = true;
        return file;
    };

    let sniffed = &content[..content.len().min(BINARY_SNIFF_BYTES)];
    if sniffed.contains(&0) {
        file.is_binary = true;
        return file;
    }

    if !budget.take(content.len()) {
        file.truncated = true;
        return file;
    }

    let text = String::from_utf8_lossy(&content);
    if let Some(hunk) = added_hunk(&text) {
        file.additions = hunk.new_lines;
        file.hunks.push(hunk);
    }

    file
}

/// The bytes of `full` when it resolves inside `project` and is readable.
pub(super) fn read_contained(project: &Path, full: &Path) -> Option<Vec<u8>> {
    let canonical = canonical_within(project, full).ok()?;
    let reader = SafeReader::open(&canonical).ok()?;

    let mut content = Vec::new();
    reader
        .take(MAX_UNTRACKED_FILE_BYTES + 1)
        .read_to_end(&mut content)
        .ok()?;

    // The file may have grown after the size check.
    (content.len() as u64 <= MAX_UNTRACKED_FILE_BYTES).then_some(content)
}

/// `@@ -0,0 +1,N @@` with every line added; `None` for an empty file.
fn added_hunk(text: &str) -> Option<Hunk> {
    if text.is_empty() {
        return None;
    }

    let ends_with_newline = text.ends_with('\n');
    let body = text.strip_suffix('\n').unwrap_or(text);

    let mut lines: Vec<DiffLine> = body
        .split('\n')
        .enumerate()
        .map(|(index, line)| DiffLine {
            kind: DiffLineKind::Add,
            old_no: None,
            new_no: Some(u32::try_from(index + 1).unwrap_or(u32::MAX)),
            text: line.to_owned(),
        })
        .collect();

    let new_lines = u32::try_from(lines.len()).unwrap_or(u32::MAX);

    if !ends_with_newline {
        lines.push(DiffLine {
            kind: DiffLineKind::NoNewline,
            old_no: None,
            new_no: None,
            text: String::new(),
        });
    }

    Some(Hunk {
        header: format!("@@ -0,0 +1,{new_lines} @@"),
        old_start: 0,
        old_lines: 0,
        new_start: 1,
        new_lines,
        lines,
    })
}
