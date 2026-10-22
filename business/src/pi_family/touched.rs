//! Files a file-editing tool call may have changed, for the "touched by this session" filter.
//!
//! Only tools named `edit`, `write`, `ast_edit`, `multi_edit` or `notebook_edit` (any case)
//! count, and only their top-level string arguments `path`, `file_path` and `moveTo`. A path is
//! resolved against the project, `.`/`..` are folded, and it is kept only when it lies inside the
//! project after canonicalization: an existing file is canonicalized itself, a new file through
//! its parent directory (so symlinked directories cannot point outside), and a symlink whose
//! target cannot be resolved is never kept. The result is project-relative with forward slashes.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

/// Tools whose calls change files.
pub const EDIT_TOOLS: [&str; 5] = ["edit", "write", "ast_edit", "multi_edit", "notebook_edit"];

/// Argument names that hold a file path.
const PATH_ARGS: [&str; 3] = ["path", "file_path", "moveTo"];

/// Longest path argument considered.
const MAX_PATH_BYTES: usize = 4096;

/// True when `tool` edits files (case-insensitive).
pub fn is_edit_tool(tool: &str) -> bool {
    EDIT_TOOLS.iter().any(|name| name.eq_ignore_ascii_case(tool))
}

/// The project-relative paths `tool` called with `args` may have changed (see the module docs).
pub fn touched_paths(project: &Path, tool: &str, args: &Value) -> Vec<String> {
    if !is_edit_tool(tool) {
        return Vec::new();
    }

    let Ok(root) = dunce::canonicalize(project) else {
        return Vec::new();
    };

    let mut touched: Vec<String> = Vec::new();

    for key in PATH_ARGS {
        let Some(raw) = args.get(key).and_then(Value::as_str) else {
            continue;
        };

        if let Some(relative) = contained(&root, raw)
            && !touched.contains(&relative)
        {
            touched.push(relative);
        }
    }

    touched
}

/// `raw` relative to canonical `root`, if it resolves inside it.
fn contained(root: &Path, raw: &str) -> Option<String> {
    if raw.is_empty() || raw.len() > MAX_PATH_BYTES || raw.contains('\0') {
        return None;
    }

    let candidate = Path::new(raw);
    let joined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        root.join(candidate)
    };

    let folded = fold(&joined)?;
    let resolved = canonical_or_new(&folded)?;
    let relative = resolved.strip_prefix(root).ok()?;

    if relative.as_os_str().is_empty() {
        return None;
    }

    let parts: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();

    Some(parts.join("/"))
}

/// Folds `.` and `..` lexically; `None` when `..` climbs above the root.
fn fold(path: &Path) -> Option<PathBuf> {
    let mut folded = PathBuf::new();

    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !folded.pop() {
                    return None;
                }
            }
            other => folded.push(other.as_os_str()),
        }
    }

    Some(folded)
}

/// The canonical path of an existing file, or of a new file's existing parent joined with its
/// name. `None` for a symlink whose target cannot be resolved (dangling, a loop): where a write
/// through it lands is unknown, so it never counts as inside the project.
fn canonical_or_new(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = dunce::canonicalize(path) {
        return Some(canonical);
    }

    let is_link = std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink());

    if is_link {
        return None;
    }

    let parent = dunce::canonicalize(path.parent()?).ok()?;
    Some(parent.join(path.file_name()?))
}
