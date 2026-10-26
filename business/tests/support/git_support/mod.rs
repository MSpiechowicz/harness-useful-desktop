//! Temporary git repositories for the git tests and the exported `git/` fixtures.
//!
//! Setup commands run the system `git` with an empty global configuration, no system
//! configuration, a fixed identity and fixed dates, so commits (and their shas) are reproducible.
//! The code under test gets an [`Env`] rooted at the temporary home with the real `PATH`.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use harness_business::env::Env;
use harness_business::error::Result;
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner, TokioRunner};
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

/// The real runner, with `dir` appended to the `PATH` of every child (git finds a
/// `git-remote-<name>` helper there).
pub struct WithPath {
    /// The directory added to `PATH`.
    pub dir: PathBuf,
}

impl ProcessRunner for WithPath {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let spec = spec.clone().search_dirs([self.dir.clone()]);

        Box::pin(async move { TokioRunner.run(&spec).await })
    }
}

/// The real runner that calls `then` once, right after the `nth` (counting from 1) command whose
/// arguments include every one of `words`: something else happening in the repository meanwhile.
pub struct AfterCommand {
    words: Vec<String>,
    nth: usize,
    seen: AtomicUsize,
    then: Box<dyn Fn() + Send + Sync>,
}

impl AfterCommand {
    pub fn new(words: &[&str], nth: usize, then: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            words: words.iter().map(|word| (*word).to_owned()).collect(),
            nth,
            seen: AtomicUsize::new(0),
            then: Box::new(then),
        }
    }
}

impl ProcessRunner for AfterCommand {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let args: Vec<String> = spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        let matches = self.words.iter().all(|word| args.contains(word));

        Box::pin(async move {
            let output = TokioRunner.run(spec).await;

            if matches && self.seen.fetch_add(1, Ordering::SeqCst) + 1 == self.nth {
                (self.then)();
            }

            output
        })
    }
}

/// A repository directory driven by setup commands.
#[derive(Clone)]
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

    /// Gives commits made by the code under test (rebase, merge, stash, which the runner starts
    /// without the `GIT_*` identity of the setup commands) an author in the global configuration.
    pub fn with_identity(self) -> Self {
        fs::write(
            self.home.join(".gitconfig"),
            "[user]\n\tname = Demo User\n\temail = demo@example.com\n",
        )
        .expect("identity in the global config");
        self
    }

    /// `git clone <remote> <home>/<name>`.
    pub fn clone_of(&self, remote: &Path, name: &str) -> Repo {
        let path = self.home.join(name);
        run_git(
            &self.home,
            &self.home,
            &["clone", "-q", &remote.to_string_lossy(), &path.to_string_lossy()],
        );

        Repo {
            path: dunce::canonicalize(&path).expect("canonical clone"),
            home: self.home.clone(),
            clock: 100,
        }
    }

    /// Runs a setup git command in `dir`; panics on failure.
    pub fn git_in(&self, dir: &Path, args: &[&str]) -> String {
        run_git(dir, &self.home, args)
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

    /// Writes `content` to `relative` and stages it: a conflict resolved.
    pub fn resolve(&self, relative: &str, content: &str) {
        self.write(relative, content);
        self.git(&["add", relative]);
    }

    /// Installs the hook `name` (an executable `sh` script running `body`).
    #[cfg(unix)]
    pub fn install_hook(&self, name: &str, body: &str) {
        use std::os::unix::fs::PermissionsExt;

        let path = self.path.join(".git/hooks").join(name);
        fs::create_dir_all(path.parent().expect("hooks dir")).expect("hooks dir");
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write hook"); // guard-allow: installer-shell
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("make hook executable");
    }

    /// The commit `HEAD` points at.
    pub fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    /// Writes `content` to `name`, then commits everything.
    pub fn commit_file(&mut self, name: &str, content: &str) {
        self.write(name, content);
        self.commit(&format!("Change {name}"));
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

/// A bare remote with `main` (ten-line `a.txt`, `b.txt`) and a clone of it, with an identity in the
/// global configuration.
pub struct World {
    /// The home holding the repositories.
    pub home: GitHome,
    /// The bare remote.
    pub remote: PathBuf,
    /// A clone of the remote, on `main` tracking `origin/main`.
    pub work: Repo,
}

/// The `a.txt` of [`world`]: ten lines, so edits to far apart lines merge cleanly.
pub const TEN_LINES: &str = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\n";

/// See [`World`].
pub fn world() -> World {
    let home = git_home().with_identity();
    let remote = home.bare("origin.git");

    let mut seed = home.repo("seed");
    seed.write("a.txt", TEN_LINES);
    seed.write("b.txt", "b\n");
    seed.commit("base");
    seed.git(&["remote", "add", "origin", &remote.to_string_lossy()]);
    seed.git(&["push", "-q", "-u", "origin", "main"]);

    let work = home.clone_of(&remote, "work");

    World { home, remote, work }
}

impl World {
    /// A second clone of the remote, to push what others would.
    pub fn other(&self) -> Repo {
        self.home.clone_of(&self.remote, "other")
    }
}

/// Moves `main` on `origin` (a commit changing `main_file` to `main_content`) and puts `work` on a
/// new `feat` branch with a commit of its own (`feat_file` set to `feat_content`), then fetches.
pub async fn diverge(world: &mut World, feat: (&str, &str), main: (&str, &str)) {
    let mut other = world.other();
    other.commit_file(main.0, main.1);
    other.git(&["push", "-q", "origin", "main"]);

    world.work.git(&["switch", "-q", "-c", "feat"]);
    world.work.commit_file(feat.0, feat.1);

    let home = &world.home;
    harness_business::git::git_fetch(&home.env, &home.runner, &world.work.path, None, false)
        .await
        .expect("fetch");
}
