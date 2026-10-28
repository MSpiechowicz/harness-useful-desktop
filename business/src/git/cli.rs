//! Hardened `git` invocations.
//!
//! `git` is resolved with [`resolve_binary`] (never through a shell) and every run goes through a
//! [`ProcessRunner`] with a 15 s timeout and a 16 MiB output cap. Each command line starts with
//! options that switch off what a repository's own configuration could otherwise execute or
//! redirect: the fsmonitor hook, the untracked cache, external diff programs, pagers, colors and
//! path quoting. Pathspecs are literal. The environment disables prompts and optional locks
//! (`status` never rewrites the index) and fixes the locale so messages can be matched. `HOME`
//! and `XDG_CONFIG_HOME` come from the injected [`Env`], so the global configuration read is the
//! one of that home. Commands that reach a remote or rewrite history take longer
//! ([`NETWORK_TIMEOUT`]) through [`GitCli::run_with`].
//!
//! Every `GIT_*` variable inherited from the app's own environment is dropped (only the ones set
//! here reach git): `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`,
//! `GIT_COMMON_DIR` and friends would point git at another repository, and `GIT_CONFIG_*`,
//! `GIT_EXTERNAL_DIFF`, `GIT_PAGER`, `GIT_EXEC_PATH`, `GIT_SSH*`, `GIT_ASKPASS` or `GIT_TRACE*`
//! would add configuration, run programs or write files. This includes `GIT_CONFIG_GLOBAL`,
//! `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_NOSYSTEM` and `GIT_CEILING_DIRECTORIES`: the user's global
//! configuration is still read from the default locations under `HOME`/`XDG_CONFIG_HOME`, and the
//! system configuration from its default location.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::env::Env;
use crate::error::{BusinessError, Result};
use crate::process::{ExecOutput, ExecSpec, ProcessRunner, resolve_binary};
use crate::security::redact::redact_text;

/// Time limit of one git command.
pub const GIT_TIMEOUT: Duration = Duration::from_secs(15);

/// Time limit of a command that reaches a remote or rewrites the work tree (fetch, push, pull,
/// rebase, merge, switch).
pub const NETWORK_TIMEOUT: Duration = Duration::from_secs(120);

/// Options placed before a git subcommand that writes: the editor never opens (a commit message or
/// rebase todo would wait for input that never comes).
pub const WRITE_CONFIG: [&str; 4] = ["-c", "core.editor=true", "-c", "sequence.editor=true"];

/// The transports a command that is not a clicked network action may use (`GIT_ALLOW_PROTOCOL`).
/// A repository-local partial clone (`remote.origin.promisor` with an `ext::` or `evil::` URL) makes
/// even `git diff` or `git show` fetch a missing object lazily; without this a read would start
/// the remote helper the repository names.
pub const SAFE_PROTOCOLS: &str = "https:http:ssh:git:file";

/// What a read reports when git needs an object a partial clone has not downloaded yet.
const NOT_DOWNLOADED: &str = "Some files of this partial clone are not downloaded yet. Pull or fetch to get them.";

/// Phrases of git's errors for a missing object, matched in a failed read's stderr.
const MISSING_OBJECT_MARKERS: [&str; 7] = [
    "unable to read",
    "bad object",
    "missing blob",
    "missing object",
    "promisor remote",
    "lazy fetching disabled",
    "did not return the expected objects",
];

/// Output cap of one git command (per stream).
pub const GIT_OUTPUT_CAP: usize = 16 * 1024 * 1024;

/// Options placed before every git subcommand.
const HARDENING: [&str; 15] = [
    "--no-pager",
    "--literal-pathspecs",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.untrackedCache=false",
    "-c",
    "diff.external=",
    "-c",
    "core.pager=cat",
    "-c",
    "color.ui=false",
    "-c",
    "core.quotepath=off",
    "--no-optional-locks",
];

/// Environment of every git command.
const GIT_ENV: [(&str, &str); 3] = [
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_OPTIONAL_LOCKS", "0"),
    ("LC_ALL", "C"),
];

