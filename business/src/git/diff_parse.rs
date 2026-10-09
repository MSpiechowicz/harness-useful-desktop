//! Unified diff text (`git diff -U3 --src-prefix=a/ --dst-prefix=b/`) to [`FileDiff`]s.
//!
//! Handles new, deleted, renamed and copied files, mode-only changes, `Binary files … differ`,
//! `\ No newline at end of file`, quoted paths (C-style escapes) and CRLF content (a line's `\r`
//! is kept). Hunk bodies are read by their header counts, so a removed line that looks like a
//! header (`--- x`) stays a line. Combined diffs of unmerged paths are reported as conflicted
//! without hunks.
//!
//! Limits: a file keeps at most [`MAX_FILE_DIFF_BYTES`] of line text and a whole result at most
//! [`MAX_TOTAL_DIFF_BYTES`] ([`DiffBudget`]); beyond that hunks are cut and the file is marked
//! `truncated`. Added/removed counts always cover the whole diff. Deny-listed paths (`.env`,
//! keys, databases, logs, ...) keep their counts but never their content.

use std::path::Path;

use crate::model::{DiffLine, DiffLineKind, FileDiff, GitFileStatus, Hunk, MAX_FILE_DIFF_BYTES, MAX_TOTAL_DIFF_BYTES};
use crate::security::is_denied;

/// The diff text still allowed in one `git_diff` result.
#[derive(Debug, Clone, Copy)]
pub struct DiffBudget {
    left: usize,
}

impl Default for DiffBudget {
    fn default() -> Self {
        Self {
            left: MAX_TOTAL_DIFF_BYTES,
        }
    }
}

impl DiffBudget {
    /// True when nothing more may be added.
    pub fn exhausted(&self) -> bool {
        self.left == 0
    }

    /// Takes `bytes` if they fit.
    pub fn take(&mut self, bytes: usize) -> bool {
        if bytes > self.left {
            self.left = 0;
            return false;
        }

        self.left -= bytes;
        true
    }
}

/// A file diff with no content (binary, withheld or over a limit).
pub fn empty_file(path: String, status: GitFileStatus, staged: bool) -> FileDiff {
    FileDiff {
        path,
        old_path: None,
        status,
        staged,
        is_binary: false,
        hunks: Vec::new(),
        truncated: false,
        additions: 0,
        deletions: 0,
    }
}

/// Lines still expected in the open hunk, and the next line numbers.
#[derive(Debug, Clone, Copy)]
struct Cursor {
    old_no: u32,
    new_no: u32,
    old_left: u32,
    new_left: u32,
}

struct Builder {
    file: FileDiff,
    bytes: usize,
    cursor: Option<Cursor>,
    combined: bool,
}

/// Parses `raw`; `staged` marks index changes.
pub fn parse_diff(raw: &str, staged: bool, budget: &mut DiffBudget) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let mut current: Option<Builder> = None;

    for line in raw.split('\n') {
        if let Some(builder) = current.as_mut()
            && builder.in_hunk_body(line)
        {
            builder.body_line(line, budget);
            continue;
        }

        let combined = line
            .strip_prefix("diff --cc ")
            .or_else(|| line.strip_prefix("diff --combined "));

        if let Some(rest) = line.strip_prefix("diff --git ") {
            files.extend(current.take().map(Builder::finish));
            current = Some(Builder::start(rest, staged, budget));
        } else if let Some(path) = combined {
            files.extend(current.take().map(Builder::finish));
            current = Some(Builder::conflicted(path, staged));
        } else if let Some(builder) = current.as_mut() {
            builder.header_line(line);
        }
    }

    files.extend(current.map(Builder::finish));
    files
}

impl Builder {
    fn start(rest: &str, staged: bool, budget: &DiffBudget) -> Self {
        let (old, new) = header_paths(rest);
        let mut file = empty_file(new.clone(), GitFileStatus::Modified, staged);
        file.old_path = (old != new).then_some(old);
        file.truncated = budget.exhausted();

        Self {
            file,
            bytes: 0,
            cursor: None,
            combined: false,
        }
    }

    fn conflicted(path: &str, staged: bool) -> Self {
        let mut file = empty_file(unquote(path).0, GitFileStatus::Conflicted, staged);
        file.truncated = true;

        Self {
            file,
            bytes: 0,
            cursor: None,
            combined: true,
        }
    }

