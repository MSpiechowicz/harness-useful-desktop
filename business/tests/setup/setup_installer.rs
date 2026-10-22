//! Running the companion installers: the fixed command per companion and OS, streamed and cleaned output, success
//! only when the binary answers afterwards, and the time limit.

use crate::setup_support;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use harness_business::Result;
use harness_business::ecosystem::catalog::{
    DASHBOARD_INSTALL_PS1, DASHBOARD_INSTALL_SH, MEMORY_INSTALL_PS1, MEMORY_INSTALL_SH,
};
use harness_business::ecosystem::installer::{MAX_LOG_LINE_BYTES, MAX_LOG_LINES};
use harness_business::ecosystem::{Companion, installer, run_installer, run_installer_with};
use harness_business::env::{Env, OsKind};
use harness_business::model::{DashboardInstallResult, InstallLogLine, InstallStream};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ExitInfo, ProcessRunner};
use harness_business::testing::DuplexSpawner;
use pretty_assertions::assert_eq;
use setup_support::{EXITED_0, assert_unix_installer, assert_windows_installer, play, powershell};
use tokio::io::AsyncWriteExt;

/// Answers every version probe with `stdout`, keeping the arguments of each.
struct Prints {
    stdout: &'static str,
    probes: Mutex<Vec<Vec<String>>>,
}

impl Prints {
    fn new(stdout: &'static str) -> Self {
        Self {
            stdout,
            probes: Mutex::new(Vec::new()),
        }
    }

    fn probes(&self) -> Vec<Vec<String>> {
        self.probes.lock().unwrap().clone()
    }
}

impl ProcessRunner for Prints {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        let args = spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        self.probes.lock().unwrap().push(args);

        Box::pin(async {
            Ok(ExecOutput {
                status: Some(0),
                stdout: self.stdout.to_owned(),
                ..ExecOutput::default()
            })
        })
    }
}

/// `harness-dashboard version` output.
const DASHBOARD_VERSION: &str = "1.26.0\n";

fn linux(home: &Path) -> Env {
    Env::for_tests(home).with_os(OsKind::Linux)
}

