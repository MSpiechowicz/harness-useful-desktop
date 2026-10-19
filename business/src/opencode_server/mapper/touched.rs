//! The project files an OpenCode tool call may have changed, and whether a session's directory
//! belongs to a project. Shared by the live chat ([`super::Mapper`]) and the CLI transcript.
//!
//! Tools: `edit`, `write` (and the older `multiedit`, `patch`) name the file in `filePath`;
//! `apply_patch` (and the older `patch`) in the `*** … File:` headers of `patchText`. A relative
//! path resolves against the session's directory, as OpenCode resolves it; only paths inside the
//! project count ([`touched_paths`]).

use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};

use crate::pi_family::touched::touched_paths;

/// Tools whose `filePath` argument names a file they change.
const FILE_PATH_TOOLS: [&str; 4] = ["edit", "write", "multiedit", "patch"];

/// Tools whose `patchText` argument names the files they change.
const PATCH_TOOLS: [&str; 2] = ["apply_patch", "patch"];

/// Patch headers followed by a file path (`packages/opencode/src/patch`).
const PATCH_HEADERS: [&str; 4] = ["*** Add File:", "*** Update File:", "*** Delete File:", "*** Move to:"];

/// The project-relative files (forward slashes) the call of a `tool` part, in a session working
/// in `directory`, may have changed (see the module docs).
pub(crate) fn part_touched(project: &Path, directory: &Path, part: &Value) -> Vec<String> {
    let tool = part.get("tool").and_then(Value::as_str).unwrap_or_default();
    let input = part.pointer("/state/input").unwrap_or(&Value::Null);
    let mut raw_paths: Vec<&str> = Vec::new();

    if is_one_of(tool, &FILE_PATH_TOOLS)
        && let Some(path) = input.get("filePath").and_then(Value::as_str)
    {
        raw_paths.push(path);
    }

    if is_one_of(tool, &PATCH_TOOLS)
        && let Some(patch) = input.get("patchText").and_then(Value::as_str)
    {
        raw_paths.extend(patch_paths(patch));
    }

    let mut touched: Vec<String> = Vec::new();

    for raw in raw_paths {
        let absolute = directory.join(raw);
        let args = json!({ "path": absolute.to_string_lossy() });

        for path in touched_paths(project, "edit", &args) {
            if !touched.contains(&path) {
                touched.push(path);
            }
        }
    }

    touched
}

/// `directory` (a session's working directory) canonicalized, when it is `project` (canonical) or
/// lies inside it. A directory that no longer exists is compared as written, without `..`.
pub(crate) fn project_dir(directory: &str, project: &Path) -> Option<PathBuf> {
    let path = Path::new(directory);

    if directory.is_empty() || !path.is_absolute() {
        return None;
    }

    let resolved = match dunce::canonicalize(path) {
        Ok(canonical) => canonical,
        Err(_) if has_parent_component(path) => return None,
        Err(_) => path.to_path_buf(),
    };

    resolved.starts_with(project).then_some(resolved)
}

fn has_parent_component(path: &Path) -> bool {
    path.components().any(|part| matches!(part, Component::ParentDir))
}

fn is_one_of(tool: &str, names: &[&str]) -> bool {
    names.iter().any(|name| name.eq_ignore_ascii_case(tool))
}

/// The file paths named by the headers of an `apply_patch` patch.
fn patch_paths(patch: &str) -> impl Iterator<Item = &str> {
    patch.lines().filter_map(|line| {
        PATCH_HEADERS
            .iter()
            .find_map(|header| line.strip_prefix(header))
            .map(str::trim)
            .filter(|path| !path.is_empty())
    })
}