/// Per-call settings of [`GitCli::run_with`].
#[derive(Debug, Clone)]
pub struct RunOpts {
    /// Time limit of the command.
    pub timeout: Duration,
    /// Extra environment variables, applied after the fixed ones.
    pub env: Vec<(String, String)>,
}

impl Default for RunOpts {
    fn default() -> Self {
        Self {
            timeout: GIT_TIMEOUT,
            env: Vec::new(),
        }
    }
}

impl RunOpts {
    /// Options with the given time limit.
    pub fn timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            ..Self::default()
        }
    }
}

/// A git command runner bound to one project directory.
pub struct GitCli<'a> {
    program: PathBuf,
    project: PathBuf,
    home: PathBuf,
    xdg_config: PathBuf,
    runner: &'a dyn ProcessRunner,
}

impl<'a> GitCli<'a> {
    /// Resolves `git` for `env`; `NotFound` when it is not installed. Batch-script wrappers are
    /// refused because `cmd.exe` would re-parse the arguments.
    pub fn new(env: &Env, runner: &'a dyn ProcessRunner, project: &Path) -> Result<Self> {
        let binary = resolve_binary(env, "git", None).ok_or_else(|| BusinessError::NotFound("git".to_owned()))?;

        if binary.is_batch_script {
            return Err(BusinessError::Unsupported(
                "git must be an executable, not a script".to_owned(),
            ));
        }

        let xdg_config = env
            .var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute())
            .unwrap_or_else(|| env.home.join(".config"));

        Ok(Self {
            program: binary.path,
            project: project.to_path_buf(),
            home: env.home.clone(),
            xdg_config,
            runner,
        })
    }

    /// The absolute path of the `git` program.
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// The project directory.
    pub fn project(&self) -> &Path {
        &self.project
    }

    /// Runs `git -C <project> <hardening> <args>` and returns its output, whatever the exit code.
    /// A read never fetches a missing object of a partial clone (`GIT_NO_LAZY_FETCH`, git 2.44 and
    /// newer) and only the [`SAFE_PROTOCOLS`] transports are allowed (older git), so a crafted
    /// repository cannot make it start a program. A clicked network or write action goes through
    /// [`GitCli::run_with`], which leaves the user's own setup alone.
    pub async fn run<I, S>(&self, args: I) -> Result<ExecOutput>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut opts = RunOpts::default();
        opts.env
            .push(("GIT_ALLOW_PROTOCOL".to_owned(), SAFE_PROTOCOLS.to_owned()));
        opts.env.push(("GIT_NO_LAZY_FETCH".to_owned(), "1".to_owned()));

        self.run_with(args, &opts).await
    }

    /// Like [`GitCli::run`] with the time limit and extra environment of `opts` (no transport
    /// restriction unless `opts` sets one); `Timeout` when
    /// git (or a hook it started) does not finish in time.
    pub async fn run_with<I, S>(&self, args: I, opts: &RunOpts) -> Result<ExecOutput>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut spec = ExecSpec::new(&self.program)
            .args([OsString::from("-C"), self.project.clone().into_os_string()])
            .args(HARDENING)
            .args(args)
            .cwd(&self.project)
            .env_remove_prefix("GIT_")
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", &self.xdg_config)
            .timeout(opts.timeout)
            .max_output_bytes(GIT_OUTPUT_CAP);

        for (name, value) in GIT_ENV {
            spec = spec.env(name, value);
        }

        for (name, value) in &opts.env {
            spec = spec.env(name, value);
        }

        let output = self.runner.run(&spec).await?;

        if output.timed_out {
            return Err(BusinessError::Timeout(format!(
                "git did not finish within {} s",
                opts.timeout.as_secs()
            )));
        }

        Ok(output)
    }

    /// Like [`GitCli::run`] but fails unless git exited with status 0.
    pub async fn run_ok<I, S>(&self, args: I) -> Result<ExecOutput>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let output = self.run(args).await?;

        if !output.success() {
            return Err(self.read_failure(&output).await);
        }

        Ok(output)
    }

    /// The error of a failed read: [`NOT_DOWNLOADED`] when git lacks an object and the repository is
    /// a partial clone, else [`failure`].
    async fn read_failure(&self, output: &ExecOutput) -> BusinessError {
        let missing = MISSING_OBJECT_MARKERS
            .iter()
            .any(|marker| output.stderr.contains(marker));

        if missing && self.is_partial_clone().await {
            return BusinessError::Process(NOT_DOWNLOADED.to_owned());
        }

        failure(output)
    }

    async fn is_partial_clone(&self) -> bool {
        let output = self.run(["config", "--get", "extensions.partialClone"]).await;

        matches!(output, Ok(output) if output.success() && !output.stdout.trim().is_empty())
    }

    /// Like [`GitCli::run_with`] but fails with [`redacted_failure`] unless git exited with status 0.
    pub async fn run_network<I, S>(&self, args: I, opts: &RunOpts) -> Result<ExecOutput>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let output = self.run_with(args, opts).await?;

        if !output.success() {
            return Err(redacted_failure(&output));
        }

        Ok(output)
    }

    /// The project's path inside its work tree (`""` at the top, else `sub/dir/`), or `None` when
    /// the project is not inside a git work tree.
    pub async fn prefix(&self) -> Result<Option<String>> {
        let output = self.run(["rev-parse", "--show-prefix"]).await?;

        if output.success() {
            return Ok(Some(output.stdout.trim_end_matches(['\n', '\r']).to_owned()));
        }

        if is_not_a_work_tree(&output.stderr) {
            return Ok(None);
        }

        Err(failure(&output))
    }
}

