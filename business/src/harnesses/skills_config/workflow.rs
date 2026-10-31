//! The Harness Skills workflow switches, read the way the plugin reads them (harness-useful-skills
//! `workflow-settings.js`, `claude/settings.js` and `opencode/settings.js`). Below the harness's
//! settings root (Claude Code's config directory, OpenCode's XDG config directory),
//! `harness-useful-skills/workflow-settings/` (`useful-skills/…` before Harness Skills 0.4.0, used when
//! only it exists) holds the repository master
//! `repositories/<sha256 of the scope root>/workflow.json` and the seven profile-wide stage files
//! `<stage>.json`. The scope root is the nearest enclosing Git checkout of the project (a `.git`
//! folder, or a `.git` file naming its `gitdir:`), else the canonical project.
//!
//! Each file holds a JSON boolean of at most [`MAX_SETTING_BYTES`]. A missing file means enabled.
//! An unsafe or unreadable file is an error for its key, never a default: a symlink, anything
//! but a regular file, a file too big or not a boolean, or a folder of the store that is a
//! symlink or (on Unix) open to other users. A settings root that cannot be resolved is an error
//! for every key, unless it is missing and the harness reads that as no saved settings
//! ([`MissingRoot`]). Nothing is created or changed.
//!
//! The last stage is `code-graph`. Before Harness Skills 0.5.0 it was `graphify-memory`: while
//! `code-graph.json` is missing, `graphify-memory.json` is read in its place, under the same safety
//! rules.

use std::fs::{self, Metadata};
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::ecosystem::skills_paths::skills_data_path;
use crate::security::SafeReader;

/// The profile-wide stages, in the plugin's order.
const STAGES: [&str; 7] = [
    "research",
    "plan",
    "review",
    "security-review",
    "release-assessment",
    "backend-memory",
    "code-graph",
];

/// The file name the `code-graph` stage had before Harness Skills 0.5.0 (`graphify-memory`); read
/// when `code-graph.json` is missing.
const LEGACY_CODE_GRAPH: &str = "graphify-memory";

/// The per-repository master switch.
const WORKFLOW: &str = "workflow";

/// Largest settings file and `.git` file read, as in the plugin.
const MAX_SETTING_BYTES: u64 = 4096;

/// What one switch is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Setting {
    /// Read from its file.
    Saved(bool),
    /// No file, so enabled (on Claude Code, or the `/plugin` option, which only its hooks see).
    Default,
    /// The repository master without a project open.
    Unavailable,
    /// The file or a folder above it could not be read safely; the plugin's reason.
    Error(String),
}

impl Setting {
    /// How the view shows it.
    pub(super) fn label(&self) -> String {
        match self {
            Self::Saved(true) => "enabled (saved file)".to_owned(),
            Self::Saved(false) => "disabled (saved file)".to_owned(),
            Self::Default => "default (enabled)".to_owned(),
            Self::Unavailable => "not available (no project open)".to_owned(),
            Self::Error(reason) => format!("error: {reason}"),
        }
    }
}

/// What a settings root that does not exist means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::harnesses) enum MissingRoot {
    /// An error for every key, as Claude Code's plugin reads it.
    Error,
    /// No saved settings, as OpenCode's plugin reads it.
    Defaults,
}

/// The settings folder below `root`: `harness-useful-skills/workflow-settings`, or the old
/// `useful-skills/workflow-settings` when only that exists.
pub(super) fn settings_dir(root: &Path) -> PathBuf {
    skills_data_path(root, &["workflow-settings"])
}

