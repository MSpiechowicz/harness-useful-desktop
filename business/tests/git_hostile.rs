//! A hostile repository: its `.git/config` names an fsmonitor hook, a textconv filter and an
//! external diff driver (selected through `.gitattributes`), each a script that leaves a marker
//! file. Control runs of plain git prove the traps fire; the business layer's status and diffs
//! must never trigger them.
#![cfg(unix)]

mod git_support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use git_support::{GitHome, Repo, git_home};
use harness_business::git::{git_branches, git_diff, git_status};
use harness_business::model::{DiffRequest, DiffScope};

fn trap(home: &GitHome, name: &str) -> (PathBuf, PathBuf) {
    let marker = home.home.join(format!("{name}.marker"));
    let script = home.home.join(format!("{name}.sh"));

    fs::write(
        &script,
        format!(
            "#!/bin/sh\ntouch '{}'\ncat \"$1\" 2>/dev/null\nexit 0\n",
            marker.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    (script, marker)
}

fn hostile_repo(home: &GitHome) -> (Repo, [PathBuf; 3]) {
    let mut repo = home.repo("hostile");
    repo.write(".gitattributes", "* diff=evil\n");
    repo.write("code.txt", "one\n");
    repo.commit("base");

    let (monitor, monitor_marker) = trap(home, "fsmonitor");
    let (textconv, textconv_marker) = trap(home, "textconv");
    let (external, external_marker) = trap(home, "external");

    repo.git(&["config", "core.fsmonitor", &monitor.to_string_lossy()]);
    repo.git(&["config", "diff.evil.textconv", &textconv.to_string_lossy()]);
    repo.git(&["config", "diff.evil.command", &external.to_string_lossy()]);
    repo.write("code.txt", "two\n");

    (repo, [monitor_marker, textconv_marker, external_marker])
}

fn clear(markers: &[PathBuf]) {
    for marker in markers {
        fs::remove_file(marker).ok();
    }
}

#[test]
fn the_traps_fire_for_plain_git() {
    let home = git_home();
    let (repo, [monitor, textconv, external]) = hostile_repo(&home);

    repo.git(&["status"]);
    assert!(monitor.exists(), "plain status runs the fsmonitor hook");

    repo.git(&["diff"]);
    assert!(external.exists(), "plain diff runs the external driver");

    repo.git(&["diff", "--no-ext-diff"]);
    assert!(textconv.exists(), "plain diff runs textconv");
}

#[tokio::test]
async fn status_branches_and_diffs_never_run_repository_programs() {
    let home = git_home();
    let (repo, markers) = hostile_repo(&home);
    clear(&markers);

    let status = git_status(&home.env, &home.runner, &repo.path).await.unwrap();
    assert!(status.is_repo);
    git_branches(&home.env, &home.runner, &repo.path).await.unwrap();

    for scope in [
        DiffScope::All,
        DiffScope::Unstaged,
        DiffScope::Staged,
        DiffScope::Untracked,
    ] {
        let request = DiffRequest {
            scope,
            paths: Vec::new(),
        };
        let files = git_diff(&home.env, &home.runner, &repo.path, &request).await.unwrap();

        if scope == DiffScope::Unstaged {
            let file = files.iter().find(|file| file.path == "code.txt").unwrap();
            assert_eq!(
                (file.additions, file.deletions),
                (1, 1),
                "the real diff, not a filter's"
            );
        }
    }

    for marker in &markers {
        assert!(!marker.exists(), "{} was triggered", marker.display());
    }
}
