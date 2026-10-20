//! Shell-free process execution behind injectable traits.
//!
//! - [`ProcessRunner`] runs a command to completion with a timeout and an output cap
//!   (version probes, `omp plugin …`, git); [`BlockingRunner`] does the same for synchronous
//!   callers.
//! - [`ProcessSpawner`] starts a long-lived child with piped stdio (RPC agent sessions).
//! - [`resolve`] finds harness binaries without asking a shell for `PATH`.
//!
//! Every spawn uses a fixed, absolute program path plus an explicit argv; a shell is never
//! involved. On Unix each child leads its own process group so a kill reaches its descendants; on
//! Windows a kill of a child that was not reaped yet ends its process tree (`taskkill /T`).
//!
//! # Why boxed futures
//!
//! The traits return [`BoxFuture`] instead of using `async fn` in traits: callers hold them as
//! `&dyn ProcessRunner` / `Box<dyn ChildHandle>` (native `async fn` in traits is not
//! dyn-compatible), and the futures must be `Send` for multi-threaded Tauri commands. One boxed
//! allocation per process is negligible next to spawning it, and no `async-trait` macro is needed.

#[cfg(unix)]
mod appimage;
mod blocking;
mod resolve;
mod runner;
mod spawner;
#[cfg(windows)]
mod windows_tree;

pub use blocking::{BlockingRunner, StdRunner};
pub use resolve::{
    ResolvedBinary, absolute_path_dirs, extra_search_dirs, find_override, is_batch_script, is_executable_file,
    launcher_search_dirs, resolve_binary,
};
pub use runner::{ExecOutput, ProcessRunner, TokioRunner};
pub use spawner::{ChildHandle, ProcessSpawner, SpawnedProcess, TokioSpawner};

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use tokio::process::{Child, Command};

use crate::error::{BusinessError, Result};

/// A `Send` future boxed for use behind trait objects.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Default timeout for run-to-completion commands.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Default per-stream output cap for run-to-completion commands (1 MiB).
pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 1024 * 1024;

/// What to execute. The child inherits the parent environment (with an AppImage launcher's changes
/// undone) minus the variables named by `env_remove_prefixes`, plus `env` overrides, with
/// `search_dirs` appended to its `PATH`. `Debug` shows the names of `env` without their values
/// (the OpenCode server password is one) and the size of `stdin` without its bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct ExecSpec {
    /// Absolute path of the executable (resolve it first, see [`resolve_binary`]).
    pub program: PathBuf,
    /// Arguments, passed verbatim (no shell, no globbing).
    pub args: Vec<OsString>,
    /// One more argument after `args`. On Windows it is passed without any quoting
    /// (`CommandExt::raw_arg`), which `cmd.exe /S /C "<command>"` needs; elsewhere it is a normal
    /// argument.
    pub raw_arg: Option<OsString>,
    /// Working directory; inherits the parent's when `None`.
    pub cwd: Option<PathBuf>,
    /// Extra environment variables layered over the inherited environment.
    pub env: Vec<(OsString, OsString)>,
    /// Inherited variables whose names start with one of these prefixes are not passed on
    /// (ASCII case-insensitively on Windows); `env` overrides are set regardless.
    pub env_remove_prefixes: Vec<OsString>,
    /// Time limit for [`ProcessRunner::run`] (ignored by spawners).
    pub timeout: Duration,
    /// Per-stream (stdout, stderr) capture cap for [`ProcessRunner::run`] (ignored by spawners).
    pub max_output_bytes: usize,
    /// Which end of a stream longer than the cap [`ProcessRunner::run`] keeps: the first bytes
    /// (default) or the last ones (see [`ExecSpec::keep_output_tail`]).
    pub keep_tail: bool,
    /// Directories appended to the child's `PATH` when missing from it. Apps started from the
    /// macOS Finder (and some Linux launchers) lack the login shell's `PATH`, so a launcher script
    /// such as `#!/usr/bin/env bun` could not find its interpreter without them.
    pub search_dirs: Vec<PathBuf>,
    /// Bytes [`ProcessRunner::run`] writes to the child's stdin before closing it; `None` gives the
    /// child a closed stdin (ignored by spawners).
    pub stdin: Option<Vec<u8>>,
    /// [`TokioRunner`] captures stdout through a private temporary file instead of a pipe (see
    /// [`ExecSpec::stdout_through_file`]); ignored by the other runners and the spawners.
    pub stdout_through_file: bool,
}

