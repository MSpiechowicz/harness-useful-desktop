//! `git_branch_search` over repositories with many branches.

use crate::git_support;

use std::fs;

use git_support::{GitHome, Repo, world};
use harness_business::git::git_branch_search;
use harness_business::model::{BranchPage, IpcErrorCode};
use pretty_assertions::assert_eq;

/// Writes the refs `names` (`refs/...`), all at HEAD, into `packed-refs`, which beats creating
/// thousands of refs one process at a time.
fn pack_refs(repo: &Repo, names: &[String]) {
    let sha = repo.head();
    let mut sorted: Vec<&String> = names.iter().collect();
    sorted.sort();

    let lines: String = sorted.iter().map(|name| format!("{sha} {name}\n")).collect();
    let path = repo.path.join(".git/packed-refs");
    // The clone already packed its remote refs: keep them, under the header written here.
    let existing: String = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| format!("{line}\n"))
        .collect();

    fs::write(
        path,
        format!("# pack-refs with: peeled fully-peeled\n{existing}{lines}"),
    )
    .unwrap();
}

async fn search(home: &GitHome, repo: &Repo, query: &str, limit: u32) -> BranchPage {
    git_branch_search(&home.env, &home.runner, &repo.path, query, limit)
        .await
        .unwrap()
}

#[tokio::test]
async fn searching_fifteen_hundred_branches_pages_and_counts_every_match() {
    let world = world();
    let mut names: Vec<String> = (0..1000)
        .map(|number| format!("refs/heads/topic-{number:04}"))
        .collect();
    names.extend((0..400).map(|number| format!("refs/remotes/origin/topic-{number:04}")));
    names.extend((0..100).map(|number| format!("refs/remotes/origin/remote-{number:03}")));
    pack_refs(&world.work, &names);
    let (home, work) = (&world.home, &world.work);

    let everything = search(home, work, "", 100).await;
    assert_eq!(everything.branches.len(), 100);
    // 1000 local + main, plus the 100 remote-only ones (the 400 twins and origin/main count once).
    assert_eq!(everything.total, 1101);

    let topics = search(home, work, "TOPIC-01", 25).await;
    assert_eq!(topics.branches.len(), 25);
    assert_eq!(topics.total, 100);
    assert!(
        topics
            .branches
            .iter()
            .all(|row| row.name.starts_with("topic-01") && !row.remote_only)
    );

    let remote = search(home, work, "remote-0", 500).await;
    assert_eq!(remote.branches.len(), 100, "the limit is clamped");
    assert!(
        remote
            .branches
            .iter()
            .all(|row| row.remote_only && !row.name.starts_with("origin/"))
    );
    assert!(remote.branches[0].last_commit_date.is_some());

    let none = search(home, work, "no-such-branch", 10).await;
    assert_eq!((none.branches.len(), none.total), (0, 0));
}

#[tokio::test]
async fn the_search_marks_the_current_branch_and_lists_local_before_remote_only() {
    let world = world();
    let mut other = world.other();
    other.git(&["switch", "-q", "-c", "aaa-remote"]);
    other.commit_file("r.txt", "r\n");
    other.git(&["push", "-q", "origin", "aaa-remote"]);
    world.work.git(&["fetch", "-q"]);

    let page = search(&world.home, &world.work, "", 10).await;
    let names: Vec<(&str, bool, bool)> = page
        .branches
        .iter()
        .map(|row| (row.name.as_str(), row.is_current, row.remote_only))
        .collect();

    assert_eq!(names, [("main", true, false), ("aaa-remote", false, true)]);
}

#[tokio::test]
async fn a_query_over_the_limit_is_refused() {
    let world = world();
    let result = git_branch_search(
        &world.home.env,
        &world.home.runner,
        &world.work.path,
        &"x".repeat(201),
        10,
    )
    .await;

    assert_eq!(result.unwrap_err().code(), IpcErrorCode::InvalidInput);
}