/// Places an executable `name` in `~/.local/bin`, where companions are looked up.
fn install_binary(home: &Path, name: &str) {
    let binary = home.join(".local/bin").join(name);
    std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
    std::fs::write(&binary, "").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// Runs the installer against a scripted peer and returns the result, the lines and the spec.
async fn run_scripted(
    env: &Env,
    companion: Companion,
    runner: &dyn ProcessRunner,
    stdout: &'static str,
    stderr: &'static str,
    exit: ExitInfo,
) -> (DashboardInstallResult, Vec<InstallLogLine>, ExecSpec) {
    let (spawner, mut peers) = DuplexSpawner::new();
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink_lines = Arc::clone(&lines);

    let player = tokio::spawn(async move { play(peers.recv().await.unwrap(), stdout, stderr, exit).await });
    let result = run_installer_with(env, companion, &spawner, runner, Duration::from_secs(30), move |line| {
        sink_lines.lock().unwrap().push(line);
    })
    .await
    .unwrap();

    let spec = player.await.unwrap();
    let lines = lines.lock().unwrap().clone();
    (result, lines, spec)
}

fn line(stream: InstallStream, text: &str) -> InstallLogLine {
    InstallLogLine {
        stream,
        line: text.to_owned(),
    }
}

#[test]
fn the_installer_command_is_fixed_per_os() {
    let home = tempfile::tempdir().unwrap();
    let download = "curl --proto '=https' --tlsv1.2 -fsSL https://";
    assert_unix_installer(home.path(), Companion::Dashboard, DASHBOARD_INSTALL_SH, download);

    let windows = Env::for_tests(home.path()).with_os(OsKind::Windows);
    let script = "irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/install.ps1 | iex";

    let default_root = installer(&windows, Companion::Dashboard);
    assert_windows_installer(&default_root, script, DASHBOARD_INSTALL_PS1);

    let custom_root = installer(&windows.with_var("SystemRoot", r"D:\Win"), Companion::Dashboard);
    assert_eq!(custom_root.program, powershell(r"D:\Win"));
}

#[test]
fn the_memory_installer_command_is_fixed_per_os() {
    let home = tempfile::tempdir().unwrap();
    let download =
        "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-memory/";
    assert_unix_installer(home.path(), Companion::Memory, MEMORY_INSTALL_SH, download);

    let windows = installer(&Env::for_tests(home.path()).with_os(OsKind::Windows), Companion::Memory);
    let script = "irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-memory/main/install.ps1 | iex";
    assert_windows_installer(&windows, script, MEMORY_INSTALL_PS1);
}

#[tokio::test]
async fn the_memory_install_is_ok_once_harness_memory_reports_its_version() {
    let home = tempfile::tempdir().unwrap();
    install_binary(home.path(), "harness-memory");
    let env = linux(home.path());
    let runner = Prints::new("harness-memory 1.0.0\n");

    let (result, _, spec) = run_scripted(&env, Companion::Memory, &runner, "done\n", "", EXITED_0).await;

    assert_eq!(spec.args[1], MEMORY_INSTALL_SH);
    assert!(result.ok);
    assert_eq!(result.version.as_deref(), Some("1.0.0"));
    assert_eq!(result.summary.as_deref(), Some("Installed harness-memory 1.0.0."));
    assert_eq!(result.command, MEMORY_INSTALL_SH);
    assert_eq!(runner.probes(), vec![vec!["--version".to_owned()]]);
}

#[tokio::test]
async fn a_memory_install_without_harness_memory_afterwards_is_not_ok() {
    let home = tempfile::tempdir().unwrap();
    // Another companion being there does not count.
    install_binary(home.path(), "harness-dashboard");
    let env = linux(home.path());
    let runner = Prints::new("harness-memory 1.0.0\n");

    let (result, _, _) = run_scripted(&env, Companion::Memory, &runner, "done\n", "", EXITED_0).await;

    assert!(!result.ok);
    assert_eq!(result.version, None);
    let summary = result.summary.unwrap();
    assert!(summary.contains("harness-memory was not found afterwards"), "{summary}");
    assert!(runner.probes().is_empty());
}

#[tokio::test]
async fn a_memory_server_that_answers_something_else_is_not_ok() {
    let home = tempfile::tempdir().unwrap();
    install_binary(home.path(), "harness-memory");
    let env = linux(home.path());

    let (result, _, _) = run_scripted(
        &env,
        Companion::Memory,
        &Prints::new(DASHBOARD_VERSION),
        "",
        "",
        EXITED_0,
    )
    .await;

    assert!(!result.ok);
    assert_eq!(
        result.summary.as_deref(),
        Some("The installer finished, but `harness-memory --version` did not report a version.")
    );
}

#[tokio::test]
async fn an_exit_code_of_zero_without_the_binary_is_not_ok() {
    let home = tempfile::tempdir().unwrap();
    let env = linux(home.path());
    let stdout = "\u{1b}[36m==>\u{1b}[0m Downloading\r\nAuthorization: Bearer CANARY0123456789abcdefXYZ\n";
    let stderr = "curl: (6) Could not resolve host: raw.githubusercontent.com";

    let (result, lines, spec) = run_scripted(
        &env,
        Companion::Dashboard,
        &Prints::new(DASHBOARD_VERSION),
        stdout,
        stderr,
        EXITED_0,
    )
    .await;

    assert_eq!(spec.program, installer(&env, Companion::Dashboard).program);
    assert_eq!(spec.cwd.as_deref(), Some(home.path()));

    assert!(!result.ok);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.version, None);
    assert!(!result.timed_out);
    assert_eq!(result.command, DASHBOARD_INSTALL_SH);
    let summary = result.summary.unwrap();
    assert!(summary.contains("not found afterwards"), "{summary}");

    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], line(InstallStream::Stdout, "==> Downloading"));
    assert!(!lines[1].line.contains("CANARY0123456789"), "{:?}", lines[1]);
    assert_eq!(
        lines[2],
        line(
            InstallStream::Stderr,
            "curl: (6) Could not resolve host: raw.githubusercontent.com"
        )
    );
}

#[tokio::test]
async fn the_install_is_ok_once_the_binary_reports_its_version() {
    let home = tempfile::tempdir().unwrap();
    install_binary(home.path(), "harness-dashboard");
    let env = linux(home.path());

    let (result, _, _) = run_scripted(
        &env,
        Companion::Dashboard,
        &Prints::new(DASHBOARD_VERSION),
        "done\n",
        "",
        EXITED_0,
    )
    .await;

    assert!(result.ok);
    assert_eq!(result.version.as_deref(), Some("1.26.0"));
    assert_eq!(result.summary.as_deref(), Some("Installed harness-dashboard 1.26.0."));
}

