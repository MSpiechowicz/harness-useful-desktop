//! `git_sync_state`: where the current branch stands against its remote and the main branch, and
//! whether a rebase or merge is stopped.
//!
//! Everything here reads the local repository only, never the network. Counts, the conflicts and
//! the branch come from one repository-wide `status --porcelain=v2` (a project in a subdirectory
//! still sees the whole repository, so a conflict outside the project is reported, marked
//! `inProject: false`).
//!
//! The main branch and the remote come from [`super::main_branch`].

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::branch_ops::{validate_branch_name, validate_remote_name};
use super::cli::{GitCli, project_relative};
use super::main_branch::{FoundMain, choose_remote, find_main, main_branch, read_config};
use super::status::{SHORT_SHA, parse_status};
use crate::agent::rfc3339;
use crate::error::Result;
use crate::model::{BranchSync, ConflictFile, GitFileStatus, GitOperation, GitOperationKind, StatusEntry};
use crate::security::SafeReader;

/// Start of the message of every stash this app makes.
pub(super) const STASH_PREFIX: &str = "Harness Desktop:";

/// Subject of the stash `rebase --autostash` and `merge --autostash` make.
const AUTOSTASH_SUBJECT: &str = "autostash";

/// Message a kept stash is stored under once its changes were applied with conflicts: the changes
/// are back in the work tree, the entry is only a copy to drop.
pub(super) const APPLIED_STASH_MESSAGE: &str = "Harness Desktop: applied with conflicts";

/// Largest rebase bookkeeping file read.
const MAX_STATE_FILE_BYTES: u64 = 4096;

/// The sync state of the repository around the project; `isRepo: false` outside a work tree.
pub async fn sync_state(git: &GitCli<'_>, picked: Option<&str>) -> Result<BranchSync> {
    let Some(prefix) = git.prefix().await? else {
        return Ok(BranchSync::default());
    };

    let output = git
        .run_ok(["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=normal"])
        .await?;
    let status = parse_status(&output.stdout, "");
    let counts = count_entries(&status.entries, &prefix);

    let paths = git_paths(git).await?;
    let progress = read_progress(&paths);
    let branch = progress
        .as_ref()
        .and_then(|progress| progress.branch.clone())
        .or(status.branch);

    let config = read_config(git).await?;
    let remotes = remotes(git).await?;
    let remote = choose_remote(&remotes, &config, branch.as_deref());

    let main = match status.head {
        Some(_) => find_main(git, &remotes, remote.as_deref(), &config, picked).await?,
        None => None,
    };
    let operation = progress.map(|progress| GitOperation {
        kind: progress.kind,
        onto: describe_onto(progress.onto.as_deref(), main.as_ref()),
        conflicts: counts.conflicts.clone(),
    });
    let main = match main {
        Some(found) => Some(main_branch(git, found, branch.as_deref()).await?),
        None => None,
    };

    let last_fetched = match remote {
        Some(_) => modified(&paths.fetch_head),
        None => None,
    };

    let top_stash = our_top_stash(git).await?;

    Ok(BranchSync {
        is_repo: true,
        head: status.head,
        upstream: status.upstream,
        remote,
        ahead: status.ahead,
        behind: status.behind,
        main,
        operation,
        unmerged: counts.conflicts,
        tracked_changes: counts.tracked,
        untracked: counts.untracked,
        last_fetched,
        stash_kept: top_stash.is_some(),
        stash_applied: top_stash.is_some_and(|stash| stash.kind == StashKind::Applied),
        branch,
    })
}

/// The remotes of the repository whose names are safe to put on a command line (a remote named
/// like an option, or with control characters, is left out: it is never fetched, pushed to or
/// chosen as the source of the main branch).
pub(super) async fn remotes(git: &GitCli<'_>) -> Result<Vec<String>> {
    let output = git.run_ok(["remote"]).await?;

    Ok(output
        .stdout
        .lines()
        .filter(|line| validate_remote_name(line).is_ok())
        .map(str::to_owned)
        .collect())
}

/// Changed files of the repository, as the sync state counts them.
struct Counts {
    tracked: u32,
    untracked: u32,
    conflicts: Vec<ConflictFile>,
}

fn count_entries(entries: &[StatusEntry], prefix: &str) -> Counts {
    let mut counts = Counts {
        tracked: 0,
        untracked: 0,
        conflicts: Vec::new(),
    };

    for entry in entries {
        if entry.unstaged == Some(GitFileStatus::Untracked) {
            counts.untracked += 1;
            continue;
        }

        counts.tracked += 1;

        if entry.staged == Some(GitFileStatus::Conflicted) {
            counts.conflicts.push(conflict_file(&entry.path, prefix));
        }
    }

    // The files of the project first, each group by path.
    counts
        .conflicts
        .sort_by(|left, right| (!left.in_project, &left.path).cmp(&(!right.in_project, &right.path)));

    counts
}

/// A conflicting `path` (relative to the repository root), relative to the project when inside it.
fn conflict_file(path: &str, prefix: &str) -> ConflictFile {
    match project_relative(path, prefix) {
        Some(relative) => ConflictFile {
            path: relative,
            in_project: true,
        },
        None => ConflictFile {
            path: path.to_owned(),
            in_project: false,
        },
    }
}

