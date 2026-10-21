//! Runs the official installer of a [`Companion`] (Harness Dashboard, Harness Memory, the Harness
//! Skills for OpenCode) for the "Recommended setup" step and the "Project memory" install, the only
//! place the app runs an install command (and the only place allowed to name a shell, enforced by
//! the `installer-shell` guard rule).
//!
//! - Command: fixed per companion and OS ([`installer`]); nothing from the webview reaches it.
//! - Process: its own process group, stdin closed at once, killed with its group on drop or after
//!   [`INSTALL_TIMEOUT`]. On Unix, whatever is left of the group is killed once the installer
//!   exited and its output was drained (Windows: background processes it started may outlive it).
//! - Output: streamed line by line to a sink with terminal escapes stripped and secrets redacted;
//!   a line keeps at most [`MAX_LOG_LINE_BYTES`], the whole log at most [`MAX_LOG_LINES`] lines
//!   and [`MAX_LOG_BYTES`] (each line counting its terminator).
//! - Success: exit code 0 **and** the companion found with a version afterwards
//!   ([`Companion::installed`]: its binary reporting one, or the Harness Skills' install record
//!   holding one), since `curl … | sh` exits with 0 when the download failed.
//! - `PATH`: the Harness Skills' installer runs `node`, so the usual install directories
//!   ([`extra_search_dirs`]) are appended to its `PATH`, the same places the setup step looks for
//!   `node` after `PATH`. It also gets OpenCode's spawn variables (`OPENCODE_DISABLE_AUTOUPDATE`),
//!   so an `opencode` it runs does not update itself.

use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::time::{Instant, sleep_until, timeout};

use super::catalog::{
    DASHBOARD_BINARY, DASHBOARD_INSTALL_PS1, DASHBOARD_INSTALL_PS1_SCRIPT, DASHBOARD_INSTALL_SH, MEMORY_BINARY,
    MEMORY_INSTALL_PS1, MEMORY_INSTALL_PS1_SCRIPT, MEMORY_INSTALL_SH, SKILLS_INSTALL_PS1, SKILLS_INSTALL_PS1_SCRIPT,
    SKILLS_INSTALL_SH,
};
use super::dashboard::resolve_dashboard;
use super::detect::dashboard_version;
use super::installer_log::{Log, Splitter};
use super::memory_binary::{memory_version, resolve_memory};
use super::skills_record::opencode_skills_installed;
use crate::env::{Env, OsKind};
use crate::error::Result;
use crate::harnesses::opencode::SPAWN_ENV;
use crate::model::{DashboardInstallResult, InstallLogLine, InstallStream};
use crate::process::{ChildHandle, ExecSpec, ExitInfo, ProcessRunner, ProcessSpawner, TokioRunner, extra_search_dirs};

/// Time limit of one installer run.
pub const INSTALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Longest log line passed to the sink, in bytes (longer lines are cut).
pub const MAX_LOG_LINE_BYTES: usize = 2 * 1024;

/// Most log bytes passed to the sink in one run, one per line terminator included (later lines
/// are dropped).
pub const MAX_LOG_BYTES: usize = 256 * 1024;

/// Most lines passed to the sink in one run (later lines are dropped).
pub const MAX_LOG_LINES: usize = 5_000;

/// How long the output is still read after the installer exited (a background child may hold
/// the pipes open).
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// How long a killed installer is waited for.
const KILL_GRACE: Duration = Duration::from_secs(5);

/// Read buffer size.
const CHUNK: usize = 8 * 1024;

/// A companion whose official installer the app runs once the user confirms it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Companion {
    /// Harness Dashboard (`harness-dashboard`).
    Dashboard,
    /// Harness Memory (`harness-memory`).
    Memory,
    /// Harness Skills for OpenCode: files in OpenCode's config directory, no binary (see
    /// [`super::skills_record`]).
    OpenCodeSkills,
}

/// A companion as found on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Installed {
    /// Nothing of it was found.
    Missing,
    /// Found, but without a version: its executable did not report one, or the Harness Skills'
    /// install record holds none.
    NoVersion,
    /// Found, with this version.
    Version(String),
}

impl Installed {
    /// The version, when found with one.
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Version(version) => Some(version),
            Self::Missing | Self::NoVersion => None,
        }
    }
}