#[tokio::test]
async fn a_failing_installer_reports_its_last_error_line() {
    let home = tempfile::tempdir().unwrap();
    install_binary(home.path(), "harness-dashboard");
    let env = linux(home.path());
    let failed = ExitInfo {
        code: Some(1),
        signal: None,
    };

    let (result, _, _) = run_scripted(
        &env,
        Companion::Dashboard,
        &Prints::new(DASHBOARD_VERSION),
        "",
        "warn: x\nerror: no release found\n",
        failed,
    )
    .await;

    assert!(!result.ok, "a failed run is not ok even when an older binary answers");
    assert_eq!(result.exit_code, Some(1));
    assert_eq!(result.summary.as_deref(), Some("error: no release found"));
}

#[tokio::test]
async fn long_lines_are_cut_and_flagged() {
    let home = tempfile::tempdir().unwrap();
    let env = linux(home.path());
    let long: &'static str = Box::leak(format!("{}\nshort\n", "é".repeat(5_000)).into_boxed_str());

    let (result, lines, _) = run_scripted(
        &env,
        Companion::Dashboard,
        &Prints::new(DASHBOARD_VERSION),
        long,
        "",
        EXITED_0,
    )
    .await;

    assert!(result.truncated);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].line.len() <= MAX_LOG_LINE_BYTES);
    assert!(lines[0].line.chars().all(|c| c == 'é'));
    assert_eq!(lines[1].line, "short");
}

#[tokio::test]
async fn a_flood_of_empty_lines_is_capped() {
    let home = tempfile::tempdir().unwrap();
    let env = linux(home.path());
    let flood: &'static str = Box::leak("\n".repeat(400_000).into_boxed_str());

    let (result, lines, _) = run_scripted(
        &env,
        Companion::Dashboard,
        &Prints::new(DASHBOARD_VERSION),
        flood,
        "",
        EXITED_0,
    )
    .await;

    assert!(result.truncated);
    assert_eq!(lines.len(), MAX_LOG_LINES);
}

#[tokio::test]
async fn what_the_installer_left_running_is_killed_after_it_exited() {
    let home = tempfile::tempdir().unwrap();
    let env = linux(home.path());
    let (spawner, mut peers) = DuplexSpawner::new();

    let player = tokio::spawn(async move {
        let mut peer = peers.recv().await.unwrap();
        peer.stdout.write_all(b"started a daemon\n").await.unwrap();
        peer.stdout.shutdown().await.unwrap();
        peer.stderr.shutdown().await.unwrap();
        peer.exit_watched(EXITED_0)
    });
    let result = run_installer_with(
        &env,
        Companion::Dashboard,
        &spawner,
        &Prints::new(DASHBOARD_VERSION),
        Duration::from_secs(30),
        |_| {},
    )
    .await
    .unwrap();
    let probe = player.await.unwrap();

    assert_eq!(result.exit_code, Some(0));
    assert!(!result.timed_out);
    assert!(
        probe.is_killed(),
        "the installer's process group was killed after it exited"
    );
}

#[tokio::test]
async fn an_installer_that_runs_too_long_is_killed() {
    let home = tempfile::tempdir().unwrap();
    let env = linux(home.path());
    let (spawner, mut peers) = DuplexSpawner::new();

    let result = run_installer_with(
        &env,
        Companion::Dashboard,
        &spawner,
        &Prints::new(DASHBOARD_VERSION),
        Duration::from_millis(100),
        |_| {},
    )
    .await
    .unwrap();
    let peer = peers.recv().await.unwrap();

    assert!(result.timed_out);
    assert!(!result.ok);
    assert_eq!(result.exit_code, None);
    assert!(peer.is_killed(), "the installer's process group was killed");
}

#[tokio::test]
async fn a_spawn_failure_is_an_error() {
    let home = tempfile::tempdir().unwrap();
    let (spawner, _peers) = DuplexSpawner::new();
    spawner.fail_next_spawn("no shell");

    let outcome = run_installer(&linux(home.path()), Companion::Dashboard, &spawner, |_| {}).await;

    assert!(outcome.is_err());
}

#[test]
fn the_run_can_be_awaited_on_a_multi_threaded_runtime() {
    fn assert_send<T: Send>(_: &T) {}

    let env = linux(Path::new("/nonexistent"));
    let (spawner, _peers) = DuplexSpawner::new();
    let lines = Arc::new(Mutex::new(Vec::new()));
    let run = run_installer(&env, Companion::Dashboard, &spawner, move |line| {
        lines.lock().unwrap().push(line)
    });

    assert_send(&run);
}
