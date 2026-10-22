//! The project repository through the `git` CLI: status, branches, working-tree diffs and
//! discarding changes.
//!
//! Every command runs through [`GitCli`] (shell-free, hardened, time- and size-limited). Paths
//! in results are relative to the project, which may be a subdirectory of the work tree; a
//! project outside any repository reports `isRepo: false` (and no branches or diffs) instead of
//! an error.
//!
//! # Residual risk
//!
//! The hardening switches off the fsmonitor hook, external diff drivers, textconv filters and
//! pagers, but a repository's own `.git/config` can still name clean/smudge `filter` drivers in
//! `.gitattributes`, and git may run them while `status` or `diff` re-reads a changed file, as
//! in a terminal. Only open repositories you trust.

mod branches;
mod cli;
mod diff_parse;
mod discard;
mod file_lines;
mod status;
mod untracked;

pub use branches::parse_branches;
pub use cli::{GIT_OUTPUT_CAP, GIT_TIMEOUT, GitCli};
pub use diff_parse::{DiffBudget, parse_diff, unquote};
pub use file_lines::git_file_lines;
pub use status::{not_a_repo, parse_status};

use std::path::{Component, Path};

use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::model::{Branch, DiffRequest, DiffScope, FileDiff, GitStatus};
use crate::process::ProcessRunner;
use crate::security::deny::is_denied_under_claude_root;
use crate::security::is_denied;

/// Paths accepted at most in one [`DiffRequest`].
pub const MAX_DIFF_PATHS: usize = 1000;

/// Untracked files diffed at most per request.
pub const MAX_UNTRACKED_FILES: usize = 2000;

/// Longest accepted path in a [`DiffRequest`].
const MAX_PATH_BYTES: usize = 4096;

/// `git_status` for the canonical project directory `project`.
pub async fn git_status(env: &Env, runner: &dyn ProcessRunner, project: &Path) -> Result<GitStatus> {
    status::git_status(&GitCli::new(env, runner, project)?).await
}

/// `git_branches` for the canonical project directory `project`.
pub async fn git_branches(env: &Env, runner: &dyn ProcessRunner, project: &Path) -> Result<Vec<Branch>> {
    branches::git_branches(&GitCli::new(env, runner, project)?).await
}

/// `git_discard_all` for the canonical project directory `project`.
pub async fn git_discard_all(env: &Env, runner: &dyn ProcessRunner, project: &Path) -> Result<()> {
    discard::git_discard_all(&GitCli::new(env, runner, project)?).await
}

/// `git_discard_files` for the canonical project directory `project` and its project-relative `paths`.
pub async fn git_discard_files(env: &Env, runner: &dyn ProcessRunner, project: &Path, paths: &[String]) -> Result<()> {
    discard::git_discard_files(&GitCli::new(env, runner, project)?, paths).await
}

/// `git_diff` for the canonical project directory `project`: staged, then unstaged, then
/// untracked changes, as `request.scope` selects, limited to `request.paths` when given.
pub async fn git_diff(
    env: &Env,
    runner: &dyn ProcessRunner,
    project: &Path,
    request: &DiffRequest,
) -> Result<Vec<FileDiff>> {
    validate_paths(&request.paths)?;

    let git = GitCli::new(env, runner, project)?;
    if git.prefix().await?.is_none() {
        return Ok(Vec::new());
    }

    let (staged, unstaged, untracked) = match request.scope {
        DiffScope::Staged => (true, false, false),
        DiffScope::Unstaged => (false, true, false),
        DiffScope::Untracked => (false, false, true),
        DiffScope::All => (true, true, true),
    };

    let mut budget = DiffBudget::default();
    let mut files = Vec::new();

    for (wanted, cached) in [(staged, true), (unstaged, false)] {
        if !wanted {
            continue;
        }

        let mut args = vec![
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "-U3",
            "-M",
            "--relative",
            "--src-prefix=a/",
            "--dst-prefix=b/",
        ];
        if cached {
            args.push("--cached");
        }
        args.push("--");
        args.extend(request.paths.iter().map(String::as_str));

        let output = git.run_ok(args).await?;
        let mut parsed = parse_diff(&output.stdout, cached, &mut budget);
        parsed
            .iter_mut()
            .filter(|file| {
                std::iter::once(&file.path)
                    .chain(&file.old_path)
                    .any(|path| is_denied_in(git.project(), path))
            })
            .for_each(|file| {
                file.hunks.clear();
                file.truncated = true;
            });

        if output.truncated
            && let Some(last) = parsed.last_mut()
        {
            last.truncated = true;
        }
        files.extend(parsed);
    }

    if untracked {
        files.extend(untracked_files(&git, &request.paths, &mut budget).await?);
    }

    Ok(files)
}

async fn untracked_files(git: &GitCli<'_>, paths: &[String], budget: &mut DiffBudget) -> Result<Vec<FileDiff>> {
    let mut args = vec!["ls-files", "--others", "--exclude-standard", "-z", "--"];
    args.extend(paths.iter().map(String::as_str));

    let output = git.run_ok(args).await?;

    let diffs = output
        .stdout
        .split('\0')
        .filter(|path| !path.is_empty())
        .take(MAX_UNTRACKED_FILES)
        .map(|path| untracked::untracked_diff(git.project(), path, budget))
        .collect();

    Ok(diffs)
}

/// True when the project-relative `relative` is deny-listed as written, or when its full or real
/// path lies among the private files of a Claude Code config directory (a `CLAUDE_CONFIG_DIR`
/// inside the project). Only the relative path meets the rules that match any ancestor's name, so
/// a project cloned below a folder such as `projects` or `opencode` keeps its diffs.
pub(crate) fn is_denied_in(project: &Path, relative: &str) -> bool {
    if is_denied(Path::new(relative)).is_some() {
        return true;
    }

    let full = project.join(relative);
    if is_denied_under_claude_root(&full).is_some() {
        return true;
    }

    dunce::canonicalize(&full).is_ok_and(|real| is_denied_under_claude_root(&real).is_some())
}

/// Request paths must be project-relative: not absolute, no `..`, no NUL, at most
/// [`MAX_DIFF_PATHS`] of at most 4 KiB each.
fn validate_paths(paths: &[String]) -> Result<()> {
    if paths.len() > MAX_DIFF_PATHS {
        return Err(BusinessError::InvalidInput(format!(
            "at most {MAX_DIFF_PATHS} paths per diff"
        )));
    }

    for path in paths {
        let relative = Path::new(path);
        let escapes = relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir));

        if path.is_empty() || path.len() > MAX_PATH_BYTES || path.contains('\0') || escapes {
            return Err(BusinessError::InvalidInput(
                "diff paths must be relative to the project".to_owned(),
            ));
        }
    }

    Ok(())
}
