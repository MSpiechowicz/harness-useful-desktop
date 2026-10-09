//! Session folder names: how OMP and PI encode a project directory into one path segment.
//!
//! - OMP mirrors `getDefaultSessionDirName` in `session/session-paths.ts` (18.6.1). Paths are
//!   compared in canonical form. A project inside the temp root becomes `-tmp-<relative>` (the
//!   temp root is checked first because it may lie inside home, as on Windows). A project inside
//!   home becomes `-<relative>`, and home itself becomes `-`. Anything else uses the legacy
//!   `--<path>--` form. In relative parts, `/`, `\` and `:` become `-`.
//! - PI uses `--<path>--`: one leading separator is stripped, then `/`, `\` and `:` become `-`
//!   (PI docs `session-format.md`).
//!
//! The functions take path strings and a Windows flag, so tests can check every platform on any
//! host. On Windows the containment checks ignore case and accept both separators, like
//! `path.relative` there.

use std::path::{Path, PathBuf};

use crate::env::{Env, OsKind};

/// The OMP session folder name of `cwd` (all three paths canonical).
pub fn omp_dir_name(cwd: &str, home: &str, temp: &str, windows: bool) -> String {
    let cwd = trim_trailing(cwd, windows);

    if let Some(relative) = relative_within(temp, cwd, windows) {
        return relative_name("-tmp", &relative);
    }

    if let Some(relative) = relative_within(home, cwd, windows) {
        return relative_name("-", &relative);
    }

    legacy_name(cwd)
}

/// The PI session folder name of `cwd`.
pub fn pi_dir_name(cwd: &str) -> String {
    legacy_name(cwd)
}

/// `os.tmpdir()`: `TMPDIR`, `TMP` or `TEMP` (else `/tmp`) on Unix; `TEMP` or `TMP` (else
/// `C:\Windows\Temp`) on Windows.
pub fn temp_root(env: &Env) -> PathBuf {
    let (names, fallback): (&[&str], &str) = match env.os {
        OsKind::Windows => (&["TEMP", "TMP"], r"C:\Windows\Temp"),
        OsKind::Linux | OsKind::MacOs => (&["TMPDIR", "TMP", "TEMP"], "/tmp"),
    };

    let raw = names.iter().find_map(|name| env.var(name)).unwrap_or(fallback);
    let windows = env.os == OsKind::Windows;

    PathBuf::from(trim_trailing(raw, windows))
}

/// `path` as a string after resolving symlinks when it exists (OMP's `resolveEquivalentPath`).
pub fn canonical_text(path: &Path) -> String {
    let canonical = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    canonical.to_string_lossy().into_owned()
}

/// `--<path without one leading separator>--` with `/`, `\` and `:` as `-`.
fn legacy_name(path: &str) -> String {
    let stripped = path.strip_prefix(['/', '\\']).unwrap_or(path);
    format!("--{}--", replace_separators(stripped))
}

/// `prefix` alone for an empty relative path, else `prefix` and the encoded parts joined by `-`.
fn relative_name(prefix: &str, relative: &[&str]) -> String {
    if relative.is_empty() {
        return prefix.to_owned();
    }

    let encoded = replace_separators(&relative.join("-"));

    if prefix.ends_with('-') {
        format!("{prefix}{encoded}")
    } else {
        format!("{prefix}-{encoded}")
    }
}

fn replace_separators(text: &str) -> String {
    text.chars()
        .map(|character| match character {
            '/' | '\\' | ':' => '-',
            other => other,
        })
        .collect()
}

/// The parts of `target` below `root`, or `None` when `target` is outside it.
fn relative_within<'a>(root: &str, target: &'a str, windows: bool) -> Option<Vec<&'a str>> {
    let root_parts = parts(root, windows);
    let target_parts = parts(target, windows);

    if root_parts.is_empty() || root_parts.len() > target_parts.len() {
        return None;
    }

    let same = |left: &str, right: &str| {
        if windows {
            left.eq_ignore_ascii_case(right)
        } else {
            left == right
        }
    };

    let contained = root_parts
        .iter()
        .zip(&target_parts)
        .all(|(left, right)| same(left, right));

    contained.then(|| target_parts[root_parts.len()..].to_vec())
}

/// Non-empty path segments (`/`, and `\` on Windows, separate them).
fn parts(path: &str, windows: bool) -> Vec<&str> {
    let separator = |character: char| character == '/' || (windows && character == '\\');
    path.split(separator).filter(|part| !part.is_empty()).collect()
}

/// Drops trailing separators except for a root (`/`, `C:\`), like `path.resolve`.
fn trim_trailing(path: &str, windows: bool) -> &str {
    let separators: &[char] = if windows { &['/', '\\'] } else { &['/'] };
    let trimmed = path.trim_end_matches(separators);

    let is_drive = windows && trimmed.len() == 2 && trimmed.ends_with(':');
    if trimmed.is_empty() || is_drive {
        return &path[..(trimmed.len() + 1).min(path.len())];
    }

    trimmed
}