    /// True for a line of the open hunk (including a trailing `\ No newline` marker).
    fn in_hunk_body(&self, line: &str) -> bool {
        if self.combined || self.file.hunks.is_empty() {
            return false;
        }

        match self.cursor {
            Some(cursor) if cursor.old_left > 0 || cursor.new_left > 0 => true,
            _ => line.starts_with('\\'),
        }
    }

    fn header_line(&mut self, line: &str) {
        if self.combined {
            return;
        }

        let file = &mut self.file;

        if let Some(header) = line.strip_prefix("@@ ") {
            self.open_hunk(line, header);
        } else if line.starts_with("new file mode ") {
            file.status = GitFileStatus::Added;
        } else if line.starts_with("deleted file mode ") {
            file.status = GitFileStatus::Deleted;
        } else if let Some(path) = line.strip_prefix("rename from ") {
            file.status = GitFileStatus::Renamed;
            file.old_path = Some(unquote(path).0);
        } else if let Some(path) = line.strip_prefix("rename to ") {
            file.path = unquote(path).0;
        } else if let Some(path) = line.strip_prefix("copy from ") {
            file.status = GitFileStatus::Copied;
            file.old_path = Some(unquote(path).0);
        } else if let Some(path) = line.strip_prefix("copy to ") {
            file.path = unquote(path).0;
        } else if let Some(path) = line.strip_prefix("+++ ") {
            if let Some(path) = side_path(path, "b/") {
                file.path = path;
            }
        } else if line.starts_with("Binary files ") || line == "GIT binary patch" {
            file.is_binary = true;
        }
    }

    fn open_hunk(&mut self, line: &str, header: &str) {
        let Some((old_start, old_lines, new_start, new_lines)) = hunk_ranges(header) else {
            return;
        };

        self.cursor = Some(Cursor {
            old_no: old_start,
            new_no: new_start,
            old_left: old_lines,
            new_left: new_lines,
        });

        let header_text = line.trim_end_matches('\r').to_owned();
        let keep = self.fits(header_text.len());

        // Hunks past a limit are still counted but not kept; the cursor keeps line numbers right.
        if keep {
            self.file.hunks.push(Hunk {
                header: header_text,
                old_start,
                old_lines,
                new_start,
                new_lines,
                lines: Vec::new(),
            });
        } else {
            self.file.hunks.push(placeholder_hunk());
        }
    }

    fn body_line(&mut self, line: &str, budget: &mut DiffBudget) {
        let Some(cursor) = self.cursor.as_mut() else {
            return;
        };

        let (kind, text) = match line.split_at_checked(1) {
            Some(("+", text)) => (DiffLineKind::Add, text),
            Some(("-", text)) => (DiffLineKind::Remove, text),
            Some(("\\", _)) => (DiffLineKind::NoNewline, ""),
            Some((_, text)) => (DiffLineKind::Context, text),
            // `diff.suppressBlankEmpty` prints blank context lines as empty lines.
            None => (DiffLineKind::Context, ""),
        };

        let (old_no, new_no) = match kind {
            DiffLineKind::Context => (Some(cursor.old_no), Some(cursor.new_no)),
            DiffLineKind::Remove => (Some(cursor.old_no), None),
            DiffLineKind::Add => (None, Some(cursor.new_no)),
            DiffLineKind::NoNewline => (None, None),
        };

        if old_no.is_some() {
            cursor.old_no += 1;
            cursor.old_left = cursor.old_left.saturating_sub(1);
        }

        if new_no.is_some() {
            cursor.new_no += 1;
            cursor.new_left = cursor.new_left.saturating_sub(1);
        }

        match kind {
            DiffLineKind::Add => self.file.additions += 1,
            DiffLineKind::Remove => self.file.deletions += 1,
            _ => {}
        }

        let cost = text.len() + 1;
        if !self.fits(cost) || !budget.take(cost) {
            self.file.truncated = true;
            return;
        }

        self.bytes += cost;
        if let Some(hunk) = self.file.hunks.last_mut().filter(|hunk| !hunk.header.is_empty()) {
            hunk.lines.push(DiffLine {
                kind,
                old_no,
                new_no,
                text: text.to_owned(),
            });
        }
    }

    /// True when `bytes` more fit the per-file limit and the file was not cut already.
    fn fits(&mut self, bytes: usize) -> bool {
        if self.file.truncated || self.bytes + bytes > MAX_FILE_DIFF_BYTES {
            self.file.truncated = true;
            return false;
        }

        true
    }