/// Paths of the git files that tell whether something is in progress.
struct GitPaths {
    rebase_merge: PathBuf,
    rebase_apply: PathBuf,
    merge_head: PathBuf,
    fetch_head: PathBuf,
}

async fn git_paths(git: &GitCli<'_>) -> Result<GitPaths> {
    let output = git
        .run_ok([
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "rebase-merge",
            "--git-path",
            "rebase-apply",
            "--git-path",
            "MERGE_HEAD",
            "--git-path",
            "FETCH_HEAD",
        ])
        .await?;
    let mut lines = output.stdout.lines().map(PathBuf::from);
    let mut next = || lines.next().unwrap_or_default();

    Ok(GitPaths {
        rebase_merge: next(),
        rebase_apply: next(),
        merge_head: next(),
        fetch_head: next(),
    })
}

/// A rebase or merge that is stopped, as git's bookkeeping files describe it.
struct Progress {
    kind: GitOperationKind,
    /// The branch a rebase works on (`None` for a merge or a detached HEAD).
    branch: Option<String>,
    /// Full sha of what is applied onto or merged in.
    onto: Option<String>,
}

fn read_progress(paths: &GitPaths) -> Option<Progress> {
    if paths.rebase_merge.is_dir() {
        return Some(rebase_progress(&paths.rebase_merge));
    }

    // The `apply` backend serves `git am` too; only `rebasing` marks a rebase.
    if paths.rebase_apply.join("rebasing").is_file() {
        return Some(rebase_progress(&paths.rebase_apply));
    }

    if paths.merge_head.is_file() {
        return Some(Progress {
            kind: GitOperationKind::Merge,
            branch: None,
            onto: read_small(&paths.merge_head)
                .and_then(|text| text.lines().next().map(str::to_owned))
                .filter(|onto| is_object_id(onto)),
        });
    }

    None
}

fn rebase_progress(dir: &Path) -> Progress {
    let branch = read_small(&dir.join("head-name"))
        .and_then(|name| name.strip_prefix("refs/heads/").map(str::to_owned))
        .filter(|name| validate_branch_name(name).is_ok());

    Progress {
        kind: GitOperationKind::Rebase,
        branch,
        onto: read_small(&dir.join("onto")).filter(|onto| is_object_id(onto)),
    }
}

/// True for a full object id (40 or 64 hexadecimal digits).
pub(super) fn is_object_id(text: &str) -> bool {
    matches!(text.len(), 40 | 64) && text.chars().all(|character| character.is_ascii_hexdigit())
}

/// The trimmed text of a small bookkeeping file; `None` when it is missing or unreadable.
fn read_small(path: &Path) -> Option<String> {
    let text = SafeReader::read_to_string(path, MAX_STATE_FILE_BYTES).ok()?;

    Some(text.trim().to_owned())
}

/// `onto` as shown to the user: the main branch's name when it points there, else a short sha.
fn describe_onto(onto: Option<&str>, main: Option<&FoundMain>) -> Option<String> {
    let onto = onto.filter(|sha| !sha.is_empty())?;

    match main {
        Some(found) if found.oid == onto => Some(found.name.clone()),
        _ => Some(onto.chars().take(SHORT_SHA).collect()),
    }
}

fn modified(path: &Path) -> Option<String> {
    let time: SystemTime = path.metadata().ok()?.modified().ok()?;

    Some(rfc3339(time))
}

/// What the app's own newest stash is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StashKind {
    /// Made by `rebase --autostash` or `merge --autostash`.
    Autostash,
    /// Stored again after its changes were applied with conflicts.
    Applied,
    /// Made by the app before a switch.
    Made,
}

/// The newest stash when this app made it or it is an autostash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OurStash {
    pub(super) oid: String,
    pub(super) kind: StashKind,
}

/// The newest stash when this app made it or it is an autostash.
pub(super) async fn our_top_stash(git: &GitCli<'_>) -> Result<Option<OurStash>> {
    let output = git.run(["stash", "list", "-1", "--format=%H %gs"]).await?;

    if !output.success() {
        return Ok(None);
    }

    let line = output.stdout.trim();

    Ok(line.split_once(' ').and_then(|(oid, subject)| {
        Some(OurStash {
            oid: oid.to_owned(),
            kind: our_stash_kind(subject)?,
        })
    }))
}

/// `subject` is a stash reflog subject (`On main: Harness Desktop: ...`, `autostash`); `None` when
/// the stash is not the app's.
fn our_stash_kind(subject: &str) -> Option<StashKind> {
    let message = match subject.strip_prefix("On ").and_then(|rest| rest.split_once(": ")) {
        Some((_, message)) => message,
        None => subject,
    };

    if message == AUTOSTASH_SUBJECT {
        Some(StashKind::Autostash)
    } else if message == APPLIED_STASH_MESSAGE {
        Some(StashKind::Applied)
    } else if message.starts_with(STASH_PREFIX) {
        Some(StashKind::Made)
    } else {
        None
    }
}
