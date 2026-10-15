//! `git_discard_all` and `git_discard_files`: put the project's work tree, or some of its files, back
//! to the last commit.
//!
//! Every pathspec is the project directory (`.` from git's `-C <project>`), so a project in a
//! subdirectory never touches the rest of the repository. Ignored files stay; an untracked nested
//! repository stays too (`clean` removes one only when forced twice).

use super::cli::{GitCli, failure};
use super::validate_paths;
use crate::error::{BusinessError, Result};

/// Reverts every staged and unstaged change under the project to HEAD and deletes its untracked
/// files. Outside a work tree there is nothing to discard.
pub async fn git_discard_all(git: &GitCli<'_>) -> Result<()> {
    if git.prefix().await?.is_none() {
        return Ok(());
    }

    if git.run(["rev-parse", "--verify", "--quiet", "HEAD"]).await?.success() {
        let output = git
            .run(["restore", "--source=HEAD", "--staged", "--worktree", "--", "."])
            .await?;

        // A project holding no tracked file has nothing to restore; git reports that as an error.
        if !output.success() && !output.stderr.contains("did not match any file(s) known to git") {
            return Err(failure(&output));
        }
    } else {
        // No commit yet: whatever is staged becomes untracked again, and the clean below removes it.
        git.run_ok(["rm", "-r", "-q", "--cached", "--ignore-unmatch", "--", "."])
            .await?;
    }

    git.run_ok(["clean", "-f", "-d", "-q", "--", "."]).await?;

    Ok(())
}

/// Puts the project-relative `paths` (a file and, for a rename, its old name) back to HEAD: a file
/// HEAD knows is restored in the index and on disk, any other (untracked or staged as new) is
/// unstaged and deleted. Ignored files are not touched; a path matching nothing is skipped.
pub async fn git_discard_files(git: &GitCli<'_>, paths: &[String]) -> Result<()> {
    validate_paths(paths)?;
    if paths.is_empty() {
        return Err(BusinessError::InvalidInput("no file to discard".to_owned()));
    }

    if git.prefix().await?.is_none() {
        return Ok(());
    }

    let has_head = git.run(["rev-parse", "--verify", "--quiet", "HEAD"]).await?.success();
    let (mut known, mut new) = (Vec::new(), Vec::new());

    for path in paths {
        let in_head = has_head
            && !git
                .run_ok(["ls-tree", "--name-only", "HEAD", "--", path])
                .await?
                .stdout
                .is_empty();

        if in_head {
            known.push(path.clone())
        } else {
            new.push(path.clone())
        }
    }

    if !known.is_empty() {
        let args = ["restore", "--source=HEAD", "--staged", "--worktree", "--"].map(String::from);
        git.run_ok(args.into_iter().chain(known)).await?;
    }

    if !new.is_empty() {
        let unstage = ["rm", "-r", "-q", "--cached", "--ignore-unmatch", "--"].map(String::from);
        git.run_ok(unstage.into_iter().chain(new.iter().cloned())).await?;

        let clean = ["clean", "-f", "-d", "-q", "--"].map(String::from);
        git.run_ok(clean.into_iter().chain(new)).await?;
    }

    Ok(())
}
