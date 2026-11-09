//! The project files the composer's `@` menu offers (`project_files`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Most matches one `project_files` call returns; a larger `limit` is cut to this.
pub const MAX_PROJECT_FILES: usize = 50;

/// Longest accepted `project_files` query; a longer one matches nothing.
pub const MAX_PROJECT_FILE_QUERY_BYTES: usize = 256;

/// A file or folder of the project that matches a `project_files` query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectFile {
    /// Path relative to the project, with `/` separators on every OS and no trailing `/`.
    pub path: String,
    /// True for a folder.
    pub is_dir: bool,
    /// True when `git status` lists the file as changed (always false outside a repository).
    pub changed: bool,
}