    fn finish(self) -> FileDiff {
        let mut file = self.file;
        file.hunks.retain(|hunk| !hunk.header.is_empty());

        if file.is_binary {
            file.hunks.clear();
        }

        if is_denied(Path::new(&file.path)).is_some() {
            file.hunks.clear();
            file.truncated = true;
        }

        file
    }
}

/// A hunk past a limit, dropped when the file is finished.
fn placeholder_hunk() -> Hunk {
    Hunk {
        header: String::new(),
        old_start: 0,
        old_lines: 0,
        new_start: 0,
        new_lines: 0,
        lines: Vec::new(),
    }
}

/// `-a[,b] +c[,d] @@ …` (counts default to 1).
fn hunk_ranges(header: &str) -> Option<(u32, u32, u32, u32)> {
    let mut parts = header.split(' ');
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;

    let range = |text: &str| -> Option<(u32, u32)> {
        match text.split_once(',') {
            Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
            None => Some((text.parse().ok()?, 1)),
        }
    };

    let (old_start, old_lines) = range(old)?;
    let (new_start, new_lines) = range(new)?;
    Some((old_start, old_lines, new_start, new_lines))
}

/// The two paths of a `diff --git a/<old> b/<new>` line, without their prefixes.
fn header_paths(rest: &str) -> (String, String) {
    if rest.starts_with('"') {
        let (old, remaining) = unquote(rest);
        let new = unquote(remaining.trim_start()).0;
        return (strip_side(old, "a/"), strip_side(new, "b/"));
    }

    if let Some((old, new)) = rest.split_once(" \"") {
        return (
            strip_side(old.to_owned(), "a/"),
            strip_side(unquote(&format!("\"{new}")).0, "b/"),
        );
    }

    // Unquoted: both halves are equal unless the file was renamed (then `rename to` follows).
    let middle = rest.len() / 2;
    let halves = rest.get(..middle).zip(rest.get(middle + 1..));
    if let Some((old, new)) = halves
        && rest.as_bytes().get(middle) == Some(&b' ')
        && old.strip_prefix("a/") == new.strip_prefix("b/")
    {
        return (strip_side(old.to_owned(), "a/"), strip_side(new.to_owned(), "b/"));
    }

    match rest.split_once(" b/") {
        Some((old, new)) => (strip_side(old.to_owned(), "a/"), new.to_owned()),
        None => (rest.to_owned(), rest.to_owned()),
    }
}

/// The path of a `---`/`+++` line (`None` for `/dev/null`); git appends a tab to names with
/// spaces.
fn side_path(raw: &str, prefix: &str) -> Option<String> {
    let raw = raw.strip_suffix('\t').unwrap_or(raw);

    if raw == "/dev/null" {
        return None;
    }

    Some(strip_side(unquote(raw).0, prefix))
}

fn strip_side(path: String, prefix: &str) -> String {
    match path.strip_prefix(prefix) {
        Some(rest) => rest.to_owned(),
        None => path,
    }
}

/// Decodes a C-style quoted string at the start of `raw` (`\t`, `\"`, `\\`, octal bytes, ...)
/// and returns it with the rest of `raw`; unquoted input is returned as is.
pub fn unquote(raw: &str) -> (String, &str) {
    let Some(body) = raw.strip_prefix('"') else {
        return (raw.to_owned(), "");
    };

    let bytes = body.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;

        match byte {
            b'"' => return (String::from_utf8_lossy(&decoded).into_owned(), &body[index..]),
            b'\\' if index < bytes.len() => {
                let escaped = bytes[index];
                index += 1;

                let octal = |offset: usize| bytes.get(index + offset).filter(|digit| (b'0'..=b'7').contains(digit));
                if (b'0'..=b'3').contains(&escaped)
                    && let (Some(second), Some(third)) = (octal(0), octal(1))
                {
                    decoded.push((escaped - b'0') * 64 + (second - b'0') * 8 + (third - b'0'));
                    index += 2;
                    continue;
                }

                decoded.push(match escaped {
                    b'a' => 0x07,
                    b'b' => 0x08,
                    b't' => b'\t',
                    b'n' => b'\n',
                    b'v' => 0x0b,
                    b'f' => 0x0c,
                    b'r' => b'\r',
                    other => other,
                });
            }
            other => decoded.push(other),
        }
    }

    (String::from_utf8_lossy(&decoded).into_owned(), "")
}
