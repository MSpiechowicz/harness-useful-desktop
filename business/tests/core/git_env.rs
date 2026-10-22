//! Inherited `GIT_*` variables must not redirect or reconfigure the business layer's git.
//!
//! The app inherits the environment it was launched from, which may carry `GIT_DIR`,
//! `GIT_WORK_TREE`, `GIT_CONFIG_PARAMETERS`, `GIT_TRACE` and the like. The tests set them on a
//! child copy of this test binary (the process environment of this one stays untouched, so no
//! test has to serialize on it); the child reads the project repository with the business git
//! functions. A control run of plain git under the same environment proves the variables bite.
#![cfg(unix)]

use crate::common::test_name;
use crate::git_support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use git_support::{GitHome, Repo, git_home};
use harness_business::env::Env;
use harness_business::git::{git_branches, git_diff, git_status};
use harness_business::model::{DiffRequest, DiffScope};
use harness_business::process::TokioRunner;

/// Set in the child: the home of the repositories.
const CHILD_HOME: &str = "HARNESS_GIT_ENV_HOME";

/// Set in the child: the project repository.
const CHILD_PROJECT: &str = "HARNESS_GIT_ENV_PROJECT";

/// The hostile setup: the project on `main`, another repository on `elsewhere`, and the
/// variables that point git at the other one, inject an fsmonitor hook and turn on tracing.
struct Hostile {
    home: GitHome,
    project: Repo,
    elsewhere: Repo,
    monitor_marker: PathBuf,
    trace_marker: PathBuf,
    vars: Vec<(&'static str, String)>,
}

fn hostile() -> Hostile {
    let home = git_home();

    let mut project = home.repo("project");
    project.write("code.txt", "one\n");
    project.commit("base");
    project.write("code.txt", "two\n");

    let mut elsewhere = home.repo("elsewhere");
    elsewhere.git(&["checkout", "-q", "-b", "elsewhere"]);
    elsewhere.write("other.txt", "other\n");
    elsewhere.commit("other");
    elsewhere.write("other.txt", "changed\n");

    let monitor_marker = home.home.join("fsmonitor.marker");
    let monitor = home.home.join("fsmonitor.sh");
    fs::write(
        &monitor,
        format!("#!/bin/sh\ntouch '{}'\nexit 0\n", monitor_marker.display()), // guard-allow: installer-shell
    )
    .unwrap();
    fs::set_permissions(&monitor, fs::Permissions::from_mode(0o755)).unwrap();

    let trace_marker = home.home.join("trace.marker");
    let vars = vec![
        ("GIT_DIR", elsewhere.path.join(".git").display().to_string()),
        ("GIT_WORK_TREE", elsewhere.path.display().to_string()),
        (
            "GIT_CONFIG_PARAMETERS",
            format!("'core.fsmonitor'='{}'", monitor.display()),
        ),
        ("GIT_CONFIG_COUNT", "1".to_owned()),
        ("GIT_CONFIG_KEY_0", "core.fsmonitor".to_owned()),
        ("GIT_CONFIG_VALUE_0", monitor.display().to_string()),
        ("GIT_EXTERNAL_DIFF", monitor.display().to_string()),
        ("GIT_TRACE", trace_marker.display().to_string()),
    ];

    Hostile {
        home,
        project,
        elsewhere,
        monitor_marker,
        trace_marker,
        vars,
    }
}

fn clear(hostile: &Hostile) {
    fs::remove_file(&hostile.monitor_marker).ok();
    fs::remove_file(&hostile.trace_marker).ok();
}

#[test]
fn the_inherited_variables_redirect_plain_git() {
    let hostile = hostile();
    clear(&hostile);

    let output = Command::new("git")
        .args(["status", "--porcelain=v2", "--branch"])
        .current_dir(&hostile.project.path)
        .env("HOME", &hostile.home.home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .envs(hostile.vars.iter().map(|(name, value)| (name, value)))
        .output()
        .expect("git runs");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("# branch.head elsewhere"), "{stdout}");
    assert!(
        hostile.monitor_marker.exists(),
        "plain status runs the injected fsmonitor"
    );
    assert!(hostile.trace_marker.exists(), "plain git writes the trace");
    assert_ne!(hostile.elsewhere.path, hostile.project.path);
}

#[test]
fn inherited_git_variables_do_not_reach_the_business_git() {
    let hostile = hostile();
    clear(&hostile);

    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            test_name(module_path!(), "child_reads_the_project_repository").as_str(),
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_HOME, &hostile.home.home)
        .env(CHILD_PROJECT, &hostile.project.path)
        .envs(hostile.vars.iter().map(|(name, value)| (name, value)))
        .output()
        .expect("the child test runs");

    assert!(
        output.status.success(),
        "child failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child test ran"
    );
    assert!(!hostile.monitor_marker.exists(), "the injected fsmonitor ran");
    assert!(!hostile.trace_marker.exists(), "git wrote a trace");
}

/// Not a real test: the body of the child started by
/// `inherited_git_variables_do_not_reach_the_business_git`. Does nothing on its own.
#[tokio::test]
#[ignore = "child process, started by inherited_git_variables_do_not_reach_the_business_git"]
async fn child_reads_the_project_repository() {
    let (Some(home), Some(project)) = (std::env::var_os(CHILD_HOME), std::env::var_os(CHILD_PROJECT)) else {
        return;
    };
    let project = PathBuf::from(project);
    let env = child_env(Path::new(&home));
    let runner = TokioRunner;

    let status = git_status(&env, &runner, &project).await.unwrap();
    assert!(status.is_repo);
    assert_eq!(status.branch.as_deref(), Some("main"));

    let paths: Vec<&str> = status.entries.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(paths, ["code.txt"]);

    let branches = git_branches(&env, &runner, &project).await.unwrap();
    let names: Vec<&str> = branches.iter().map(|branch| branch.name.as_str()).collect();
    assert_eq!(names, ["main"]);

    let request = DiffRequest {
        scope: DiffScope::All,
        paths: Vec::new(),
    };
    let files = git_diff(&env, &runner, &project, &request).await.unwrap();
    let diffed: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(diffed, ["code.txt"]);
    assert_eq!((files[0].additions, files[0].deletions), (1, 1));
}

fn child_env(home: &Path) -> Env {
    let path = std::env::var("PATH").unwrap_or_default();

    Env::for_tests(home).with_var("PATH", path)
}
