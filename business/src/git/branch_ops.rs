//! Branch names, `git_branch_search` and `git_switch`.
//!
//! The search lists branches in one `for-each-ref` and filters in Rust, so a repository with
//! thousands of branches answers in one process run. A switch reports git's refusal because of
//! uncommitted changes as an outcome (the UI offers "Stash and switch"), every other failure as an
//! error.

use std::collections::HashSet;
use std::time::{Duration, UNIX_EPOCH};

use super::cli::{GitCli, WRITE_CONFIG, redacted_failure};
use super::repo_state::{STASH_PREFIX, remotes, sync_state};
use super::stash::{apply_stash_commit, drop_stash_commit};
use super::sync::{Ending, conclude, require_repo, write_opts};
use crate::agent::rfc3339;
use crate::error::{BusinessError, Result};
use crate::model::{
    ActionOutcome, BranchPage, BranchRow, BranchStart, GitActionResult, MAX_BRANCH_QUERY_BYTES, MAX_BRANCH_SEARCH,
    SwitchRequest,
};

/// Longest accepted branch name.
const MAX_BRANCH_NAME_BYTES: usize = 255;

/// Characters git does not allow in a ref name (besides control characters and space).
const FORBIDDEN_CHARS: [char; 6] = ['~', '^', ':', '?', '*', '['];

/// Fields: ref name, HEAD marker, committer date (unix).
const FORMAT: &str = "--format=%(refname)%00%(HEAD)%00%(committerdate:unix)";

/// Checks `name` against git's branch name rules (a stricter subset) so that no name is taken as a
/// git option or revision expression; `InvalidInput` with the reason otherwise.
pub fn validate_branch_name(name: &str) -> Result<()> {
    match ref_name_problem(name) {
        Some(reason) => Err(BusinessError::InvalidInput(format!("invalid branch name: {reason}"))),
        None => Ok(()),
    }
}

/// The same rules for a remote name, which ends up in `refs/remotes/<remote>/<branch>` and on
/// command lines: `InvalidInput` with the reason otherwise.
pub(super) fn validate_remote_name(name: &str) -> Result<()> {
    match ref_name_problem(name) {
        Some(reason) => Err(BusinessError::InvalidInput(format!("invalid remote name: {reason}"))),
        None => Ok(()),
    }
}

/// Why `name` is not a safe ref name component sequence, if it is not.
fn ref_name_problem(name: &str) -> Option<&'static str> {
    if name.is_empty() || name.len() > MAX_BRANCH_NAME_BYTES {
        return Some("it must have 1 to 255 bytes");
    }

    if name.starts_with('-') || name == "HEAD" || name == "@" {
        return Some("it must not start with `-` or be HEAD");
    }

    if name
        .chars()
        .any(|character| character.is_control() || character == ' ' || character == '\\')
    {
        return Some("it must not contain spaces, control characters or backslashes");
    }

    if name.contains(FORBIDDEN_CHARS) || name.contains("..") || name.contains("@{") || name.contains("//") {
        return Some("it contains a sequence git does not allow");
    }

    if name.starts_with('/') || name.ends_with('/') || name.ends_with('.') {
        return Some("it must not start or end with `/` or end with `.`");
    }

    if name
        .split('/')
        .any(|part| part.starts_with('.') || part.ends_with(".lock"))
    {
        return Some("no part may start with `.` or end with `.lock`");
    }

    None
}

