//! Working-tree state of the project repository (`git_status`, `git_branches`, `git_diff`).
//!
//! All paths are relative to the project root and use `/` separators.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Maximum diff text per file; longer diffs are cut and marked `truncated`.
pub const MAX_FILE_DIFF_BYTES: usize = 1024 * 1024;

/// Maximum diff text of one `git_diff` result; later files are marked `truncated` without hunks.
pub const MAX_TOTAL_DIFF_BYTES: usize = 8 * 1024 * 1024;

/// Untracked files larger than this are listed without content.
pub const MAX_UNTRACKED_FILE_BYTES: u64 = 512 * 1024;

/// Maximum number of branches returned by `git_branches`.
pub const MAX_BRANCHES: usize = 200;

/// How a file differs from its base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GitFileStatus {
    /// Content changed.
    Modified,
    /// New file.
    Added,
    /// File removed.
    Deleted,
    /// Moved, possibly with changes (see `origPath`/`oldPath`).
    Renamed,
    /// Copied from another file.
    Copied,
    /// File type changed (e.g. file to symlink).
    TypeChanged,
    /// Not tracked by git.
    Untracked,
    /// Unmerged (merge conflict).
    Conflicted,
    /// Ignored by `.gitignore`.
    Ignored,
}

/// One changed path in `git status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StatusEntry {
    /// Current path.
    pub path: String,
    /// Original path of a rename or copy.
    pub orig_path: Option<String>,
    /// Change in the index (staged), if any.
    pub staged: Option<GitFileStatus>,
    /// Change in the working tree (unstaged), if any; `untracked` for untracked files.
    pub unstaged: Option<GitFileStatus>,
}

/// Result of `git_status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GitStatus {
    /// False when the project is not inside a git repository; every other field is then empty.
    pub is_repo: bool,
    /// Current branch; `None` when HEAD is detached.
    pub branch: Option<String>,
    /// Short sha of HEAD; `None` before the first commit.
    pub head: Option<String>,
    /// Upstream of the current branch (e.g. `origin/main`).
    pub upstream: Option<String>,
    /// Commits on the branch that are not on the upstream.
    pub ahead: u32,
    /// Commits on the upstream that are not on the branch.
    pub behind: u32,
    /// Changed paths, sorted by path.
    pub entries: Vec<StatusEntry>,
}

/// A commit as shown next to a branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitInfo {
    /// Short sha.
    pub sha: String,
    /// First line of the commit message.
    pub summary: String,
    /// Author name.
    pub author: String,
    /// Committer date (RFC 3339).
    pub date: String,
}

/// One branch in `git_branches` (local branches first, then remote ones).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Branch {
    /// Short name (`main`, `origin/main`).
    pub name: String,
    /// True for the checked-out branch.
    pub is_current: bool,
    /// True for a remote-tracking branch.
    pub is_remote: bool,
    /// Upstream of a local branch.
    pub upstream: Option<String>,
    /// Commits ahead of the upstream; `None` without an upstream.
    pub ahead: Option<u32>,
    /// Commits behind the upstream; `None` without an upstream.
    pub behind: Option<u32>,
    /// The commit the branch points to.
    pub last_commit: Option<CommitInfo>,
}

/// Which changes `git_diff` compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DiffScope {
    /// Working tree against the index.
    Unstaged,
    /// Index against HEAD.
    Staged,
    /// Untracked files as all-added diffs.
    Untracked,
    /// Staged, unstaged and untracked changes.
    All,
}

/// Arguments of `git_diff`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiffRequest {
    /// Which changes to compare.
    pub scope: DiffScope,
    /// Project-relative paths to limit the diff to; empty means every changed file.
    pub paths: Vec<String>,
}

/// What a diff line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DiffLineKind {
    /// Unchanged line.
    Context,
    /// Added line.
    Add,
    /// Removed line.
    Remove,
    /// `\ No newline at end of file` after the previous line.
    NoNewline,
}

/// One line of a hunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiffLine {
    /// What the line is.
    pub kind: DiffLineKind,
    /// Line number in the old file (context and removed lines).
    pub old_no: Option<u32>,
    /// Line number in the new file (context and added lines).
    pub new_no: Option<u32>,
    /// Line content without the `+`/`-`/` ` prefix and line ending (a `\r` of CRLF content is
    /// kept); empty for `noNewline`.
    pub text: String,
}

/// One hunk of a file diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Hunk {
    /// The `@@ … @@` header line, including any section heading.
    pub header: String,
    /// First line of the hunk in the old file.
    pub old_start: u32,
    /// Number of old-file lines in the hunk.
    pub old_lines: u32,
    /// First line of the hunk in the new file.
    pub new_start: u32,
    /// Number of new-file lines in the hunk.
    pub new_lines: u32,
    /// Lines in order.
    pub lines: Vec<DiffLine>,
}

/// The diff of one file in `git_diff`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileDiff {
    /// Current path.
    pub path: String,
    /// Previous path of a rename or copy.
    pub old_path: Option<String>,
    /// How the file changed.
    pub status: GitFileStatus,
    /// True for an index (staged) change, false for a working-tree or untracked one.
    pub staged: bool,
    /// True for binary content, which has no hunks.
    pub is_binary: bool,
    /// Hunks in file order.
    pub hunks: Vec<Hunk>,
    /// True when the hunks were cut at a size limit or the content was withheld.
    pub truncated: bool,
    /// Added lines.
    pub additions: u32,
    /// Removed lines.
    pub deletions: u32,
}
