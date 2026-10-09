//! Temporary git repositories for the git tests and the exported `git/` fixtures.
//!
//! Setup commands run the system `git` with an empty global configuration, no system
//! configuration, a fixed identity and fixed dates, so commits (and their shas) are reproducible.
//! The code under test gets an [`Env`] rooted at the temporary home with the real `PATH`.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use harness_business::env::Env;
use harness_business::process::TokioRunner;
use tempfile::TempDir;

/// A temporary home holding repositories.
pub struct GitHome {
    /// Keeps the directory alive.
    pub dir: TempDir,
    /// Canonical home.
    pub home: PathBuf,
    /// Environment for the code under test.
    pub env: Env,
    /// The runner for the code under test.
    pub runner: TokioRunner,
}

/// A fresh home with an empty global git configuration.
pub fn git_home() -> GitHome {
    let dir = tempfile::tempdir().expect("temp home");
    let home = dunce::canonicalize(dir.path()).expect("canonical home");
    fs::write(home.join(".gitconfig"), "").expect("empty global config");

    let path = std::env::var("PATH").unwrap_or_default();
    let env = Env::for_tests(&home).with_var("PATH", path);

    GitHome {
        dir,
        home,
        env,
        runner: TokioRunner,
    }
}

/// A repository directory driven by setup commands.
pub struct Repo {
    /// Work tree (canonical).
    pub path: PathBuf,
    home: PathBuf,
    clock: u32,
}

impl GitHome {
    /// `git init -b main <home>/<name>`.
    pub fn repo(&self, name: &str) -> Repo {
        let path = self.home.join(name);
        fs::create_dir_all(&path).expect("repo dir");

        let repo = Repo {
            path: dunce::canonicalize(&path).expect("canonical repo"),
            home: self.home.clone(),
            clock: 0,
        };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    /// A bare repository at `<home>/<name>` (a local remote).
    pub fn bare(&self, name: &str) -> PathBuf {
        let path = self.home.join(name);
        run_git(
            &self.home,
            &self.home,
            &["init", "-q", "--bare", "-b", "main", &path.to_string_lossy()],
        );
        path
    }
}

impl Repo {
    /// Runs a setup git command and returns its stdout; panics on failure.
    pub fn git(&self, args: &[&str]) -> String {
        run_git(&self.path, &self.home, args)
    }

    /// Writes `content` to `relative`, creating parent directories.
    pub fn write(&self, relative: &str, content: impl AsRef<[u8]>) {
        let path = self.path.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("parent dir");
        fs::write(path, content).expect("write file");
    }

    /// Stages everything and commits with the next fixed date.
    pub fn commit(&mut self, message: &str) {
        self.git(&["add", "-A"]);
        self.commit_staged(message);
    }

    /// Commits what is staged as the default author with the next fixed date.
    pub fn commit_staged(&mut self, message: &str) {
        self.commit_as(message, "Demo User");
    }

    /// Commits what is staged as `author`, one hour after the previous commit (from
    /// 2026-10-01T01:00:00Z).
    pub fn commit_as(&mut self, message: &str, author: &str) {
        self.clock += 1;
        let date = format!("2026-10-{:02}T{:02}:00:00Z", 1 + self.clock / 24, self.clock % 24);

        let status = command(&self.path, &self.home, &["commit", "-q", "-m", message])
            .env("GIT_AUTHOR_NAME", author)
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .status()
            .expect("git commit runs");
        assert!(status.success(), "git commit {message}");
    }
}

fn command(dir: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Demo User")
        .env("GIT_AUTHOR_EMAIL", "demo@example.com")
        .env("GIT_COMMITTER_NAME", "Demo User")
        .env("GIT_COMMITTER_EMAIL", "demo@example.com")
        .env("LC_ALL", "C");
    command
}

fn run_git(dir: &Path, home: &Path, args: &[&str]) -> String {
    let output = command(dir, home, args).output().expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).into_owned()
}