/// Branches of the project's repository matching `query` (case-insensitive substring of the name):
/// local ones first, then remote-only ones (short names without the remote), newest commit first.
/// The page starts at match `offset` (0-based) and holds at most `limit` (1 to
/// [`MAX_BRANCH_SEARCH`]) of them, `total` counts all matches (an `offset` past the last one gives
/// an empty page). Empty outside a work tree.
pub async fn git_branch_search(git: &GitCli<'_>, query: &str, offset: u32, limit: u32) -> Result<BranchPage> {
    if query.len() > MAX_BRANCH_QUERY_BYTES {
        return Err(BusinessError::InvalidInput(format!(
            "the branch search is limited to {MAX_BRANCH_QUERY_BYTES} bytes"
        )));
    }

    if git.prefix().await?.is_none() {
        return Ok(BranchPage {
            branches: Vec::new(),
            total: 0,
        });
    }

    let limit = usize::try_from(limit)
        .unwrap_or(MAX_BRANCH_SEARCH)
        .clamp(1, MAX_BRANCH_SEARCH);
    let remotes = remotes(git).await?;
    let output = git
        .run_ok([
            "for-each-ref",
            "--sort=-committerdate",
            FORMAT,
            "refs/heads",
            "refs/remotes",
        ])
        .await?;

    let query = query.to_lowercase();
    let matches = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);

    let mut local = Vec::new();
    let mut remote_only = Vec::new();

    for line in output.stdout.lines() {
        let fields: Vec<&str> = line.split('\0').collect();
        let [refname, head, date] = fields.as_slice() else {
            continue;
        };

        if let Some(name) = refname.strip_prefix("refs/heads/") {
            if matches(name) {
                local.push(row(name, *head == "*", false, date));
            }
        } else if let Some(name) = remote_branch_name(refname, &remotes)
            && matches(name)
        {
            remote_only.push(row(name, false, true, date));
        }
    }

    // A remote branch with a local twin (or on a second remote) is listed once.
    let mut seen: HashSet<String> = local.iter().map(|row| row.name.clone()).collect();
    remote_only.retain(|row| seen.insert(row.name.clone()));
    local.append(&mut remote_only);

    let total = u32::try_from(local.len()).unwrap_or(u32::MAX);
    let offset = usize::try_from(offset).unwrap_or(usize::MAX);
    let branches = local.into_iter().skip(offset).take(limit).collect();

    Ok(BranchPage { branches, total })
}

/// The branch name of a remote-tracking `refname`, without the remote; `None` for a remote's `HEAD`.
fn remote_branch_name<'a>(refname: &'a str, remotes: &[String]) -> Option<&'a str> {
    let rest = refname.strip_prefix("refs/remotes/")?;

    let name = remotes
        .iter()
        .filter_map(|remote| rest.strip_prefix(remote.as_str())?.strip_prefix('/'))
        .min_by_key(|name| name.len())
        .or_else(|| rest.split_once('/').map(|(_, name)| name))?;

    (name != "HEAD" && !name.ends_with("/HEAD")).then_some(name)
}

fn row(name: &str, is_current: bool, remote_only: bool, date: &str) -> BranchRow {
    let last_commit_date = date
        .parse::<u64>()
        .ok()
        .map(|seconds| rfc3339(UNIX_EPOCH + Duration::from_secs(seconds)));

    BranchRow {
        name: name.to_owned(),
        is_current,
        remote_only,
        last_commit_date,
    }
}