impl fmt::Debug for ExecSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let env: Vec<String> = self
            .env
            .iter()
            .map(|(name, _)| format!("{}=<redacted>", name.to_string_lossy()))
            .collect();

        formatter
            .debug_struct("ExecSpec")
            .field("program", &self.program)
            .field("args", &self.args)
            .field("raw_arg", &self.raw_arg)
            .field("cwd", &self.cwd)
            .field("env", &env)
            .field("env_remove_prefixes", &self.env_remove_prefixes)
            .field("timeout", &self.timeout)
            .field("max_output_bytes", &self.max_output_bytes)
            .field("keep_tail", &self.keep_tail)
            .field("search_dirs", &self.search_dirs)
            .field(
                "stdin",
                &self.stdin.as_ref().map(|bytes| format!("<{} bytes>", bytes.len())),
            )
            .field("stdout_through_file", &self.stdout_through_file)
            .finish()
    }
}

impl ExecSpec {
    /// A spec with default timeout and output cap.
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            raw_arg: None,
            cwd: None,
            env: Vec::new(),
            env_remove_prefixes: Vec::new(),
            search_dirs: Vec::new(),
            timeout: DEFAULT_TIMEOUT,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            keep_tail: false,
            stdin: None,
            stdout_through_file: false,
        }
    }

    /// Appends arguments.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Sets the argument after `args` (see [`ExecSpec::raw_arg`]).
    pub fn raw_arg(mut self, arg: impl Into<OsString>) -> Self {
        self.raw_arg = Some(arg.into());
        self
    }

    /// Sets the working directory.
    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Adds an environment override.
    pub fn env(mut self, name: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((name.into(), value.into()));
        self
    }

    /// Adds environment overrides.
    pub fn envs<I, K, V>(mut self, vars: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<OsString>,
        V: Into<OsString>,
    {
        self.env
            .extend(vars.into_iter().map(|(name, value)| (name.into(), value.into())));
        self
    }

    /// Drops every inherited variable whose name starts with `prefix` (see
    /// [`ExecSpec::env_remove_prefixes`]).
    pub fn env_remove_prefix(mut self, prefix: impl Into<OsString>) -> Self {
        self.env_remove_prefixes.push(prefix.into());
        self
    }

    /// Appends directories to the child's `PATH` (see [`ExecSpec::search_dirs`]).
    pub fn search_dirs(mut self, dirs: impl IntoIterator<Item = PathBuf>) -> Self {
        self.search_dirs.extend(dirs);
        self
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the per-stream output cap.
    pub fn max_output_bytes(mut self, max: usize) -> Self {
        self.max_output_bytes = max;
        self
    }

    /// Keeps the last `max_output_bytes` of each stream instead of the first ones. A stream cut
    /// that way loses its first, cut-off word too (the bytes up to the first whitespace): a secret
    /// cut in half could no longer be recognized and redacted.
    pub fn keep_output_tail(mut self) -> Self {
        self.keep_tail = true;
        self
    }

    /// Sets the bytes written to the child's stdin (see [`ExecSpec::stdin`]).
    pub fn stdin(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(bytes.into());
        self
    }

    /// Captures stdout through a private temporary file rather than a pipe, for a program that
    /// exits without writing what did not fit in the pipe (OpenCode, a Bun program, keeps only the
    /// first 64 KiB of `opencode debug config` written to a pipe). The cap still applies to what
    /// is read back; the file is removed once the run ends.
    pub fn stdout_through_file(mut self) -> Self {
        self.stdout_through_file = true;
        self
    }

    /// Short label for error messages: the program's file name only (never arguments, which
    /// may carry user text).
    pub(crate) fn label(&self) -> String {
        self.program
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "process".to_owned())
    }
}