/// Every switch, the master first (see the module docs).
pub(super) fn read_settings(root: &Path, missing: MissingRoot, project: Option<&Path>) -> Vec<(&'static str, Setting)> {
    let directory = match store_dir(root, missing) {
        Ok(directory) => directory,
        Err(reason) => return every_key(&Setting::Error(reason)),
    };

    let workflow = match (project.map(scope_root), &directory) {
        (None, _) => Setting::Unavailable,
        (Some(Err(reason)), _) => Setting::Error(reason),
        (Some(Ok(_)), None) => Setting::Default,
        (Some(Ok(root)), Some(directory)) => read_master(directory, &root),
    };

    let mut settings = vec![(WORKFLOW, workflow)];
    settings.extend(STAGES.iter().map(|&stage| {
        let setting = match &directory {
            Some(directory) => read_setting(directory, stage),
            None => Setting::Default,
        };

        (stage, setting)
    }));

    settings
}

/// The settings folder below the canonical `root`, `None` when there is none yet, or why it
/// cannot be read safely.
fn store_dir(root: &Path, missing: MissingRoot) -> Result<Option<PathBuf>, String> {
    let canonical = match dunce::canonicalize(root) {
        Ok(canonical) => canonical,
        Err(_) if missing == MissingRoot::Defaults && is_missing(root) => return Ok(None),
        Err(_) => return Err("Cannot resolve agent profile directory.".to_owned()),
    };

    let directory = settings_dir(&canonical);
    let present = check_ancestor(directory.parent().unwrap_or(&directory))? && check_ancestor(&directory)?;

    Ok(present.then_some(directory))
}

/// True only when nothing is at `path`, not even a dangling link.
fn is_missing(path: &Path) -> bool {
    fs::symlink_metadata(path).is_err_and(|error| error.kind() == ErrorKind::NotFound)
}

fn every_key(setting: &Setting) -> Vec<(&'static str, Setting)> {
    std::iter::once(WORKFLOW)
        .chain(STAGES)
        .map(|key| (key, setting.clone()))
        .collect()
}

/// The master of the repository at `root`: `repositories/<sha256 hex of root>/workflow.json`.
fn read_master(directory: &Path, root: &Path) -> Setting {
    let repositories = directory.join("repositories");
    let local = repositories.join(repository_key(root));

    let present = check_ancestor(&repositories).and_then(|present| Ok(present && check_ancestor(&local)?));

    match present {
        Ok(true) => read_setting(&local, WORKFLOW),
        Ok(false) => Setting::Default,
        Err(reason) => Setting::Error(reason),
    }
}

/// The folder name of the repository at `root`: the SHA-256 hex digest of its path.
fn repository_key(root: &Path) -> String {
    format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes()))
}

/// Whether the store folder `dir` exists, or why it is unsafe.
fn check_ancestor(dir: &Path) -> Result<bool, String> {
    let metadata = match fs::symlink_metadata(dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("Cannot inspect managed workflow directory.".to_owned()),
    };

    if !metadata.file_type().is_dir() {
        return Err("Managed workflow ancestor must be a non-symlink directory.".to_owned());
    }

    if !is_private(&metadata) {
        return Err("Managed workflow ancestor must be private.".to_owned());
    }

    Ok(true)
}

/// The `<key>.json` file of `dir`; for `code-graph`, the legacy `graphify-memory.json` while
/// that is missing.
fn read_setting(dir: &Path, key: &str) -> Setting {
    let setting = read_file(dir, key);

    if key == "code-graph" && setting == Setting::Default {
        return read_file(dir, LEGACY_CODE_GRAPH);
    }

    setting
}

/// The `<name>.json` file of `dir`: `Default` when it is missing.
fn read_file(dir: &Path, name: &str) -> Setting {
    let file = dir.join(format!("{name}.json"));

    let metadata = match fs::symlink_metadata(&file) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Setting::Default,
        Err(_) => return error("Cannot inspect workflow setting safely."),
    };

    if !metadata.file_type().is_file() {
        return error("Workflow setting must be a non-symlink regular file.");
    }

    if metadata.len() > MAX_SETTING_BYTES {
        return error("Workflow setting exceeds the size limit.");
    }

    let Ok(text) = SafeReader::read_to_string(&file, MAX_SETTING_BYTES) else {
        return error("Cannot read workflow setting safely.");
    };

    match serde_json::from_str(&text) {
        Ok(Value::Bool(enabled)) => Setting::Saved(enabled),
        _ => error("Workflow setting must contain a JSON boolean."),
    }
}