/// True for git's "not a repository" and "must be run in a work tree" errors.
fn is_not_a_work_tree(stderr: &str) -> bool {
    stderr.contains("not a git repository") || stderr.contains("must be run in a work tree")
}

/// A process error carrying git's first `fatal:`/`error:` line (no file contents).
pub(super) fn failure(output: &ExecOutput) -> BusinessError {
    BusinessError::Process(first_error_line(output))
}

/// git's first `fatal:`/`error:` line, cut to 300 characters.
fn first_error_line(output: &ExecOutput) -> String {
    output
        .stderr
        .lines()
        .find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
        .unwrap_or("git failed")
        .chars()
        .take(300)
        .collect()
}

/// Like [`failure`] for a command that writes or talks to a remote (its hooks and its remote can
/// print anything): the first `fatal:`/`error:` line, else the first line git printed, with URL
/// userinfo (`https://user:token@host`) cut out and credential-like text masked before the line
/// leaves the process.
pub(super) fn redacted_failure(output: &ExecOutput) -> BusinessError {
    let line = if has_error_line(output) {
        first_error_line(output)
    } else {
        first_line(output)
    };

    BusinessError::Process(redact_text(&strip_userinfo(&line)))
}

fn has_error_line(output: &ExecOutput) -> bool {
    output
        .stderr
        .lines()
        .any(|line| line.starts_with("fatal:") || line.starts_with("error:"))
}

/// The first non-empty line of stderr, cut to 300 characters; `git failed` when there is none.
fn first_line(output: &ExecOutput) -> String {
    output
        .stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("git failed")
        .chars()
        .take(300)
        .collect()
}

/// `text` with the userinfo of every `scheme://user:password@host` removed.
fn strip_userinfo(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find("://") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);

        let authority_end = tail
            .find(|character: char| character == '/' || character.is_whitespace() || "'\"<>".contains(character))
            .unwrap_or(tail.len());
        let authority = &tail[..authority_end];

        match authority.rfind('@') {
            Some(index) => out.push_str(&tail[index + 1..authority_end]),
            None => out.push_str(authority),
        }

        rest = &tail[authority_end..];
    }

    out.push_str(rest);
    out
}

/// `path` relative to the project: `repo_path` without the project `prefix`; `None` outside it.
pub fn project_relative(repo_path: &str, prefix: &str) -> Option<String> {
    let relative = repo_path.strip_prefix(prefix)?;
    (!relative.is_empty()).then(|| relative.to_owned())
}