/// Switches to `request.branch`, creating it first when `request.create` says so, optionally
/// stashing all changes (untracked files too) before. A branch only a remote has is created
/// tracking it (git's own guess). Changes in the way end as [`ActionOutcome::BlockedByChanges`].
///
/// When the switch fails after the stash was made, the stash is put back by its id (see
/// [`restore_stash`]); if that fails the stash is left alone and the result says so.
pub async fn git_switch(git: &GitCli<'_>, picked: Option<&str>, request: &SwitchRequest) -> Result<GitActionResult> {
    validate_branch_name(&request.branch)?;

    let before = sync_state(git, picked).await?;
    require_repo(&before)?;

    let mut args = vec!["switch".to_owned()];
    match request.create {
        Some(BranchStart::Main) => {
            let main = before
                .main
                .as_ref()
                .ok_or_else(|| BusinessError::InvalidInput("this project has no main branch".to_owned()))?;

            args.extend(["--no-track".to_owned(), "-c".to_owned(), request.branch.clone()]);
            args.extend(["--end-of-options".to_owned(), main.reference.clone()]);
        }
        Some(BranchStart::Current) => {
            args.extend(["--no-track".to_owned(), "-c".to_owned(), request.branch.clone()]);
        }
        None => args.extend(["--end-of-options".to_owned(), request.branch.clone()]),
    }

    let stash = if request.stash {
        stash_all(git, &request.branch).await?
    } else {
        None
    };
    let output = git
        .run_with(WRITE_CONFIG.into_iter().map(str::to_owned).chain(args), &write_opts())
        .await;

    // Whether the switch worked or not (a timeout or a spawn error included), the changes only stay
    // in the stash when it could not be put back.
    let failed = output.as_ref().map_or(true, |output| !output.success());
    let kept = match &stash {
        Some(oid) if failed => !restore_stash(git, oid).await,
        Some(_) => true,
        None => false,
    };

    let output = output.map_err(|error| with_stash_note(error, kept))?;
    let mut result = conclude(git, picked, &before, output, Ending::Plain)
        .await
        .map_err(|error| with_stash_note(error, kept))?;

    if kept && matches!(result.outcome, ActionOutcome::Done | ActionOutcome::BlockedByChanges) {
        result.outcome = ActionOutcome::StashKept;
    }

    Ok(result)
}

/// `error` saying that the user's changes are in the stash when `kept`.
fn with_stash_note(error: BusinessError, kept: bool) -> BusinessError {
    const NOTE: &str = "Your changes are kept in the stash.";

    match error {
        BusinessError::Process(message) if kept => BusinessError::Process(format!("{message} {NOTE}")),
        BusinessError::Timeout(message) if kept => BusinessError::Timeout(format!("{message}. {NOTE}")),
        other => other,
    }
}

/// Stashes every change, untracked files included; the id of the new stash, `None` when there was
/// nothing to stash.
///
/// Literal pathspecs (on for every command) make `stash push --include-untracked` leave the
/// untracked files in place, so they are switched off here: no pathspec is passed.
async fn stash_all(git: &GitCli<'_>, target: &str) -> Result<Option<String>> {
    let message = format!("{STASH_PREFIX} before switching to {target}");
    let args = WRITE_CONFIG.into_iter().chain([
        "--no-literal-pathspecs",
        "stash",
        "push",
        "--include-untracked",
        "-m",
        &message,
    ]);

    let output = git.run_with(args, &write_opts()).await?;

    if !output.success() {
        return Err(redacted_failure(&output));
    }

    if output.stdout.contains("No local changes to save") {
        return Ok(None);
    }

    match newest_stash(git).await {
        Ok(Some(oid)) => Ok(Some(oid)),
        Ok(None) => Err(BusinessError::Process("git made no stash".to_owned())),
        // The push worked: the changes are in the stash, only its id could not be read back.
        Err(_) => Err(BusinessError::Process(
            "The branch was not switched. Your changes are kept in the stash.".to_owned(),
        )),
    }
}

/// The id of the newest stash; `None` when there is none. (`stash@{0}` is a fixed literal, so no
/// `--end-of-options`, which older `rev-parse` versions do not know.)
async fn newest_stash(git: &GitCli<'_>) -> Result<Option<String>> {
    let output = git.run(["rev-parse", "--verify", "--quiet", "stash@{0}"]).await?;

    Ok(output.success().then(|| output.stdout.trim().to_owned()))
}

/// Puts the stash `oid` back after a failed switch: it is applied by its id, so a stash someone
/// else made since is never applied in its place, and dropped by a verified drop. False when it
/// stays in the stash (it did not apply cleanly or could not be dropped safely).
async fn restore_stash(git: &GitCli<'_>, oid: &str) -> bool {
    match apply_stash_commit(git, oid).await {
        Ok(output) if output.success() => matches!(drop_stash_commit(git, oid).await, Ok(true)),
        _ => false,
    }
}
