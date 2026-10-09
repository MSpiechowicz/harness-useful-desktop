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