/// How a child process ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExitInfo {
    /// Exit code, when the process exited normally.
    pub code: Option<i32>,
    /// Terminating signal on Unix.
    pub signal: Option<i32>,
}

impl From<ExitStatus> for ExitInfo {
    fn from(status: ExitStatus) -> Self {
        #[cfg(unix)]
        let signal = std::os::unix::process::ExitStatusExt::signal(&status);
        #[cfg(not(unix))]
        let signal = None;

        Self {
            code: status.code(),
            signal,
        }
    }
}

/// Builds a shell-free command: absolute program, explicit argv, `kill_on_drop`, own process group.
fn build_command(spec: &ExecSpec, stdin: Stdio) -> Result<Command> {
    let mut command = Command::from(build_std_command(spec, stdin)?);
    command.kill_on_drop(true);

    Ok(command)
}

/// The synchronous form of [`build_command`] (without `kill_on_drop`, which `std` lacks): absolute
/// program, explicit argv, piped stdout and stderr, the spec's environment and `PATH`, own process
/// group on Unix.
fn build_std_command(spec: &ExecSpec, stdin: Stdio) -> Result<std::process::Command> {
    if !spec.program.is_absolute() {
        return Err(BusinessError::InvalidInput(format!(
            "program must be an absolute path: {}",
            spec.label()
        )));
    }

    let mut command = std::process::Command::new(&spec.program);

    // Inside an AppImage, give the child the host's environment, not the launcher's.
    #[cfg(unix)]
    let host_env = appimage::host_env(std::env::vars_os());
    #[cfg(not(unix))]
    let host_env: Vec<(OsString, Option<OsString>)> = Vec::new();

    for (name, value) in &host_env {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }

    // Removals first: the overrides in `spec.env` must survive them.
    for name in removed_vars(&spec.env_remove_prefixes) {
        command.env_remove(name);
    }

    command
        .args(&spec.args)
        .envs(spec.env.iter().map(|(name, value)| (name, value)))
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(raw) = &spec.raw_arg {
        #[cfg(windows)]
        std::os::windows::process::CommandExt::raw_arg(&mut command, raw);
        #[cfg(not(windows))]
        command.arg(raw);
    }

    if let Some(cwd) = &spec.cwd {
        command.current_dir(cwd);
    }

    if let Some(path) = child_path(spec, &host_env) {
        command.env("PATH", path);
    }

    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);

    Ok(command)
}

/// The names of the current process's variables that start with one of `prefixes`, read at spawn
/// time so variables set after startup are covered too.
fn removed_vars(prefixes: &[OsString]) -> Vec<OsString> {
    if prefixes.is_empty() {
        return Vec::new();
    }

    std::env::vars_os()
        .map(|(name, _)| name)
        .filter(|name| prefixes.iter().any(|prefix| has_prefix(name, prefix)))
        .collect()
}

/// True when variable `name` starts with `prefix`; Windows variable names ignore ASCII case.
fn has_prefix(name: &OsStr, prefix: &OsStr) -> bool {
    let name = name.as_encoded_bytes();
    let prefix = prefix.as_encoded_bytes();

    let Some(head) = name.get(..prefix.len()) else {
        return false;
    };

    if cfg!(windows) {
        head.eq_ignore_ascii_case(prefix)
    } else {
        head == prefix
    }
}

/// True when variable `name` is `PATH` (any ASCII case on Windows).
fn is_path_var(name: &OsStr) -> bool {
    name.len() == 4 && has_prefix(name, OsStr::new("PATH"))
}