impl Companion {
    /// Its name in messages: the executable's for a binary.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dashboard => DASHBOARD_BINARY,
            Self::Memory => MEMORY_BINARY,
            Self::OpenCodeSkills => "Harness Skills for OpenCode",
        }
    }

    /// Whether and which version of it is installed: a binary's executable (where its installer puts
    /// it or anywhere on `PATH`) and the version it reports within the version probe's time limit;
    /// the Harness Skills' install record and the version it holds.
    pub async fn installed(self, env: &Env, runner: &dyn ProcessRunner) -> Installed {
        match self {
            Self::Dashboard => match resolve_dashboard(env) {
                Some(binary) => probed(dashboard_version(env, runner, &binary).await),
                None => Installed::Missing,
            },
            Self::Memory => match resolve_memory(env) {
                Some(binary) => probed(memory_version(env, runner, &binary).await),
                None => Installed::Missing,
            },
            Self::OpenCodeSkills => opencode_skills_installed(env),
        }
    }

    /// Where the version comes from, for the summary.
    const fn version_source(self) -> &'static str {
        match self {
            Self::Dashboard => "`harness-dashboard version`",
            Self::Memory => "`harness-memory --version`",
            Self::OpenCodeSkills => "the Harness Skills install record",
        }
    }

    /// The Unix installer, the Windows installer's PowerShell script and its copyable command.
    const fn scripts(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Dashboard => (
                DASHBOARD_INSTALL_SH,
                DASHBOARD_INSTALL_PS1_SCRIPT,
                DASHBOARD_INSTALL_PS1,
            ),
            Self::Memory => (MEMORY_INSTALL_SH, MEMORY_INSTALL_PS1_SCRIPT, MEMORY_INSTALL_PS1),
            Self::OpenCodeSkills => (SKILLS_INSTALL_SH, SKILLS_INSTALL_PS1_SCRIPT, SKILLS_INSTALL_PS1),
        }
    }

    /// Directories appended to its installer's `PATH`: the usual install directories for the
    /// Harness Skills, whose installer runs `node`.
    fn search_dirs(self, env: &Env) -> Vec<PathBuf> {
        match self {
            Self::Dashboard | Self::Memory => Vec::new(),
            Self::OpenCodeSkills => extra_search_dirs(env),
        }
    }

    /// Variables set for its installer: the Harness Skills' installer may run `opencode`, which
    /// gets OpenCode's own spawn variables so it never updates itself meanwhile.
    fn installer_env(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Dashboard | Self::Memory => &[],
            Self::OpenCodeSkills => SPAWN_ENV,
        }
    }
}

/// A binary companion that was found, with the `version` its probe reported.
fn probed(version: Option<String>) -> Installed {
    match version {
        Some(version) => Installed::Version(version),
        None => Installed::NoVersion,
    }
}

/// The program and arguments that run a companion's installer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallerSpec {
    /// Absolute path of the interpreter.
    pub program: PathBuf,
    /// Its arguments.
    pub args: Vec<String>,
    /// The copyable equivalent, for display.
    pub command: &'static str,
}

/// The installer of `companion` for `env`'s OS: the system `sh` running the `install.sh` pipe on
/// Linux and macOS; on Windows, Windows PowerShell from
/// `<SystemRoot>\System32\WindowsPowerShell\v1.0` (`SystemRoot` defaults to `C:\Windows`) running
/// `irm <install.ps1> | iex` without the user's profile.
pub fn installer(env: &Env, companion: Companion) -> InstallerSpec {
    let (install_sh, install_ps1_script, install_ps1) = companion.scripts();

    match env.os {
        OsKind::Linux | OsKind::MacOs => InstallerSpec {
            program: PathBuf::from("/bin/sh"), // guard-allow: installer-shell
            args: vec!["-c".to_owned(), install_sh.to_owned()],
            command: install_sh,
        },
        OsKind::Windows => {
            let system_root = env.var("SystemRoot").unwrap_or(r"C:\Windows");
            let program = PathBuf::from(system_root)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe"); // guard-allow: installer-shell
            let args = [
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                install_ps1_script,
            ];

            InstallerSpec {
                program,
                args: args.map(str::to_owned).to_vec(),
                command: install_ps1,
            }
        }
    }
}

/// Runs the installer of `companion` (see the module docs), passing each output line to `sink`.
/// Errors only when the installer could not start; a failed install is a result with `ok: false`.
pub async fn run_installer(
    env: &Env,
    companion: Companion,
    spawner: &dyn ProcessSpawner,
    sink: impl FnMut(InstallLogLine),
) -> Result<DashboardInstallResult> {
    run_installer_with(env, companion, spawner, &TokioRunner, INSTALL_TIMEOUT, sink).await
}

