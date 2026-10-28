//! `git_fetch` and `git_sync_action`: fetch, push, publish, pull, rebase, merge, abort, continue
//! and apply or drop the stash left behind.
//!
//! These are the app's git writes, run when the user clicks (a fetch also in the background).
//! A clicked action runs `git` as a terminal would, so the repository's own hooks run too. Every
//! write command gets [`WRITE_CONFIG`] (no editor opens) and the [`NETWORK_TIMEOUT`], and every
//! revision or ref name comes after `--end-of-options`.
//!
//! A background fetch nobody asked for just now is held to more: it never recurses into submodules,
//! never starts garbage collection, allows only the https, http, ssh, git and file transports, runs no hook (`core.hooksPath` is
//! the `git` program's own path, under which no hook can exist, and the config-based `reference-transaction` hooks are disabled), never asks for credentials (git's own prompt, `GIT_ASKPASS`, the credential
//! manager) and runs `ssh` with `BatchMode=yes` unless `core.sshCommand` names a command. It does
//! not run at all while the repository's own configuration names a program or redirects the
//! remote (see [`PROGRAM_CONFIG`]): only a click runs those.
//!
//! How a command ended is read from its exit code and the repository afterwards: a rebase or merge
//! that left an operation behind is `Stopped`, changes in the way are `BlockedByChanges`, an
//! autostash that did not apply cleanly is `StashKept`, and its entry is then stored again under the
//! message "applied with conflicts" (the changes are in the tree), which `DropStash` removes once
//! the conflicts are resolved. Failures of every write are cut to git's
//! first error line, with URL userinfo and credential-like text removed.

use super::branch_ops::validate_remote_name;
use super::cli::{GitCli, NETWORK_TIMEOUT, RunOpts, SAFE_PROTOCOLS, WRITE_CONFIG, redacted_failure};
use super::repo_state::{StashKind, our_top_stash, remotes, sync_state};
use super::stash::{apply_stash, drop_applied_stash, relabelled};
use crate::error::{BusinessError, Result};
use crate::model::{ActionOutcome, BranchSync, GitActionResult, GitOperationKind, SyncAction};
use crate::process::ExecOutput;

/// `ssh` options of a background fetch: fail instead of asking for a password or a host key.
const BATCH_SSH: &str = "ssh -o BatchMode=yes";

/// Environment of a background fetch: no askpass program (git's or `ssh`'s), no interactive
/// credential manager, and only the transports git itself speaks (a repository-local `evil::x`
/// URL would otherwise run a `git-remote-evil` helper found on `PATH`).
const BACKGROUND_ENV: [(&str, &str); 4] = [
    ("GIT_ASKPASS", ""),
    ("GCM_INTERACTIVE", "never"),
    ("GIT_ALLOW_PROTOCOL", SAFE_PROTOCOLS),
    ("SSH_ASKPASS_REQUIRE", "never"),
];

/// Keys (lowercase, as `git config --get-regexp` matches them) of the repository's own
/// configuration that run a program (`hook.<name>.command` included) or redirect or rewrite a remote. A background fetch does not
/// run while the repository (or its worktree) sets any of them, or includes another file that
/// could. `core.hooksPath` is not one of them: the fetch overrides it itself, and
/// tools such as husky set it in most repositories, which would stop the fetch for good.
const PROGRAM_CONFIG: &str = r"^(core\.(sshcommand|askpass|gitproxy|alternaterefscommand)|credential\..*|remote\..*\.(uploadpack|receivepack|vcs|proxy|proxyauthmethod)|http\..*|url\..*\.(insteadof|pushinsteadof)|protocol\..*|hook\..*|include\..*|includeif\..*)$";

/// Texts with which git says the work tree is in the way.
const BLOCKED_BY_CHANGES: [&str; 4] = [
    "would be overwritten",
    "Please commit your changes or stash them",
    "You have unstaged changes",
    "Your local changes",
];

/// Texts (whitespace collapsed) with which git says an autostash did not apply cleanly: older
/// versions word it one way, newer ones the other.
const AUTOSTASH_CONFLICTS: [&str; 2] = [
    "Applying autostash resulted in conflicts",
    "applying them resulted in conflicts",
];

/// Texts with which git says there was nothing to do.
const UP_TO_DATE: [&str; 3] = ["Already up to date", "Everything up-to-date", "is up to date"];

