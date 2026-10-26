//! `git_sync_state` and `git_fetch` over temporary repositories with a local bare remote: counts,
//! the main branch detection, stopped operations, subdirectory projects and the edge cases.

use crate::git_support;

use std::fs;

use git_support::{GitHome, Repo, World, git_home, world};
use harness_business::git::{git_fetch, git_sync_action, git_sync_state};
use harness_business::model::{BranchSync, ConflictFile, GitOperationKind, IpcErrorCode, MainSource, SyncAction};
use pretty_assertions::assert_eq;

async fn state(home: &GitHome, repo: &Repo) -> BranchSync {
    git_sync_state(&home.env, &home.runner, &repo.path, None).await.unwrap()
}

async fn state_picking(home: &GitHome, repo: &Repo, picked: &str) -> BranchSync {
    git_sync_state(&home.env, &home.runner, &repo.path, Some(picked))
        .await
        .unwrap()
}

/// The main branch of `state` as (name, reference, source).
fn main_of(state: &BranchSync) -> Option<(String, String, MainSource)> {
    state
        .main
        .as_ref()
        .map(|main| (main.name.clone(), main.reference.clone(), main.source))
}

#[tokio::test]
async fn a_folder_outside_a_repository_is_not_a_repo_and_refuses_writes() {
    let home = git_home();
    let plain = home.home.join("plain");
    fs::create_dir_all(&plain).unwrap();

    let state = git_sync_state(&home.env, &home.runner, &plain, None).await.unwrap();
    assert_eq!(state, BranchSync::default());
    assert!(!state.is_repo);

    let fetch = git_fetch(&home.env, &home.runner, &plain, None, false).await;
    assert_eq!(fetch.unwrap_err().code(), IpcErrorCode::InvalidInput);

    let action = git_sync_action(&home.env, &home.runner, &plain, None, SyncAction::Push).await;
    assert_eq!(action.unwrap_err().code(), IpcErrorCode::InvalidInput);
}

#[tokio::test]
async fn a_clone_reports_its_upstream_remote_and_the_remote_default_branch() {
    let world = world();
    let state = state(&world.home, &world.work).await;

    assert!(state.is_repo);
    assert_eq!(state.branch.as_deref(), Some("main"));
    assert_eq!(state.upstream.as_deref(), Some("origin/main"));
    assert_eq!(state.remote.as_deref(), Some("origin"));
    assert_eq!((state.ahead, state.behind), (0, 0));
    assert_eq!((state.tracked_changes, state.untracked), (0, 0));
    assert_eq!(state.operation, None);
    assert!(!state.stash_kept);

    // On the main branch itself the main row has nothing to compare with.
    assert_eq!(
        main_of(&state),
        Some(("main".to_owned(), "origin/main".to_owned(), MainSource::Remote))
    );
    let main = state.main.unwrap();
    assert!(main.is_current);
    assert_eq!(main.new_commits, 0);
}

#[tokio::test]
async fn fetching_updates_the_counts_and_the_fetch_time() {
    let World { home, remote, work } = world();
    work.git(&["switch", "-q", "-c", "feat"]);
    let mut other = home.clone_of(&remote, "other");
    other.commit_file("m.txt", "m\n");
    other.git(&["push", "-q", "origin", "main"]);

    let before = state(&home, &work).await;
    assert_eq!(before.main.as_ref().map(|main| main.new_commits), Some(0));
    assert_eq!(before.last_fetched, None);

    let fetched = git_fetch(&home.env, &home.runner, &work.path, None, false)
        .await
        .unwrap();
    assert_eq!(fetched.main.as_ref().map(|main| main.new_commits), Some(1));
    assert!(fetched.last_fetched.is_some_and(|time| time.ends_with('Z')));

    // Tracking a branch the remote moved ahead of shows as behind.
    work.git(&["switch", "-q", "main"]);
    let behind = state(&home, &work).await;
    assert_eq!((behind.ahead, behind.behind), (0, 1));
}

#[tokio::test]
async fn a_background_fetch_works_and_changes_nothing_but_the_remote_refs() {
    let World { home, remote, work } = world();
    let mut other = home.clone_of(&remote, "other");
    other.commit_file("m.txt", "m\n");
    other.git(&["push", "-q", "origin", "main"]);

    let fetched = git_fetch(&home.env, &home.runner, &work.path, None, true)
        .await
        .unwrap();

    assert_eq!(fetched.behind, 1);
    assert_eq!(fetched.branch.as_deref(), Some("main"));
    assert!(!work.path.join("m.txt").exists());
}

#[tokio::test]
async fn fetching_without_a_remote_is_refused_and_no_remote_is_reported() {
    let home = git_home().with_identity();
    let mut repo = home.repo("solo");
    repo.commit_file("a.txt", "a\n");

    let state = state(&home, &repo).await;
    assert_eq!((state.remote, state.upstream, state.last_fetched), (None, None, None));

    let fetch = git_fetch(&home.env, &home.runner, &repo.path, None, false).await;
    assert_eq!(fetch.unwrap_err().code(), IpcErrorCode::InvalidInput);
}

#[tokio::test]
async fn the_main_branch_comes_from_the_upstream_remote_before_origin() {
    let World { home, remote: _, work } = world();
    let upstream = home.bare("up.git");
    work.git(&["remote", "add", "upstream", &upstream.to_string_lossy()]);
    work.git(&["push", "-q", "upstream", "main:develop"]);
    work.git(&["fetch", "-q", "upstream"]);
    work.git(&["remote", "set-head", "upstream", "develop"]);
    work.git(&["switch", "-q", "-c", "feat"]);

    let state = state(&home, &work).await;

    assert_eq!(
        main_of(&state),
        Some(("develop".to_owned(), "upstream/develop".to_owned(), MainSource::Remote))
    );
    // Neither the upstream nor the tracked branch is the upstream's remote: this one is origin.
    assert_eq!(state.remote.as_deref(), Some("origin"));
}

