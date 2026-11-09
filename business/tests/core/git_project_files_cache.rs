//! `project_files` listings: one shared listing for requests that arrive together, the cache's
//! lifetime and size, a listing cut by git's output cap, and a hostile repository's fsmonitor hook.

use crate::git_support::git_home;

use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::git_project_files::{base_repo, paths, search};
use harness_business::error::Result;
use harness_business::git::{ProjectFileCache, project_files};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner, TokioRunner};
use pretty_assertions::assert_eq;

/// Runs git for real, but cuts the output of every `ls-files` after `cap` bytes the way
/// `GIT_OUTPUT_CAP` would.
struct Capped {
    cap: usize,
}

impl ProcessRunner for Capped {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let listing = spec.args.iter().any(|arg| arg.to_str() == Some("ls-files"));

        Box::pin(async move {
            let mut output = TokioRunner.run(spec).await?;

            if listing && output.stdout.len() > self.cap {
                output.stdout.truncate(self.cap);
                output.truncated = true;
            }

            Ok(output)
        })
    }
}

#[tokio::test]
async fn a_listing_cut_by_the_output_cap_is_used_up_to_its_last_whole_name() {
    let home = git_home();
    let repo = base_repo(&home);

    // "README.md\0docs/guide.md\0src/li" is what survives of the tracked list.
    let files = project_files(&home.env, &Capped { cap: 30 }, &repo.path, "", 50)
        .await
        .unwrap();

    assert!(paths(&files).contains(&"README.md") && paths(&files).contains(&"docs/guide.md"));
    // The cut name "src/li" would still give its folder.
    assert!(!paths(&files).contains(&"src"), "{:?}", paths(&files));
}

#[tokio::test]
async fn each_part_of_the_listing_has_its_own_output_cap() {
    let home = git_home();
    let repo = base_repo(&home);
    for number in 0..4 {
        repo.write(&format!("untracked-work-{number}.txt"), "x\n");
    }

    // The tracked list (47 bytes) fits the cap, the untracked one (96 bytes) does not: one shared
    // cap for both would cut the tracked files off, whichever of them git prints first.
    let files = project_files(&home.env, &Capped { cap: 60 }, &repo.path, "", 50)
        .await
        .unwrap();
    let listed = paths(&files);

    for tracked in ["README.md", "docs/guide.md", "src/lib.rs", "src/main.rs"] {
        assert!(listed.contains(&tracked), "{tracked} is missing from {listed:?}");
    }
    assert!(listed.contains(&"untracked-work-0.txt"), "{listed:?}");
    assert!(
        !listed.contains(&"untracked-work-3.txt"),
        "the cut list is not whole: {listed:?}"
    );
}

/// Counts the listings of the real runner: one per `ls-files` of the untracked files, which every
/// listing of a project runs once (next to its `ls-files` of the tracked ones).
struct Counting {
    listings: AtomicUsize,
}

impl ProcessRunner for Counting {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        if spec.args.iter().any(|arg| arg.to_str() == Some("--others")) {
            self.listings.fetch_add(1, Ordering::SeqCst);
        }

        Box::pin(async move { TokioRunner.run(spec).await })
    }
}

#[tokio::test]
async fn requests_that_arrive_together_share_one_listing_and_a_later_one_lists_again() {
    let home = git_home();
    let repo = base_repo(&home);
    let runner = Counting {
        listings: AtomicUsize::new(0),
    };
    let cache = ProjectFileCache::with_ttl(Duration::from_millis(1500));
    let ask = |query: &'static str| cache.search(&home.env, &runner, &repo.path, query, 10);

    let (first, second, third, fourth) = tokio::join!(ask("main"), ask("lib"), ask(""), ask("guide"));

    assert_eq!(paths(&first.unwrap()), ["src/main.rs"]);
    assert_eq!(paths(&second.unwrap()), ["src/lib.rs"]);
    assert!(!third.unwrap().is_empty());
    assert_eq!(paths(&fourth.unwrap()), ["docs/guide.md"]);
    assert_eq!(runner.listings.load(Ordering::SeqCst), 1);

    // Within the lifetime the list is reused, even for a file that appeared since.
    repo.write("src/fresh.rs", "x\n");
    assert!(ask("fresh").await.unwrap().is_empty());
    assert_eq!(runner.listings.load(Ordering::SeqCst), 1);

    tokio::time::sleep(Duration::from_millis(1600)).await;

    assert_eq!(paths(&ask("fresh").await.unwrap()), ["src/fresh.rs"]);
    assert_eq!(runner.listings.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_long_query_does_not_list_the_project() {
    let home = git_home();
    let repo = base_repo(&home);
    let runner = Counting {
        listings: AtomicUsize::new(0),
    };
    let cache = ProjectFileCache::default();

    let files = cache
        .search(&home.env, &runner, &repo.path, &"x".repeat(257), 10)
        .await
        .unwrap();

    assert!(files.is_empty());
    assert_eq!(runner.listings.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn the_cache_keeps_a_list_for_each_project_and_forgets_the_oldest_beyond_eight() {
    let home = git_home();
    let runner = Counting {
        listings: AtomicUsize::new(0),
    };
    let cache = ProjectFileCache::default();

    let projects: Vec<_> = (0..9)
        .map(|number| {
            let repo = home.repo(&format!("p{number}"));
            repo.write("a.txt", "x\n");
            repo
        })
        .collect();

    for project in &projects {
        cache.search(&home.env, &runner, &project.path, "", 5).await.unwrap();
    }
    assert_eq!(runner.listings.load(Ordering::SeqCst), 9);

    // The newest eight are still there, the first one was given up for the ninth.
    for project in &projects[1..] {
        cache.search(&home.env, &runner, &project.path, "", 5).await.unwrap();
    }
    assert_eq!(runner.listings.load(Ordering::SeqCst), 9);

    cache
        .search(&home.env, &runner, &projects[0].path, "", 5)
        .await
        .unwrap();
    assert_eq!(runner.listings.load(Ordering::SeqCst), 10);
}

#[cfg(unix)]
#[tokio::test]
async fn a_repository_that_names_an_fsmonitor_hook_does_not_run_it() {
    use std::os::unix::fs::PermissionsExt;

    let home = git_home();
    let repo = base_repo(&home);
    let marker = home.home.join("fsmonitor.marker");
    let script = home.home.join("fsmonitor.sh");
    fs::write(
        &script,
        format!("#!/bin/sh\ntouch '{}'\ncat >/dev/null\nexit 0\n", marker.display()), // guard-allow: installer-shell
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    repo.git(&["config", "core.fsmonitor", &script.to_string_lossy()]);

    // Control run: plain git starts the hook while it lists files.
    repo.git(&["ls-files", "-z", "--cached", "--others"]);
    assert!(marker.exists(), "the trap must fire for plain git");
    fs::remove_file(&marker).unwrap();

    let files = search(&home, &repo.path, "main").await;

    assert_eq!(paths(&files), ["src/main.rs"]);
    assert!(!marker.exists(), "the hook ran");
}