/// The time limit of a command that writes.
pub(super) fn write_opts() -> RunOpts {
    RunOpts::timeout(NETWORK_TIMEOUT)
}

/// `InvalidInput` unless the project is inside a git repository.
pub(super) fn require_repo(state: &BranchSync) -> Result<()> {
    if state.is_repo {
        return Ok(());
    }

    Err(BusinessError::InvalidInput(
        "this folder is not a git repository".to_owned(),
    ))
}

/// Fetches the sync remote, and the remote the main branch comes from when that is another one.
/// `background` is a fetch nobody asked for just now: it is hardened (see the module docs) and
/// returns the unchanged state without fetching while the repository's own configuration names a
/// program. Returns the state afterwards.
pub async fn git_fetch(git: &GitCli<'_>, picked: Option<&str>, background: bool) -> Result<BranchSync> {
    let state = sync_state(git, picked).await?;
    require_repo(&state)?;

    let remote = state
        .remote
        .clone()
        .ok_or_else(|| BusinessError::InvalidInput("this project has no remote".to_owned()))?;
    validate_remote_name(&remote)?;

    let mut targets = vec![remote];
    if let Some(main_remote) = main_remote(git, &state).await?
        && !targets.contains(&main_remote)
    {
        validate_remote_name(&main_remote)?;
        targets.push(main_remote);
    }

    let mut args: Vec<String> = Vec::new();
    let mut opts = RunOpts::timeout(NETWORK_TIMEOUT);

    if background {
        if configuration_runs_programs(git).await? {
            return Ok(state);
        }

        // A path git would be handed in a lossy form is not the program any more: no fetch then.
        let Some(program) = git.program().to_str() else {
            return Ok(state);
        };
        args.extend(background_config(program));
        opts.env
            .extend(BACKGROUND_ENV.map(|(name, value)| (name.to_owned(), value.to_owned())));

        if !ssh_command_is_configured(git).await? {
            opts.env.push(("GIT_SSH_COMMAND".to_owned(), BATCH_SSH.to_owned()));
        }
    }

    args.push("fetch".to_owned());
    if background {
        args.push("--no-recurse-submodules".to_owned());
    }
    args.extend(["--multiple".to_owned(), "--end-of-options".to_owned()]);
    args.extend(targets);

    git.run_network(args, &opts).await?;

    sync_state(git, picked).await
}

