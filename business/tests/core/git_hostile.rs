//! A hostile repository: its `.git/config` names an fsmonitor hook, a textconv filter and an
//! external diff driver (selected through `.gitattributes`), each a script that leaves a marker
//! file. Control runs of plain git prove the traps fire; the business layer's status and diffs
//! must never trigger them.
#![cfg(unix)]

use crate::git_support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use git_support::{GitHome, Repo, git_home};
use harness_business::git::{
    git_branches, git_diff, git_fetch, git_file_lines, git_status, git_switch, git_sync_action, validate_branch_name,
};
use harness_business::model::{DiffRequest, DiffScope, IpcErrorCode, SwitchRequest, SyncAction};

fn trap(home: &GitHome, name: &str) -> (PathBuf, PathBuf) {
    let marker = home.home.join(format!("{name}.marker"));
    let script = home.home.join(format!("{name}.sh"));

    fs::write(
        &script,
        format!(
            "#!/bin/sh\ntouch '{}'\ncat \"$1\" 2>/dev/null\nexit 0\n", // guard-allow: installer-shell
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

#[test]
fn branch_names_that_git_or_a_shell_could_misread_are_rejected() {
    let long = "x".repeat(256);
    let rejected = [
        "",
        "-x",
        "--force",
        "HEAD",
        "@",
        "a..b",
        "a@{1}",
        "a//b",
        "a b",
        "a\tb",
        "a\nb",
        "a~1",
        "a^",
        "a:b",
        "a?",
        "a*",
        "a[b",
        "a\\b",
        "/a",
        "a/",
        "a.",
        "a/.hidden",
        ".hidden",
        "topic.lock",
        "a/topic.lock/b",
        long.as_str(),
    ];

    for name in rejected {
        let error = validate_branch_name(name).unwrap_err();
        assert_eq!(error.code(), IpcErrorCode::InvalidInput, "{name:?}");
    }

    for name in ["main", "feat/empty-input", "release-1.2", "ünï/çode", "a.b", "fix_123"] {
        validate_branch_name(name).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

#[tokio::test]
async fn a_switch_to_an_option_shaped_name_never_reaches_git() {
    let home = git_home();
    let mut repo = home.repo("repo");
    repo.write("a.txt", "a\n");
    repo.commit("base");

    for name in ["--detach", "-c", "--orphan=x"] {
        let request = SwitchRequest {
            branch: name.to_owned(),
            create: None,
            stash: true,
        };
        let result = git_switch(&home.env, &home.runner, &repo.path, None, &request).await;

        assert_eq!(result.unwrap_err().code(), IpcErrorCode::InvalidInput, "{name}");
    }

    assert_eq!(repo.git(&["branch", "--show-current"]).trim(), "main");
}

#[tokio::test]
async fn a_remote_named_like_an_option_is_never_passed_to_git() {
    let home = git_home().with_identity();
    let mut repo = home.repo("repo");
    repo.write("a.txt", "a\n");
    repo.commit("base");
    repo.git(&[
        "config",
        "remote.-x.url",
        &home.home.join("nowhere.git").to_string_lossy(),
    ]);

    let fetch = git_fetch(&home.env, &home.runner, &repo.path, None, false).await;
    assert_eq!(fetch.unwrap_err().code(), IpcErrorCode::InvalidInput);

    let publish = git_sync_action(&home.env, &home.runner, &repo.path, None, SyncAction::Publish).await;
    assert_eq!(publish.unwrap_err().code(), IpcErrorCode::InvalidInput);
}

#[tokio::test]
async fn a_failing_network_command_reports_no_credentials_from_the_remote_url() {
    let home = git_home().with_identity();
    let remote = home.bare("origin.git");
    let mut repo = home.repo("repo");
    repo.write("a.txt", "a\n");
    repo.commit("base");
    repo.git(&["remote", "add", "origin", &remote.to_string_lossy()]);

    // Built at run time: a literal token shape in the source would trip secret scanners.
    let token = ["gh", "p_", "0123456789abcdefghijklmnopqrstuvwxyzAB"].concat();
    fs::write(
        repo.path.join(".git/hooks/pre-push"),
        format!(
            "#!/bin/sh\necho 'error: rejected by https://alice:{token}@git.example.invalid/team/project.git' >&2\nexit 1\n" // guard-allow: installer-shell
        ),
    )
    .unwrap();
    fs::set_permissions(repo.path.join(".git/hooks/pre-push"), fs::Permissions::from_mode(0o755)).unwrap();

    let publish = git_sync_action(&home.env, &home.runner, &repo.path, None, SyncAction::Publish)
        .await
        .unwrap_err();

    assert_eq!(publish.code(), IpcErrorCode::Process);

    let text = publish.to_string();
    assert!(
        text.contains("git.example.invalid"),
        "the message still names the host: {text}"
    );
    assert!(!text.contains(&token), "{text}");
    assert!(!text.contains("alice"), "{text}");
}

/// A partial clone whose promisor remote is an `ext::` command and whose blob `code.txt` (as staged
/// and as committed) is missing, with `ext::` allowed: any command that needs the blob fetches it
/// lazily. Returns the repository and the marker the command leaves.
fn lazy_fetch_repo(home: &GitHome) -> (Repo, PathBuf) {
    let (command, marker) = trap(home, "lazy");
    let mut repo = home.repo("partial");
    repo.write("code.txt", "one\n");
    repo.commit("base");
    let blob = repo.git(&["rev-parse", ":code.txt"]);
    let blob = blob.trim();

    repo.git(&["config", "core.repositoryformatversion", "1"]);
    repo.git(&["config", "extensions.partialClone", "origin"]);
    repo.git(&["config", "remote.origin.promisor", "true"]);
    repo.git(&["config", "remote.origin.url", &format!("ext::{}", command.display())]);
    repo.git(&["config", "protocol.ext.allow", "always"]);
    repo.write("code.txt", "two\n");

    let object = repo.path.join(".git/objects").join(&blob[..2]).join(&blob[2..]);
    fs::remove_file(object).unwrap();

    (repo, marker)
}

#[tokio::test]
async fn a_partial_clone_cannot_make_a_read_start_its_remote_helper() {
    let home = git_home();
    let (repo, marker) = lazy_fetch_repo(&home);

    // Control: plain git fetches the missing blob through the helper (and fails).
    std::process::Command::new("git")
        .args(["diff"])
        .current_dir(&repo.path)
        .env("HOME", &home.home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(marker.exists(), "plain git lazily fetches through the helper");
    fs::remove_file(&marker).unwrap();

    let request = DiffRequest {
        scope: DiffScope::All,
        paths: Vec::new(),
    };
    let _ = git_diff(&home.env, &home.runner, &repo.path, &request).await;
    let _ = git_file_lines(&home.env, &home.runner, &repo.path, "code.txt", true).await;
    let _ = git_file_lines(&home.env, &home.runner, &repo.path, "code.txt", false).await;
    git_status(&home.env, &home.runner, &repo.path).await.unwrap();
    git_branches(&home.env, &home.runner, &repo.path).await.unwrap();

    assert!(!marker.exists(), "a read started the remote helper");
}

/// How a crafted partial clone reaches its promisor remote through a program of the repository's
/// own configuration.
#[derive(Clone, Copy, Debug)]
enum Transport {
    Ssh,
    UploadPack,
    GitProxy,
}

/// A partial clone with a rename staged and two blobs missing (`old.txt`, as committed, which rename
/// detection reads, and `keep.txt`, as staged). The promisor remote is reached through a program
/// the repository's configuration names. Returns the repository and the marker that program leaves.
fn crafted_partial_clone(home: &GitHome, transport: Transport) -> (Repo, PathBuf) {
    let (command, marker) = trap(home, "transport");
    let mut repo = home.repo("crafted");
    repo.write("old.txt", "alpha\nbeta\ngamma\ndelta\nepsilon\n");
    repo.write("keep.txt", "keep\n");
    repo.commit("base");

    let old = repo.git(&["rev-parse", ":old.txt"]);
    let keep = repo.git(&["rev-parse", ":keep.txt"]);
    let command = command.to_string_lossy().into_owned();

    repo.git(&["config", "core.repositoryformatversion", "1"]);
    repo.git(&["config", "extensions.partialClone", "origin"]);
    repo.git(&["config", "remote.origin.promisor", "true"]);

    match transport {
        Transport::Ssh => {
            repo.git(&["config", "remote.origin.url", "ssh://example.invalid/repo.git"]);
            repo.git(&["config", "core.sshCommand", &command]);
        }
        Transport::UploadPack => {
            repo.git(&["config", "remote.origin.url", "file:///nonexistent/repo.git"]);
            repo.git(&["config", "remote.origin.uploadpack", &command]);
        }
        Transport::GitProxy => {
            repo.git(&["config", "remote.origin.url", "git://example.invalid/repo.git"]);
            repo.git(&["config", "core.gitProxy", &command]);
        }
    }

    repo.git(&["mv", "old.txt", "new.txt"]);
    repo.write("new.txt", "alpha\nbeta\ngamma\ndelta\nepsilon\nzeta\n");
    repo.git(&["add", "new.txt"]);

    for blob in [old.trim(), keep.trim()] {
        fs::remove_file(repo.path.join(".git/objects").join(&blob[..2]).join(&blob[2..])).unwrap();
    }

    (repo, marker)
}

#[tokio::test]
async fn a_crafted_partial_clone_cannot_make_a_read_run_a_transport_program() {
    let mut triggered = Vec::new();

    for transport in [Transport::Ssh, Transport::UploadPack, Transport::GitProxy] {
        let home = git_home();
        let (repo, marker) = crafted_partial_clone(&home, transport);

        // Control: plain git runs the program while it looks for the missing blobs.
        for args in [["diff", "--cached", "-M"], ["status", "--porcelain=v2", "--branch"]] {
            std::process::Command::new("git")
                .args(args)
                .current_dir(&repo.path)
                .env("HOME", &home.home)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap();

            if marker.exists() {
                break;
            }
        }
        assert!(marker.exists(), "{transport:?}: plain git runs the program");
        fs::remove_file(&marker).unwrap();

        let request = DiffRequest {
            scope: DiffScope::All,
            paths: Vec::new(),
        };
        let _ = git_status(&home.env, &home.runner, &repo.path).await;
        let _ = git_diff(&home.env, &home.runner, &repo.path, &request).await;
        let _ = git_file_lines(&home.env, &home.runner, &repo.path, "keep.txt", true).await;

        if marker.exists() {
            triggered.push(transport);
        }
    }

    assert!(triggered.is_empty(), "a read ran the program of {triggered:?}");
}

#[tokio::test]
async fn a_read_of_a_missing_object_says_the_clone_is_not_fully_downloaded() {
    let home = git_home();
    let (repo, _marker) = crafted_partial_clone(&home, Transport::Ssh);
    let request = DiffRequest {
        scope: DiffScope::All,
        paths: Vec::new(),
    };

    let errors = [
        git_file_lines(&home.env, &home.runner, &repo.path, "keep.txt", true)
            .await
            .map(drop)
            .unwrap_err(),
        git_status(&home.env, &home.runner, &repo.path)
            .await
            .map(drop)
            .unwrap_err(),
        git_diff(&home.env, &home.runner, &repo.path, &request)
            .await
            .map(drop)
            .unwrap_err(),
    ];

    for error in errors {
        assert_eq!(error.code(), IpcErrorCode::Process);
        assert!(
            error
                .to_string()
                .contains("Some files of this partial clone are not downloaded yet. Pull or fetch to get them."),
            "{error}"
        );
    }
}

#[tokio::test]
async fn a_real_blob_none_clone_still_reports_its_status() {
    let home = git_home();
    let mut origin = home.repo("origin");
    origin.write("a.txt", "one\n");
    origin.write("b.txt", "two\n");
    origin.commit("base");
    origin.git(&["config", "uploadpack.allowFilter", "true"]);
    origin.git(&["config", "uploadpack.allowAnySHA1InWant", "true"]);

    let path = home.home.join("lean");
    home.git_in(
        &home.home,
        &[
            "clone",
            "-q",
            "--filter=blob:none",
            "--no-checkout",
            &format!("file://{}", origin.path.display()),
            &path.to_string_lossy(),
        ],
    );
    home.git_in(&path, &["sparse-checkout", "set", "--no-cone", "a.txt"]);
    home.git_in(&path, &["checkout", "-q", "main"]);
    fs::write(path.join("a.txt"), "changed\n").unwrap();

    let status = git_status(&home.env, &home.runner, &path).await.unwrap();

    assert!(status.is_repo);
    assert!(status.entries.iter().any(|entry| entry.path == "a.txt"), "{status:?}");
}
