//! Git setup for the command tests: a fixed identity, an opened repository and a folder the app does not know.

use std::path::Path;
use std::process::Command;

use harness_business::env::Env;

use super::{TempDir, state_with_project};
use crate::state::AppState;

/// A setup git command with `home` as the empty home, no system configuration and a fixed identity.
pub fn setup_git(home: &Path, dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Demo User")
        .env("GIT_AUTHOR_EMAIL", "demo@example.com")
        .env("GIT_COMMITTER_NAME", "Demo User")
        .env("GIT_COMMITTER_EMAIL", "demo@example.com")
        .status()
        .expect("git runs");

    assert!(status.success(), "git {args:?}");
}

/// The state with `project` (under `home`, which keeps `PATH` so git runs) open as the active
/// project, and `project` as the `project` argument of a command.
pub fn open_git_project(dir: &TempDir, home: &Path, project: &Path) -> (AppState, String) {
    let path = std::env::var("PATH").unwrap_or_default();
    let state = state_with_project(dir, Env::for_tests(home).with_var("PATH", path), project);

    (state, project.to_string_lossy().into_owned())
}

/// A folder next to `project` that is neither open nor recent, as the `project` argument of a command.
pub fn stranger_beside(project: &str) -> String {
    let stranger = Path::new(project)
        .parent()
        .expect("project has a parent")
        .join("stranger");
    std::fs::create_dir_all(&stranger).expect("create stranger");

    stranger.to_string_lossy().into_owned()
}
