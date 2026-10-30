//! `git_status` from `git status --porcelain=v2 --branch -z`, with untracked folders listed file by
//! file (`--untracked-files=all`) while that stays a list a person can read: past
//! [`MAX_LISTED_ENTRIES`] an untracked folder is one entry, as git shows it by default
//! (`node_modules/` of a repository without a `.gitignore` is tens of thousands of files).
//!
//! Porcelain paths are relative to the work-tree root; they are made relative to the project
//! (which may be a subdirectory) and entries outside the project are dropped.

use super::cli::{GitCli, project_relative};
use crate::error::Result;
use crate::model::{GitFileStatus, GitStatus, StatusEntry};

/// Length of the short sha reported for HEAD (matches `git_branches`).
pub const SHORT_SHA: usize = 7;

/// The most entries listed with every untracked file on its own line. The Changes view and the rail
/// read the status every few seconds and show each entry, so a larger list folds untracked folders.
pub const MAX_LISTED_ENTRIES: usize = 1000;

/// The status of the project's repository; `isRepo: false` outside a work tree.
pub async fn git_status(git: &GitCli<'_>) -> Result<GitStatus> {
    let Some(prefix) = git.prefix().await? else {
        return Ok(not_a_repo());
    };

    // An untracked folder is one entry (`dir/`) here: without one, this is the whole list already.
    let folded = git
        .run_ok(["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=normal"])
        .await?;
    let folded = parse_status(&folded.stdout, &prefix);

    if !folded.entries.iter().any(is_untracked_folder) {
        return Ok(folded);
    }

    // Its files one by one, unless that is too many to show (or too much output to read).
    let all = git
        .run_ok(["status", "--porcelain=v2", "--branch", "-z", "--untracked-files=all"])
        .await
        .map(|output| parse_status(&output.stdout, &prefix));

    match all {
        Ok(all) if all.entries.len() <= MAX_LISTED_ENTRIES => Ok(all),
        _ => Ok(folded),
    }
}

/// An untracked folder as git lists it by default: its path ends with `/`.
fn is_untracked_folder(entry: &StatusEntry) -> bool {
    entry.unstaged == Some(GitFileStatus::Untracked) && entry.path.ends_with('/')
}

/// The status of a directory outside any repository.
pub fn not_a_repo() -> GitStatus {
    GitStatus {
        is_repo: false,
        branch: None,
        head: None,
        upstream: None,
        ahead: 0,
        behind: 0,
        entries: Vec::new(),
    }
}

/// Parses porcelain v2 `-z` output; paths are made relative to `prefix` (see the module docs).
pub fn parse_status(raw: &str, prefix: &str) -> GitStatus {
    let mut status = GitStatus {
        is_repo: true,
        ..not_a_repo()
    };
    let mut records = raw.split('\0');

    while let Some(record) = records.next() {
        if let Some(header) = record.strip_prefix("# ") {
            apply_header(&mut status, header);
            continue;
        }

        let entry = match record.split_at_checked(2) {
            Some(("1 ", rest)) => ordinary(rest, 7),
            Some(("2 ", rest)) => {
                let original = records.next().unwrap_or_default();
                ordinary(rest, 8).map(|entry| StatusEntry {
                    orig_path: Some(original.to_owned()),
                    ..entry
                })
            }
            Some(("u ", rest)) => unmerged(rest),
            Some(("? ", path)) => Some(StatusEntry {
                path: path.to_owned(),
                orig_path: None,
                staged: None,
                unstaged: Some(GitFileStatus::Untracked),
            }),
            _ => None,
        };

        if let Some(entry) = entry.and_then(|entry| relative_entry(entry, prefix)) {
            status.entries.push(entry);
        }
    }

    status.entries.sort_by(|left, right| left.path.cmp(&right.path));
    status
}

fn apply_header(status: &mut GitStatus, header: &str) {
    let Some((key, value)) = header.split_once(' ') else {
        return;
    };

    match key {
        "branch.oid" if value != "(initial)" => status.head = Some(value.chars().take(SHORT_SHA).collect()),
        "branch.head" if value != "(detached)" => status.branch = Some(value.to_owned()),
        "branch.upstream" => status.upstream = Some(value.to_owned()),
        "branch.ab" => {
            for part in value.split(' ') {
                if let Some(ahead) = part.strip_prefix('+') {
                    status.ahead = ahead.parse().unwrap_or(0);
                } else if let Some(behind) = part.strip_prefix('-') {
                    status.behind = behind.parse().unwrap_or(0);
                }
            }
        }
        _ => {}
    }
}

/// `XY sub mH mI mW hH hI [Xscore] path` with `fields` space-separated fields before the path.
fn ordinary(rest: &str, fields: usize) -> Option<StatusEntry> {
    let mut parts = rest.splitn(fields + 1, ' ');
    let codes = parts.next()?;
    let path = parts.nth(fields - 1)?;
    let (staged, unstaged) = codes.split_at_checked(1)?;

    Some(StatusEntry {
        path: path.to_owned(),
        orig_path: None,
        staged: file_status(staged),
        unstaged: file_status(unstaged),
    })
}

/// `XY sub m1 m2 m3 mW h1 h2 h3 path`: an unmerged path is conflicted on both sides.
fn unmerged(rest: &str) -> Option<StatusEntry> {
    let path = rest.splitn(10, ' ').nth(9)?;

    Some(StatusEntry {
        path: path.to_owned(),
        orig_path: None,
        staged: Some(GitFileStatus::Conflicted),
        unstaged: Some(GitFileStatus::Conflicted),
    })
}

/// One status letter of a porcelain `XY` pair.
fn file_status(code: &str) -> Option<GitFileStatus> {
    match code {
        "M" => Some(GitFileStatus::Modified),
        "T" => Some(GitFileStatus::TypeChanged),
        "A" => Some(GitFileStatus::Added),
        "D" => Some(GitFileStatus::Deleted),
        "R" => Some(GitFileStatus::Renamed),
        "C" => Some(GitFileStatus::Copied),
        "U" => Some(GitFileStatus::Conflicted),
        _ => None,
    }
}

fn relative_entry(entry: StatusEntry, prefix: &str) -> Option<StatusEntry> {
    let path = project_relative(&entry.path, prefix)?;
    let orig_path = entry
        .orig_path
        .as_deref()
        .and_then(|original| project_relative(original, prefix));

    Some(StatusEntry {
        path,
        orig_path,
        ..entry
    })
}
