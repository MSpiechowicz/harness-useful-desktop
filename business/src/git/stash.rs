//! Applying, relabelling and dropping one exact stash entry (`ApplyStash`, `DropStash`).
//!
//! The stash list is shared with everything else that runs in the repository, so a position
//! (`stash@{0}`) read in one process may name another entry in the next. Both steps therefore go
//! by commit id: the entry is applied by its id, and dropped only after finding its current
//! position and checking, from the id git prints, that the entry removed is the right one.

use super::cli::{GitCli, WRITE_CONFIG};
use super::repo_state::{APPLIED_STASH_MESSAGE, OurStash, StashKind, is_object_id, our_top_stash, sync_state};
use super::sync::{Ending, conclude, write_opts};
use crate::error::{BusinessError, Result};
use crate::model::{ActionOutcome, BranchSync, GitActionResult};
use crate::process::ExecOutput;

/// How many times the drop is tried when another program's stash moves under it.
const DROP_ATTEMPTS: usize = 3;

/// `git stash apply` of the stash commit `oid`: only that entry's changes are applied, whatever
/// the stash list holds now, and the entry stays in the list. Like `stash pop`, the index is not
/// restored (no `--index`).
pub(super) async fn apply_stash_commit(git: &GitCli<'_>, oid: &str) -> Result<ExecOutput> {
    let args = WRITE_CONFIG
        .into_iter()
        .chain(["stash", "apply", "--end-of-options", oid]);

    git.run_with(args, &write_opts()).await
}

/// Drops the stash entry `oid` and nothing else. True when it is no longer in the list; false when
/// it could not be removed safely and is still there (the caller reports the stash as kept).
///
/// `stash drop` takes a position, so a stash another program pushed between listing and dropping
/// would be dropped instead. Git prints the id it dropped: when that is not `oid`, the entry is
/// stored back under its own message and the drop is tried again, [`DROP_ATTEMPTS`] times at most.
/// A stored-back entry goes on top of the list, so the order of other programs' stashes can change.
/// An `Err` means that an entry that is not ours was dropped and could not be stored back: its id
/// is in the message.
pub(super) async fn drop_stash_commit(git: &GitCli<'_>, oid: &str) -> Result<bool> {
    for _ in 0..DROP_ATTEMPTS {
        let Some(list) = list_stashes(git).await else {
            return Ok(false);
        };

        // Not in the list: already gone.
        let Some(position) = list.iter().position(|listed| listed == oid) else {
            return Ok(true);
        };

        let reference = format!("stash@{{{position}}}");
        let args = WRITE_CONFIG.into_iter().chain(["stash", "drop", &reference]);

        let Ok(output) = git.run_with(args, &write_opts()).await else {
            return Ok(false);
        };
        if !output.success() {
            return Ok(false);
        }

        let Some(dropped) = dropped_oid(&output.stdout) else {
            return Ok(false);
        };
        if dropped == oid {
            return Ok(true);
        }

        store_back(git, dropped).await?;
    }

    // Our entry is still listed: better a stash left behind than one of someone else's lost.
    Ok(!is_listed(git, oid).await)
}

/// Replaces the stash entry `oid` by the same commit stored under [`APPLIED_STASH_MESSAGE`], so a
/// stash whose changes were applied with conflicts no longer reads as one still to apply. True when
/// the entry now carries the new message. On false the entry is untouched, or (when only the
/// storing failed) put back under its own message.
pub(super) async fn relabel_applied(git: &GitCli<'_>, oid: &str) -> bool {
    if !is_object_id(oid) {
        return false;
    }

    if !matches!(drop_stash_commit(git, oid).await, Ok(true)) {
        return false;
    }

    // `stash store` does not know `--end-of-options`; `oid` is a checked object id.
    let args = WRITE_CONFIG
        .into_iter()
        .chain(["stash", "store", "-m", APPLIED_STASH_MESSAGE, "--", oid]);

    if matches!(git.run_with(args, &write_opts()).await, Ok(output) if output.success()) {
        return true;
    }

    // Never leave the changes without an entry: store the commit back under its own message.
    let _ = store_back(git, oid).await;

    false
}

/// The commit ids of all stashes, newest first; `None` when git could not list them.
async fn list_stashes(git: &GitCli<'_>) -> Option<Vec<String>> {
    let output = git.run(["stash", "list", "--format=%H"]).await.ok()?;

    if !output.success() {
        return None;
    }

    let list = output
        .stdout
        .lines()
        .map(str::trim)
        .filter(|line| is_object_id(line))
        .map(str::to_owned)
        .collect();

    Some(list)
}

async fn is_listed(git: &GitCli<'_>, oid: &str) -> bool {
    match list_stashes(git).await {
        Some(list) => list.iter().any(|listed| listed == oid),
        None => true,
    }
}