/// The `-c` options of a background fetch. The hooks path is the `git` program itself: an absolute
/// path to a file, so no hook can sit below it whatever the repository holds. Windows' `NUL` would be
/// read as a `NUL` directory at the top of the work tree.
fn background_config(git_program: &str) -> Vec<String> {
    [
        "gc.auto=0",
        "maintenance.auto=false",
        "protocol.ext.allow=never",
        "credential.interactive=false",
        "hook.reference-transaction.enabled=false",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain([format!("core.hooksPath={git_program}")])
    .flat_map(|setting| ["-c".to_owned(), setting])
    .collect()
}

/// True when the repository's own configuration (or its worktree's) sets a key of
/// [`PROGRAM_CONFIG`]. A configuration git cannot read counts as one that does.
async fn configuration_runs_programs(git: &GitCli<'_>) -> Result<bool> {
    let mut scopes = vec!["--local"];

    // `--worktree` fails (exit 128) in a repository with linked work trees unless
    // `extensions.worktreeConfig` is on, and without it there is no worktree file to read.
    let extension = git
        .run(["config", "--local", "--type=bool", "--get", "extensions.worktreeConfig"])
        .await?;

    match extension.status {
        Some(1) => {}
        Some(0) if extension.stdout.trim() == "true" => scopes.push("--worktree"),
        Some(0) => {}
        _ => return Ok(true),
    }

    for scope in scopes {
        let output = git.run(["config", scope, "--get-regexp", PROGRAM_CONFIG]).await?;

        // Exit code 1 means no such key is set.
        if output.status != Some(1) {
            return Ok(true);
        }
    }

    Ok(false)
}

/// The remote the main branch's reference lives on (`upstream` for `upstream/main`).
async fn main_remote(git: &GitCli<'_>, state: &BranchSync) -> Result<Option<String>> {
    let Some(main) = &state.main else {
        return Ok(None);
    };

    let remotes = remotes(git).await?;

    Ok(remotes
        .into_iter()
        .filter(|remote| {
            main.reference
                .strip_prefix(remote.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(String::len))
}

async fn ssh_command_is_configured(git: &GitCli<'_>) -> Result<bool> {
    let output = git.run(["config", "--get", "core.sshCommand"]).await?;

    Ok(output.success())
}

/// Runs `action` and reports how it ended with the state afterwards.
pub async fn git_sync_action(git: &GitCli<'_>, picked: Option<&str>, action: SyncAction) -> Result<GitActionResult> {
    let state = sync_state(git, picked).await?;
    require_repo(&state)?;

    let resolving = matches!(action, SyncAction::Abort | SyncAction::Continue);
    if state.operation.is_some() && !resolving {
        return Err(BusinessError::InvalidInput(
            "finish or abort the rebase or merge first".to_owned(),
        ));
    }

    let (args, ending) = match action {
        SyncAction::Push => (push_args(&state)?, Ending::Plain),
        SyncAction::Publish => (publish_args(&state)?, Ending::Plain),
        SyncAction::Pull => (pull_args(&state)?, Ending::Plain),
        SyncAction::Rebase => (
            integrate_args(&state, "rebase", &["--autostash"])?,
            Ending::StartsOperation,
        ),
        SyncAction::Merge => (
            integrate_args(&state, "merge", &["--autostash", "--no-edit"])?,
            Ending::StartsOperation,
        ),
        SyncAction::Abort => (abort_args(&state)?, Ending::Strict),
        SyncAction::Continue => continue_args(&state)?,
        SyncAction::ApplyStash => return apply_stash(git, picked, &state).await,
        SyncAction::DropStash => return drop_applied_stash(git, picked, &state).await,
    };

    let args = WRITE_CONFIG.into_iter().map(str::to_owned).chain(args);
    let output = git.run_with(args, &write_opts()).await?;
    let result = conclude(git, picked, &state, output, ending).await?;

    // An autostash that conflicted stays in the list: mark it as applied.
    let autostash = matches!(action, SyncAction::Rebase | SyncAction::Merge | SyncAction::Continue);
    if autostash && result.outcome == ActionOutcome::StashKept {
        let top = our_top_stash(git).await?;

        return relabelled(
            git,
            picked,
            result,
            top.filter(|stash| stash.kind == StashKind::Autostash),
        )
        .await;
    }

    Ok(result)
}

/// What a failing write command may mean besides a plain failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ending {
    /// A rebase or merge that starts: it may stop on conflicts.
    StartsOperation,
    /// `rebase --continue`: it may stop again at a later commit.
    ContinuesRebase,
    /// Changes in the way and a stash that did not apply cleanly have outcomes of their own.
    Plain,
    /// Abort and the merge commit: a failure (a hook that said no, a lock) is always an error.
    Strict,
}

/// The result of a finished write command (see the module docs). `before` is the state the
/// command started from.
pub(super) async fn conclude(
    git: &GitCli<'_>,
    picked: Option<&str>,
    before: &BranchSync,
    output: ExecOutput,
    ending: Ending,
) -> Result<GitActionResult> {
    let state = sync_state(git, picked).await?;
    let text = format!("{}\n{}", output.stdout, output.stderr);
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mentions = |needles: &[&str]| needles.iter().any(|needle| collapsed.contains(needle));
    let lenient = ending != Ending::Strict;

    let outcome = if output.success() {
        if mentions(&AUTOSTASH_CONFLICTS) {
            ActionOutcome::StashKept
        } else if mentions(&UP_TO_DATE) {
            ActionOutcome::UpToDate
        } else {
            ActionOutcome::Done
        }
    } else if stopped(before, &state, ending) {
        ActionOutcome::Stopped
    } else if lenient && mentions(&BLOCKED_BY_CHANGES) {
        ActionOutcome::BlockedByChanges
    } else if lenient && state.stash_kept && collapsed.contains("CONFLICT") {
        ActionOutcome::StashKept
    } else {
        return Err(redacted_failure(&output));
    };

    Ok(GitActionResult { outcome, state })
}

/// True when a command that failed left a rebase or merge waiting for the user: one that was not
/// in progress before, or (after `rebase --continue`) one that moved on to another commit or
/// other conflicts. The same stop as before means the command itself was refused, e.g. by a hook.
fn stopped(before: &BranchSync, after: &BranchSync, ending: Ending) -> bool {
    let Some(operation) = &after.operation else {
        return false;
    };

    match ending {
        Ending::StartsOperation => before.operation.is_none(),
        Ending::ContinuesRebase => {
            let earlier = before.operation.as_ref().map(|earlier| &earlier.conflicts);

            before.head != after.head || earlier != Some(&operation.conflicts)
        }
        Ending::Plain | Ending::Strict => false,
    }
}

fn strings<const N: usize>(parts: [&str; N]) -> Vec<String> {
    parts.map(str::to_owned).to_vec()
}

fn require_branch(state: &BranchSync) -> Result<&str> {
    state
        .branch
        .as_deref()
        .ok_or_else(|| BusinessError::InvalidInput("HEAD is not on a branch".to_owned()))
}

fn require_upstream(state: &BranchSync) -> Result<()> {
    require_branch(state)?;

    if state.upstream.is_some() {
        return Ok(());
    }

    Err(BusinessError::InvalidInput(
        "this branch is not on the remote yet".to_owned(),
    ))
}

fn push_args(state: &BranchSync) -> Result<Vec<String>> {
    require_upstream(state)?;

    Ok(strings(["push"]))
}

fn publish_args(state: &BranchSync) -> Result<Vec<String>> {
    let branch = require_branch(state)?;
    super::validate_branch_name(branch)?;

    let remote = state
        .remote
        .as_deref()
        .ok_or_else(|| BusinessError::InvalidInput("this project has no remote".to_owned()))?;
    validate_remote_name(remote)?;

    Ok(strings(["push", "-u", "--end-of-options", remote, branch]))
}

fn pull_args(state: &BranchSync) -> Result<Vec<String>> {
    require_upstream(state)?;

    Ok(strings(["pull", "--ff-only", "--no-rebase"]))
}

/// `git <verb> <flags> <main reference>` for the current branch.
fn integrate_args(state: &BranchSync, verb: &str, flags: &[&str]) -> Result<Vec<String>> {
    require_branch(state)?;

    let main = state
        .main
        .as_ref()
        .ok_or_else(|| BusinessError::InvalidInput("this project has no main branch".to_owned()))?;

    if main.is_current {
        return Err(BusinessError::InvalidInput(
            "the main branch is already checked out".to_owned(),
        ));
    }

    let mut args = vec![verb.to_owned()];
    args.extend(flags.iter().map(|flag| (*flag).to_owned()));
    args.extend(["--end-of-options".to_owned(), main.reference.clone()]);

    Ok(args)
}

fn abort_args(state: &BranchSync) -> Result<Vec<String>> {
    match state.operation.as_ref().map(|operation| operation.kind) {
        Some(GitOperationKind::Rebase) => Ok(strings(["rebase", "--abort"])),
        Some(GitOperationKind::Merge) => Ok(strings(["merge", "--abort"])),
        None => Err(BusinessError::InvalidInput("no rebase or merge to abort".to_owned())),
    }
}

fn continue_args(state: &BranchSync) -> Result<(Vec<String>, Ending)> {
    let Some(operation) = &state.operation else {
        return Err(BusinessError::InvalidInput("no rebase or merge to continue".to_owned()));
    };

    if !operation.conflicts.is_empty() {
        return Err(BusinessError::InvalidInput(
            "resolve the conflicting files first".to_owned(),
        ));
    }

    match operation.kind {
        GitOperationKind::Rebase => Ok((strings(["rebase", "--continue"]), Ending::ContinuesRebase)),
        GitOperationKind::Merge => Ok((strings(["commit", "--no-edit"]), Ending::Strict)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::Env;
    use crate::process::resolve_binary;

    /// Runs on every OS, Windows included: the hooks path is the absolute path of a file, never a
    /// relative name such as `NUL` that git would look up in the work tree.
    #[test]
    fn the_background_hooks_path_is_the_absolute_git_program() {
        let env = Env::from_process().unwrap();
        let Some(git) = resolve_binary(&env, "git", None) else {
            return;
        };
        let program = git.path.to_str().unwrap();
        let config = background_config(program);

        let hooks = config
            .iter()
            .find_map(|setting| setting.strip_prefix("core.hooksPath="))
            .unwrap();
        assert_eq!(hooks, program);
        assert!(git.path.is_absolute(), "{hooks} is relative");
        assert!(git.path.is_file(), "{hooks} is not a file");
    }
}