#[tokio::test]
async fn without_a_remote_head_the_default_branch_setting_then_the_usual_names_decide() {
    let home = git_home().with_identity();
    let mut repo = home.repo("local");
    repo.commit_file("a.txt", "a\n");
    repo.git(&["branch", "-m", "work"]);
    assert_eq!(main_of(&state(&home, &repo).await), None);

    repo.git(&["branch", "trunk"]);
    repo.git(&["branch", "master"]);
    assert_eq!(
        main_of(&state(&home, &repo).await),
        Some(("master".to_owned(), "master".to_owned(), MainSource::Fallback))
    );

    repo.git(&["branch", "develop"]);
    repo.git(&["config", "init.defaultBranch", "develop"]);
    assert_eq!(
        main_of(&state(&home, &repo).await),
        Some(("develop".to_owned(), "develop".to_owned(), MainSource::Fallback))
    );

    // A default branch that does not exist is skipped.
    repo.git(&["config", "init.defaultBranch", "missing"]);
    assert_eq!(
        main_of(&state(&home, &repo).await).map(|main| main.0),
        Some("master".to_owned())
    );
}

#[tokio::test]
async fn a_picked_main_branch_wins_unless_it_no_longer_exists() {
    let World { home, remote: _, work } = world();
    work.git(&["branch", "develop"]);
    work.git(&["switch", "-q", "-c", "feat"]);

    let picked = state_picking(&home, &work, "develop").await;
    assert_eq!(
        main_of(&picked),
        Some(("develop".to_owned(), "develop".to_owned(), MainSource::Picked))
    );

    // A pick that was deleted since is ignored, the detection answers again.
    let stale = state_picking(&home, &work, "deleted-long-ago").await;
    assert_eq!(
        main_of(&stale),
        Some(("main".to_owned(), "origin/main".to_owned(), MainSource::Remote))
    );
}

#[tokio::test]
async fn detached_and_unborn_repositories_report_without_errors() {
    let World { home, remote: _, work } = world();
    work.git(&["checkout", "-q", "--detach"]);

    let detached = state(&home, &work).await;
    assert_eq!(detached.branch, None);
    assert!(detached.head.is_some());
    assert_eq!(detached.operation, None);

    let empty = home.repo("empty");
    let unborn = state(&home, &empty).await;
    assert!(unborn.is_repo);
    assert_eq!(unborn.head, None);
    assert_eq!(unborn.main, None);
    assert_eq!(unborn.branch.as_deref(), Some("main"));
}

#[tokio::test]
async fn changes_are_counted_tracked_and_untracked_apart() {
    let World { home, remote: _, work } = world();
    work.write("a.txt", "changed\n");
    work.write("b.txt", "changed too\n");
    work.write("new.txt", "new\n");
    work.write("dir/inner.txt", "inner\n");

    let state = state(&home, &work).await;

    assert_eq!((state.tracked_changes, state.untracked), (2, 2));
}

#[tokio::test]
async fn a_stopped_rebase_names_its_branch_and_lists_the_conflicts() {
    let World { home, remote: _, work } = world();
    let mut work = work;
    work.git(&["switch", "-q", "-c", "feat"]);
    work.commit_file("a.txt", "feature side\n");
    work.git(&["switch", "-q", "main"]);
    work.commit_file("a.txt", "main side\n");
    work.git(&["push", "-q"]);
    work.git(&["switch", "-q", "feat"]);

    let before = state_picking(&home, &work, "main").await;
    assert_eq!(before.main.as_ref().map(|main| main.new_commits), Some(1));

    let result = git_sync_action(&home.env, &home.runner, &work.path, Some("main"), SyncAction::Rebase)
        .await
        .unwrap();
    let state = result.state;

    assert_eq!(state.branch.as_deref(), Some("feat"), "never detached");
    let operation = state.operation.expect("rebase in progress");
    assert_eq!(operation.kind, GitOperationKind::Rebase);
    assert_eq!(operation.onto.as_deref(), Some("main"));
    assert_eq!(
        operation.conflicts,
        [ConflictFile {
            path: "a.txt".to_owned(),
            in_project: true,
        }]
    );
}

#[tokio::test]
async fn conflicts_outside_a_subdirectory_project_are_marked() {
    let World { home, remote: _, work } = world();
    let mut work = work;
    work.git(&["switch", "-q", "-c", "feat"]);
    work.commit_file("pkg/inner.txt", "feature\n");
    work.commit_file("outer.txt", "feature\n");
    work.git(&["switch", "-q", "main"]);
    work.commit_file("pkg/inner.txt", "main\n");
    work.commit_file("outer.txt", "main\n");
    work.git(&["push", "-q"]);
    work.git(&["switch", "-q", "feat"]);

    let project = work.path.join("pkg");
    git_sync_action(&home.env, &home.runner, &project, Some("main"), SyncAction::Merge)
        .await
        .unwrap();
    let state = git_sync_state(&home.env, &home.runner, &project, Some("main"))
        .await
        .unwrap();

    let operation = state.operation.expect("merge in progress");
    assert_eq!(operation.kind, GitOperationKind::Merge);
    assert_eq!(
        operation.conflicts,
        [
            ConflictFile {
                path: "inner.txt".to_owned(),
                in_project: true,
            },
            ConflictFile {
                path: "outer.txt".to_owned(),
                in_project: false,
            },
        ],
        "the project's files come first"
    );
}