fn error(reason: &str) -> Setting {
    Setting::Error(reason.to_owned())
}

/// The canonical checkout holding `project`, else the canonical project.
fn scope_root(project: &Path) -> Result<PathBuf, String> {
    let canonical = dunce::canonicalize(project)
        .ok()
        .filter(|path| path.is_dir())
        .ok_or_else(|| "Cannot resolve active workspace directory.".to_owned())?;

    Ok(enclosing_checkout(&canonical)?.unwrap_or(canonical))
}

/// The nearest folder from `canonical` up with a safe `.git` marker (the checkout itself, never a
/// worktree's or submodule's shared Git folder), or `None` outside Git.
fn enclosing_checkout(canonical: &Path) -> Result<Option<PathBuf>, String> {
    for directory in canonical.ancestors() {
        let marker = directory.join(".git");

        let metadata = match fs::symlink_metadata(&marker) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_) => return Err("Cannot inspect workspace Git marker.".to_owned()),
        };

        check_marker(directory, &marker, &metadata)
            .ok_or_else(|| "Cannot safely resolve workspace Git marker.".to_owned())?;

        return Ok(Some(directory.to_path_buf()));
    }

    Ok(None)
}

/// `Some` when `marker` is a usable `.git` folder, or a `.git` file of one `gitdir:` line naming one.
fn check_marker(directory: &Path, marker: &Path, metadata: &Metadata) -> Option<()> {
    let file_type = metadata.file_type();

    if file_type.is_dir() {
        return check_git_dir(marker);
    }

    if !file_type.is_file() {
        return None;
    }

    let text = SafeReader::read_to_string(marker, MAX_SETTING_BYTES).ok()?;
    let target = text.strip_prefix("gitdir: ")?;
    let target = target.strip_suffix('\n').unwrap_or(target);
    let target = target.strip_suffix('\r').unwrap_or(target);

    if target.trim().is_empty() || target.contains(['\r', '\n', '\0']) {
        return None;
    }

    check_git_dir(&normalize(&directory.join(target)))
}

/// `Some` when `dir` is a readable, searchable folder reached without symlinks.
fn check_git_dir(dir: &Path) -> Option<()> {
    let canonical = dunce::canonicalize(dir).ok()?;
    let metadata = fs::symlink_metadata(&canonical).ok()?;

    (canonical == dir && metadata.file_type().is_dir() && is_searchable(&metadata)).then_some(())
}

/// `path` with `.` and `..` resolved by name, as Node's `path.resolve` does.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other),
        }
    }

    normal
}

/// No access for the group or others (Unix); other systems have no such bits.
#[cfg(unix)]
fn is_private(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o077 == 0
}

#[cfg(not(unix))]
fn is_private(_metadata: &Metadata) -> bool {
    true
}

/// Some read and some search bit set (Unix).
#[cfg(unix)]
fn is_searchable(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let mode = metadata.permissions().mode();
    mode & 0o444 != 0 && mode & 0o111 != 0
}

#[cfg(not(unix))]
fn is_searchable(_metadata: &Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_resolve_by_name() {
        assert_eq!(
            normalize(Path::new("/w/repo/./sub/../.git/worktrees/x")),
            PathBuf::from("/w/repo/.git/worktrees/x")
        );
    }

    /// The reference value is Node's `createHash("sha256").update("/w/repo").digest("hex")`.
    #[test]
    fn the_repository_key_is_the_sha256_of_the_path() {
        assert_eq!(
            repository_key(Path::new("/w/repo")),
            "7005185596c2e32bd0688ebe72725f926012fffbf101203830050954a66e9f98"
        );
    }
}
