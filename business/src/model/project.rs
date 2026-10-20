use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A project directory the user opened or picked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectInfo {
    /// Absolute path of the directory.
    pub path: String,
    /// Display name (the directory name).
    pub name: String,
    /// True when the directory still exists.
    pub exists: bool,
    /// True when the directory is inside a git work tree.
    pub is_git_repo: bool,
}

/// A project the user works with in the desktop (the active one or a recent one) with the
/// identities the views match documents against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectScope {
    /// Absolute path of the directory, as in [`ProjectInfo::path`] (what saves pass as `project`).
    pub path: String,
    /// Display name (the directory name).
    pub name: String,
    /// The project root (git root, else the folder) hooks and routines store, or `None` when the
    /// folder is gone.
    pub root: Option<String>,
    /// The Harness Memory repository key (`git:…` or `path:…`), or `None` when Harness Memory is
    /// missing or could not resolve it.
    pub key: Option<String>,
}