/// [`run_installer`] with the runner of the version probe and the time limit given.
pub async fn run_installer_with(
    env: &Env,
    companion: Companion,
    spawner: &dyn ProcessSpawner,
    runner: &dyn ProcessRunner,
    time_limit: Duration,
    sink: impl FnMut(InstallLogLine),
) -> Result<DashboardInstallResult> {
    let installer = installer(env, companion);
    let spec = ExecSpec::new(&installer.program)
        .args(installer.args.iter().cloned())
        .cwd(&env.home)
        .env("NO_COLOR", "1")
        .envs(companion.installer_env().iter().copied())
        .search_dirs(companion.search_dirs(env));

    let process = spawner.spawn(&spec)?;
    // Closed at once: an installer that asks a question reads end of input instead of hanging.
    drop(process.stdin);

    let mut log = Log::new(sink);
    let run = drive(process.stdout, process.stderr, process.child, time_limit, &mut log).await;

    // A stopped installer fails whatever it left behind, so nothing is probed then.
    let found = if run.timed_out {
        Installed::Missing
    } else {
        companion.installed(env, runner).await
    };
    let exit_code = run.exit.and_then(|exit| exit.code);
    let ok = exit_code == Some(0) && !run.timed_out && found.version().is_some();

    let summary = summary(
        &run,
        time_limit,
        exit_code,
        companion,
        &found,
        log.last_stderr.as_deref(),
    );

    Ok(DashboardInstallResult {
        ok,
        summary: Some(summary),
        exit_code,
        timed_out: run.timed_out,
        truncated: log.truncated,
        version: found.version().map(str::to_owned),
        command: installer.command.to_owned(),
    })
}

/// How the installer process ended.
struct Run {
    /// Its exit, when it ended by itself.
    exit: Option<ExitInfo>,
    timed_out: bool,
}

/// Reads both streams into `log` until the installer exited and its pipes closed (or
/// [`DRAIN_GRACE`] passed), killing its process group after `time_limit`.
async fn drive<F: FnMut(InstallLogLine)>(
    mut stdout: Box<dyn AsyncRead + Send + Unpin>,
    mut stderr: Box<dyn AsyncRead + Send + Unpin>,
    mut child: Box<dyn ChildHandle>,
    time_limit: Duration,
    log: &mut Log<F>,
) -> Run {
    let mut deadline = Instant::now() + time_limit;
    let mut out = Splitter::new(InstallStream::Stdout);
    let mut err = Splitter::new(InstallStream::Stderr);
    let mut out_chunk = vec![0_u8; CHUNK];
    let mut err_chunk = vec![0_u8; CHUNK];
    let (mut out_open, mut err_open) = (true, true);
    let mut exit: Option<ExitInfo> = None;

    while out_open || err_open || exit.is_none() {
        tokio::select! {
            read = stdout.read(&mut out_chunk), if out_open => match read {
                Ok(read) if read > 0 => out.push(&out_chunk[..read], log),
                _ => out_open = false,
            },
            read = stderr.read(&mut err_chunk), if err_open => match read {
                Ok(read) if read > 0 => err.push(&err_chunk[..read], log),
                _ => err_open = false,
            },
            waited = child.wait(), if exit.is_none() => {
                exit = Some(waited.unwrap_or_default());
                deadline = deadline.min(Instant::now() + DRAIN_GRACE);
            },
            () = sleep_until(deadline) => break,
        }
    }

    out.finish(log);
    err.finish(log);

    if exit.is_some() {
        // Background processes the installer left behind (holding the pipes or not) end with it.
        child.kill_process_group();

        return Run { exit, timed_out: false };
    }

    child.start_kill().ok();
    timeout(KILL_GRACE, child.wait()).await.ok();

    Run {
        exit: None,
        timed_out: true,
    }
}

/// The one-line outcome shown with the result.
fn summary(
    run: &Run,
    time_limit: Duration,
    exit_code: Option<i32>,
    companion: Companion,
    found: &Installed,
    last_stderr: Option<&str>,
) -> String {
    if run.timed_out {
        let limit = match time_limit.as_secs() / 60 {
            0 => format!("{} seconds", time_limit.as_secs()),
            minutes => format!("{minutes} minutes"),
        };

        return format!("The installer was stopped after {limit} without finishing.");
    }

    if exit_code != Some(0) {
        if let Some(line) = last_stderr {
            return line.to_owned();
        }

        return match exit_code {
            Some(code) => format!("The installer exited with code {code}."),
            None => "The installer was ended by a signal.".to_owned(),
        };
    }

    let name = companion.name();

    match (found, companion) {
        (Installed::Version(version), _) => format!("Installed {name} {version}."),
        (Installed::NoVersion, _) => format!(
            "The installer finished, but {} did not report a version.",
            companion.version_source()
        ),
        (Installed::Missing, Companion::Dashboard | Companion::Memory) => {
            format!("The installer finished, but {name} was not found afterwards; the download may have failed.")
        }
        (Installed::Missing, Companion::OpenCodeSkills) => "The installer finished, but the Harness Skills install \
             record was not found afterwards. The install may have failed."
            .to_owned(),
    }
}
