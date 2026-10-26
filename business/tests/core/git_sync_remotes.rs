//! Remote and branch names that git or a shell could misread never reach a git command line as
//! anything but a name, and bookkeeping files of a stopped rebase are not trusted either.

use crate::git_support;

use std::fs;

use git_support::{GitHome, Repo, git_home};
use harness_business::git::{git_switch, git_sync_action, git_sync_state};
use harness_business::model::{ActionOutcome, BranchStart, SwitchRequest, SyncAction};
use pretty_assertions::assert_eq;

/// A remote name that, taken for an option of `git rebase`, runs a command in the work tree.
const EVIL_REMOTE: &str = "--exec=touch${IFS}PWNED;true";

/// A repository on `topic` (one commit of its own) whose `main` moved on, with a remote named
/// [`EVIL_REMOTE`] that holds a `main`, is the remote of `topic` and has no default branch.
fn repo_with_evil_remote(home: &GitHome, name: &str) -> Repo {
    let mut repo = home.repo(name);
    repo.commit_file("a.txt", "a\n");
    repo.git(&["switch", "-q", "-c", "topic"]);
    repo.commit_file("t.txt", "t\n");
    repo.git(&["switch", "-q", "main"]);
    repo.commit_file("m.txt", "m\n");
    repo.git(&["switch", "-q", "topic"]);

    repo.git(&["config", &format!("remote.{EVIL_REMOTE}.url"), "/nowhere"]);
    repo.git(&["config", "branch.topic.remote", EVIL_REMOTE]);
    repo.git(&["update-ref", &format!("refs/remotes/{EVIL_REMOTE}/main"), "main"]);
    repo
}

fn assert_nothing_ran(repo: &Repo) {
    assert!(
        !repo.path.join("PWNED").exists(),
        "the remote name was run as a command"
    );
}

#[tokio::test]
async fn a_remote_named_like_an_option_is_never_the_main_branch_or_run_by_a_rebase() {
    let home = git_home().with_identity();
    let repo = repo_with_evil_remote(&home, "rebase");

    let state = git_sync_state(&home.env, &home.runner, &repo.path, None).await.unwrap();
    assert!(
        state.main.iter().all(|main| !main.reference.starts_with('-')),
        "{:?}",
        state.main
    );
    assert!(state.remote.iter().all(|remote| !remote.starts_with('-')));

    let result = git_sync_action(&home.env, &home.runner, &repo.path, None, SyncAction::Rebase).await;

    assert_nothing_ran(&repo);
    if let Ok(done) = result {
        assert_eq!(done.outcome, ActionOutcome::Done);
        assert!(repo.path.join("m.txt").exists(), "rebased onto the local main");
    }
}

#[tokio::test]
async fn a_remote_named_like_an_option_is_not_run_by_a_merge_or_a_branch_created_from_main() {
    let home = git_home().with_identity();
    let merge = repo_with_evil_remote(&home, "merge");
    let create = repo_with_evil_remote(&home, "create");

    let merged = git_sync_action(&home.env, &home.runner, &merge.path, None, SyncAction::Merge).await;
    let request = SwitchRequest {
        branch: "fresh".to_owned(),
        create: Some(BranchStart::Main),
        stash: false,
    };
    let created = git_switch(&home.env, &home.runner, &create.path, None, &request).await;

    assert_nothing_ran(&merge);
    assert_nothing_ran(&create);
    if let Ok(done) = merged {
        assert!(merge.path.join("m.txt").exists());
        assert_eq!(done.outcome, ActionOutcome::Done);
    }
    if let Ok(done) = created {
        assert_eq!(done.state.branch.as_deref(), Some("fresh"));
    }
}

#[tokio::test]
async fn a_default_branch_symref_with_a_hostile_name_is_not_trusted() {
    let home = git_home().with_identity();
    let mut repo = home.repo("symref");
    repo.commit_file("a.txt", "a\n");
    repo.git(&["remote", "add", "origin", "/nowhere"]);
    repo.git(&["update-ref", "refs/remotes/origin/--exec=x", "main"]);
    repo.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/--exec=x",
    ]);

    let state = git_sync_state(&home.env, &home.runner, &repo.path, None).await.unwrap();

    assert!(
        state.main.iter().all(|main| !main.name.starts_with('-')),
        "{:?}",
        state.main
    );
}

#[tokio::test]
async fn a_hostile_branch_name_in_the_rebase_bookkeeping_is_not_reported() {
    let home = git_home().with_identity();
    let mut repo = home.repo("bookkeeping");
    repo.commit_file("a.txt", "a\n");

    let dir = repo.path.join(".git/rebase-merge");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("head-name"), "refs/heads/--exec=evil\n").unwrap();
    fs::write(dir.join("onto"), "not a sha; rm -rf\n").unwrap();

    let state = git_sync_state(&home.env, &home.runner, &repo.path, None).await.unwrap();

    let operation = state.operation.expect("a rebase directory is a rebase");
    assert!(state.branch.iter().all(|branch| !branch.starts_with('-')));
    assert_eq!(operation.onto, None);
}
