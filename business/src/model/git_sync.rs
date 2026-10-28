//! Branch sync of the project repository (`git_sync_state`, `git_fetch`, `git_sync_action`,
//! `git_switch`, `git_branch_search`): where the current branch stands against its remote and the
//! main branch, an unfinished rebase or merge, and the actions that move it forward.
//!
//! Paths are relative to the project unless a [`ConflictFile`] says otherwise.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Most branches one `git_branch_search` returns.
pub const MAX_BRANCH_SEARCH: usize = 100;

/// Longest accepted `git_branch_search` query.
pub const MAX_BRANCH_QUERY_BYTES: usize = 200;

/// Where the main branch of a project came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MainSource {
    /// The default branch the remote reports (`refs/remotes/<remote>/HEAD`).
    Remote,
    /// Guessed from the branch names (`init.defaultBranch`, `main`, `master`, `trunk`).
    Fallback,
    /// Picked by the user for this project.
    Picked,
}

/// The branch the current one is compared with and rebased onto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MainBranch {
    /// Short name (`main`).
    pub name: String,
    /// What the actions use: the remote-tracking branch (`origin/main`) when there is one, else the
    /// local branch (`main`).
    pub reference: String,
    /// How the branch was found.
    pub source: MainSource,
    /// Commits on `reference` that the current branch does not have.
    pub new_commits: u32,
    /// True when the main branch is the checked-out one.
    pub is_current: bool,
}

/// An unfinished operation that needs the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GitOperationKind {
    /// `git rebase` stopped.
    Rebase,
    /// `git merge` stopped.
    Merge,
}

/// A file with a conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConflictFile {
    /// Relative to the project when `inProject`, else relative to the repository root.
    pub path: String,
    /// False for a file outside the project folder (the project is a subdirectory of the repository).
    pub in_project: bool,
}

/// A rebase or merge that stopped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GitOperation {
    /// Which operation.
    pub kind: GitOperationKind,
    /// What is applied onto or merged in: the main branch's name when it points at the same commit,
    /// else a short sha.
    pub onto: Option<String>,
    /// Files still in conflict: those of the project first, each group sorted by path. Empty when only the continue step is left.
    pub conflicts: Vec<ConflictFile>,
}

/// The sync state of the project repository.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BranchSync {
    /// False when the project is not inside a git repository; every other field is then empty.
    pub is_repo: bool,
    /// Current branch, or the branch being rebased while a rebase is stopped. `None` when HEAD is
    /// detached.
    pub branch: Option<String>,
    /// Short sha of HEAD; `None` before the first commit.
    pub head: Option<String>,
    /// Upstream of the current branch (e.g. `origin/feat`).
    pub upstream: Option<String>,
    /// The remote to fetch and push: the upstream's remote, else `remote.pushDefault`, else
    /// `origin`, else the only remote. `None` without a remote.
    pub remote: Option<String>,
    /// Commits on the branch that are not on the upstream.
    pub ahead: u32,
    /// Commits on the upstream that are not on the branch.
    pub behind: u32,
    /// The main branch; `None` when none was found.
    pub main: Option<MainBranch>,
    /// A rebase or merge that stopped.
    pub operation: Option<GitOperation>,
    /// Every file of the repository with unresolved conflicts (same order as
    /// `operation.conflicts`), also when no rebase or merge is in progress: an autostash or a
    /// `stash pop` that did not apply cleanly leaves such files behind. `applyStash` and `dropStash` are
    /// refused while any remain.
    pub unmerged: Vec<ConflictFile>,
    /// Tracked files with changes (an autostash takes these along).
    pub tracked_changes: u32,
    /// Untracked files and folders.
    pub untracked: u32,
    /// When the repository was last fetched (RFC 3339), from `FETCH_HEAD`; `None` when unknown or
    /// without a remote.
    pub last_fetched: Option<String>,
    /// True when the newest stash is one this app made (or an autostash), so `applyStash` applies.
    pub stash_kept: bool,
    /// True when that stash (`stashKept`) was already applied with conflicts, so its changes are in
    /// the work tree and the entry is only a copy: `applyStash` is refused and `dropStash` applies
    /// once no conflict is left.
    pub stash_applied: bool,
}

/// What a sync action does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SyncAction {
    /// Push the current branch to its upstream.
    Push,
    /// Push the current branch to the remote and make it the upstream.
    Publish,
    /// Fast-forward the current branch to its upstream.
    Pull,
    /// Rebase the current branch onto the main branch (changes are stashed and put back).
    Rebase,
    /// Merge the main branch into the current branch (changes are stashed and put back).
    Merge,
    /// Abort the stopped rebase or merge.
    Abort,
    /// Continue the stopped rebase, or commit the stopped merge, once no conflict is left.
    Continue,
    /// Apply and drop the stash the app (or an autostash) left.
    ApplyStash,
    /// Drop the stash copy of changes that were applied with conflicts and are resolved now.
    DropStash,
}

/// How a sync action or a branch switch ended. Git failures that need no choice are errors instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ActionOutcome {
    /// It worked.
    Done,
    /// There was nothing to do.
    UpToDate,
    /// A rebase or merge stopped on conflicts.
    Stopped,
    /// Git refused because uncommitted changes are in the way.
    BlockedByChanges,
    /// It worked, but changes are kept in a stash (stashed by the switch, or an autostash that did
    /// not apply cleanly).
    StashKept,
}

/// The result of a sync action or a branch switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GitActionResult {
    /// How it ended.
    pub outcome: ActionOutcome,
    /// The sync state afterwards.
    pub state: BranchSync,
}

/// Where a new branch starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BranchStart {
    /// The main branch.
    Main,
    /// The current commit.
    Current,
}

/// A `git_switch` request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SwitchRequest {
    /// Branch to switch to, or to create. A remote-only branch is created tracking its remote.
    pub branch: String,
    /// Create the branch, starting here; `None` switches to an existing one.
    pub create: Option<BranchStart>,
    /// Stash all changes (untracked ones too) first.
    pub stash: bool,
}

/// One branch in a `git_branch_search` page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BranchRow {
    /// Short name; a remote-only branch has no remote prefix (`feat`, not `origin/feat`).
    pub name: String,
    /// True for the checked-out branch.
    pub is_current: bool,
    /// True when only a remote has the branch.
    pub remote_only: bool,
    /// Date of the last commit (RFC 3339).
    pub last_commit_date: Option<String>,
}

/// A page of `git_branch_search`: local branches first, then remote-only ones, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BranchPage {
    /// The matches from the requested offset, at most the requested limit.
    pub branches: Vec<BranchRow>,
    /// All matches.
    pub total: u32,
}