/// The child's `PATH` with `spec.search_dirs` appended, or `None` when nothing is appended.
/// The base is what the child would otherwise get: the last `PATH` override in `spec.env`, else
/// the AppImage-restored value in `host_env`, else the inherited one.
fn child_path(spec: &ExecSpec, host_env: &[(OsString, Option<OsString>)]) -> Option<OsString> {
    if spec.search_dirs.is_empty() {
        return None;
    }

    let base = spec
        .env
        .iter()
        .rev()
        .find(|(name, _)| is_path_var(name))
        .map(|(_, value)| Some(value.clone()))
        .or_else(|| {
            host_env
                .iter()
                .find(|(name, _)| is_path_var(name))
                .map(|(_, value)| value.clone())
        })
        .unwrap_or_else(|| std::env::var_os("PATH"));

    appended_path(base.as_deref(), &spec.search_dirs)
}

/// `base` followed by every directory of `dirs` it does not already contain; `None` when nothing
/// is missing or the result cannot be joined (an entry containing the separator). Empty `base`
/// entries (the working directory) are dropped.
fn appended_path(base: Option<&OsStr>, dirs: &[PathBuf]) -> Option<OsString> {
    let mut entries: Vec<PathBuf> = base
        .map(|base| {
            std::env::split_paths(base)
                .filter(|entry| !entry.as_os_str().is_empty())
                .collect()
        })
        .unwrap_or_default();
    let before = entries.len();

    for dir in dirs {
        if !entries.contains(dir) {
            entries.push(dir.clone());
        }
    }

    if entries.len() == before {
        return None;
    }

    std::env::join_paths(entries).ok()
}

/// Spawns `command`, mapping failures to a [`BusinessError`] that names only the program.
fn spawn_command(mut command: Command, spec: &ExecSpec) -> Result<Child> {
    command.spawn().map_err(|error| spawn_error(&error, spec))
}

/// A failed spawn of `spec` as a [`BusinessError`] that names only the program.
fn spawn_error(error: &std::io::Error, spec: &ExecSpec) -> BusinessError {
    match error.kind() {
        std::io::ErrorKind::NotFound => BusinessError::NotFound(format!("executable {}", spec.label())),
        _ => BusinessError::Process(format!("failed to start {}: {error}", spec.label())),
    }
}

/// Kills the child and its whole process group (Unix) or process tree (Windows). A no-op once the
/// child was reaped (see [`ChildHandle::kill_process_group`] for what is left after that).
fn kill_tree(child: &mut Child) -> Result<()> {
    // `id()` is `None` once tokio reaped the child; the pid (and its group id) may then be reused.
    let Some(pid) = child.id() else {
        return Ok(());
    };

    #[cfg(unix)]
    kill_group(pid);
    #[cfg(windows)]
    windows_tree::kill_process_tree(pid);

    match child.start_kill() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => Ok(()),
        Err(error) => Err(BusinessError::Process(format!("failed to kill process: {error}"))),
    }
}

/// Sends SIGKILL to the process group led by `pid` (the child was spawned with `process_group(0)`).
#[cfg(unix)]
fn kill_group(pid: u32) {
    signal_group(pid, nix::sys::signal::Signal::SIGKILL);
}

/// Sends SIGTERM to the process group led by `pid`, asking it to shut down.
#[cfg(unix)]
fn terminate_group(pid: u32) {
    signal_group(pid, nix::sys::signal::Signal::SIGTERM);
}

#[cfg(unix)]
fn signal_group(pid: u32, signal: nix::sys::signal::Signal) {
    use nix::sys::signal::killpg;
    use nix::unistd::Pid;

    let Ok(raw) = i32::try_from(pid) else {
        return;
    };

    // ESRCH (group already gone) is the expected race; anything else is not actionable here.
    if let Err(error) = killpg(Pid::from_raw(raw), signal)
        && error != nix::errno::Errno::ESRCH
    {
        tracing::debug!(%error, "killpg failed");
    }
}