/// The id in `Dropped stash@{n} (<oid>)` (git is run with `LC_ALL=C`).
fn dropped_oid(stdout: &str) -> Option<&str> {
    let (_, tail) = stdout.trim().rsplit_once('(')?;
    let oid = tail.strip_suffix(')')?;

    is_object_id(oid).then_some(oid)
}

/// Puts the dropped stash commit `oid` back in the list. Its message is the subject of the commit
/// (`On main: ...`), which is what the list showed: the entry was not in the list this run read, so
/// the list cannot say.
async fn store_back(git: &GitCli<'_>, oid: &str) -> Result<()> {
    let subject = git
        .run(["show", "-s", "--format=%s", "--end-of-options", oid])
        .await
        .ok()
        .filter(ExecOutput::success)
        .map(|output| output.stdout.trim().to_owned())
        .unwrap_or_default();

    let mut args: Vec<&str> = WRITE_CONFIG.into_iter().chain(["stash", "store"]).collect();

    if !subject.is_empty() {
        args.extend(["-m", &subject]);
    }
    // `stash store` does not know `--end-of-options`; `oid` is a checked object id.
    args.extend(["--", oid]);

    match git.run_with(args, &write_opts()).await {
        Ok(output) if output.success() => Ok(()),
        _ => Err(BusinessError::Process(format!(
            "A stash made meanwhile was removed by mistake and could not be put back. Restore it with git stash store {oid}"
        ))),
    }
}

/// `result` with the state read again when the stash `ours` was relabelled "applied with
/// conflicts"; `result` unchanged when there is no such stash or the relabelling failed.
pub(super) async fn relabelled(
    git: &GitCli<'_>,
    picked: Option<&str>,
    mut result: GitActionResult,
    ours: Option<OurStash>,
) -> Result<GitActionResult> {
    if let Some(stash) = ours
        && relabel_applied(git, &stash.oid).await
    {
        result.state = sync_state(git, picked).await?;
    }

    Ok(result)
}

/// Applies the stash this app (or an autostash) left, exactly the one that was checked, and drops
/// that entry once it applied. The stash is applied by its commit id, so a stash someone made after
/// the check is never applied in its place, and it is dropped by a verified drop (see
/// [`drop_stash_commit`]), so none of someone else's is lost. A stash that cannot be dropped safely
/// stays in the list.
pub(super) async fn apply_stash(git: &GitCli<'_>, picked: Option<&str>, state: &BranchSync) -> Result<GitActionResult> {
    if state.stash_applied {
        return Err(BusinessError::InvalidInput(
            "your changes are already back, drop the copy instead".to_owned(),
        ));
    }

    // An apply onto unresolved conflicts would only pile more on them.
    if state.stash_kept && !state.unmerged.is_empty() {
        return Err(BusinessError::InvalidInput(
            "resolve the conflicting files first".to_owned(),
        ));
    }

    let ours = match our_top_stash(git).await? {
        Some(stash) if state.stash_kept && stash.kind != StashKind::Applied => stash,
        _ => {
            return Err(BusinessError::InvalidInput(
                "the newest stash is not one Harness Desktop made".to_owned(),
            ));
        }
    };

    let output = apply_stash_commit(git, &ours.oid).await?;

    // Conflicts or a refusal keep the stash.
    if output.success() {
        drop_stash_commit(git, &ours.oid).await?;
    }

    let result = conclude(git, picked, state, output, Ending::Plain).await?;

    // Applied onto conflicts: the changes are in the tree, the entry is only a copy now.
    if result.outcome == ActionOutcome::StashKept && !result.state.unmerged.is_empty() {
        return relabelled(git, picked, result, Some(ours)).await;
    }

    Ok(result)
}

/// Drops the stash copy of changes that were applied with conflicts, once none is left. Only the
/// entry that was checked goes, by a verified drop (see [`drop_stash_commit`]).
pub(super) async fn drop_applied_stash(
    git: &GitCli<'_>,
    picked: Option<&str>,
    state: &BranchSync,
) -> Result<GitActionResult> {
    if !state.stash_kept || !state.stash_applied {
        return Err(BusinessError::InvalidInput(
            "there is no applied stash copy to drop".to_owned(),
        ));
    }

    if !state.unmerged.is_empty() {
        return Err(BusinessError::InvalidInput(
            "resolve the conflicting files first".to_owned(),
        ));
    }

    let ours = match our_top_stash(git).await? {
        Some(stash) if stash.kind == StashKind::Applied => stash,
        _ => {
            return Err(BusinessError::InvalidInput(
                "there is no applied stash copy to drop".to_owned(),
            ));
        }
    };

    if !drop_stash_commit(git, &ours.oid).await? {
        return Err(BusinessError::Process("the stash copy could not be dropped".to_owned()));
    }

    Ok(GitActionResult {
        outcome: ActionOutcome::Done,
        state: sync_state(git, picked).await?,
    })
}
