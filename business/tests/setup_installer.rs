//! Running the dashboard installer: the fixed command per OS, streamed and cleaned output, success
//! only when the binary answers afterwards, and the time limit.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use harness_business::Result;
use harness_business::ecosystem::catalog::{DASHBOARD_INSTALL_PS1, DASHBOARD_INSTALL_SH};
use harness_business::ecosystem::installer::{MAX_LOG_LINE_BYTES, MAX_LOG_LINES, dashboard_installer};
use harness_business::ecosystem::{run_dashboard_installer, run_dashboard_installer_with};
use harness_business::env::{Env, OsKind};
use harness_business::model::{DashboardInstallResult, InstallLogLine, InstallStream};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ExitInfo, ProcessRunner};
use harness_business::testing::{DuplexSpawner, FakePeer};
use pretty_assertions::assert_eq;
use tokio::io::AsyncWriteExt;

const EXITED_0: ExitInfo = ExitInfo {
    code: Some(0),
    signal: None,
};

/// Answers every probe with `harness-dashboard version` output.
struct PrintsVersion;

impl ProcessRunner for PrintsVersion {
    fn run<'a>(&'a self, _spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        Box::pin(async {
            Ok(ExecOutput {
                status: Some(0),
                stdout: "1.26.0\n".to_owned(),
                ..ExecOutput::default()
            })
        })
    }
}

fn linux(home: &Path) -> Env {
    Env::for_tests(home).with_os(OsKind::Linux)
}

fn install_binary(home: &Path) {
    let binary = home.join(".local/bin/harness-dashboard");
    std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
    std::fs::write(&binary, "").unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// Plays one installer run: writes `stdout` and `stderr`, closes them, exits with `exit`.
async fn play(mut peer: FakePeer, stdout: &str, stderr: &str, exit: ExitInfo) -> ExecSpec {
    peer.stdout.write_all(stdout.as_bytes()).await.unwrap();
    peer.stderr.write_all(stderr.as_bytes()).await.unwrap();
    peer.stdout.shutdown().await.unwrap();
    peer.stderr.shutdown().await.unwrap();

    let spec = peer.spec.clone();
    peer.exit(exit);
    spec
}

/// Runs the installer against a scripted peer and returns the result, the lines and the spec.
async fn run_scripted(
    env: &Env,
    runner: &dyn ProcessRunner,
    stdout: &'static str,
    stderr: &'static str,
    exit: ExitInfo,
) -> (DashboardInstallResult, Vec<InstallLogLine>, ExecSpec) {
    let (spawner, mut peers) = DuplexSpawner::new();
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink_lines = Arc::clone(&lines);

    let player = tokio::spawn(async move { play(peers.recv().await.unwrap(), stdout, stderr, exit).await });
    let result = run_dashboard_installer_with(env, &spawner, runner, Duration::from_secs(30), move |line| {
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

    for os in [OsKind::Linux, OsKind::MacOs] {
        let spec = dashboard_installer(&Env::for_tests(home.path()).with_os(os));

        assert_eq!(spec.program, PathBuf::from("/bin/sh")); // guard-allow: installer-shell
        assert_eq!(spec.args, vec!["-c".to_owned(), DASHBOARD_INSTALL_SH.to_owned()]);
        assert_eq!(spec.command, DASHBOARD_INSTALL_SH);
        assert!(
            spec.args[1].starts_with("curl --proto '=https' --tlsv1.2 -fsSL https://"),
            "the download is held to HTTPS: {}",
            spec.args[1]
        );
    }

    let windows = Env::for_tests(home.path()).with_os(OsKind::Windows);
    let powershell = |root: &str| {
        PathBuf::from(root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe") // guard-allow: installer-shell
    };
    let script = "irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-dashboard/main/install.ps1 | iex";

    let default_root = dashboard_installer(&windows);
    assert_eq!(default_root.program, powershell(r"C:\Windows"));
    assert_eq!(
        default_root.args,
        ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script]
    );
    assert_eq!(default_root.command, DASHBOARD_INSTALL_PS1);

    let custom_root = dashboard_installer(&windows.with_var("SystemRoot", r"D:\Win"));
    assert_eq!(custom_root.program, powershell(r"D:\Win"));
}

#[tokio::test]
async fn an_exit_code_of_zero_without_the_binary_is_not_ok() {
    let home = tempfile::tempdir().unwrap();
    let env = linux(home.path());
    let stdout = "\u{1b}[36m==>\u{1b}[0m Downloading\r\nAuthorization: Bearer CANARY0123456789abcdefXYZ\n";
    let stderr = "curl: (6) Could not resolve host: raw.githubusercontent.com";

    let (result, lines, spec) = run_scripted(&env, &PrintsVersion, stdout, stderr, EXITED_0).await;

    assert_eq!(spec.program, dashboard_installer(&env).program);
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
    install_binary(home.path());
    let env = linux(home.path());

    let (result, _, _) = run_scripted(&env, &PrintsVersion, "done\n", "", EXITED_0).await;

    assert!(result.ok);
    assert_eq!(result.version.as_deref(), Some("1.26.0"));
    assert_eq!(result.summary.as_deref(), Some("Installed harness-dashboard 1.26.0."));
}

#[tokio::test]
async fn a_failing_installer_reports_its_last_error_line() {
    let home = tempfile::tempdir().unwrap();
    install_binary(home.path());
    let env = linux(home.path());
    let failed = ExitInfo {
        code: Some(1),
        signal: None,
    };

    let (result, _, _) = run_scripted(&env, &PrintsVersion, "", "warn: x\nerror: no release found\n", failed).await;

    assert!(!result.ok, "a failed run is not ok even when an older binary answers");
    assert_eq!(result.exit_code, Some(1));
    assert_eq!(result.summary.as_deref(), Some("error: no release found"));
}

#[tokio::test]
async fn long_lines_are_cut_and_flagged() {
    let home = tempfile::tempdir().unwrap();
    let env = linux(home.path());
    let long: &'static str = Box::leak(format!("{}\nshort\n", "é".repeat(5_000)).into_boxed_str());

    let (result, lines, _) = run_scripted(&env, &PrintsVersion, long, "", EXITED_0).await;

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

    let (result, lines, _) = run_scripted(&env, &PrintsVersion, flood, "", EXITED_0).await;

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
    let result = run_dashboard_installer_with(&env, &spawner, &PrintsVersion, Duration::from_secs(30), |_| {})
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

    let result = run_dashboard_installer_with(&env, &spawner, &PrintsVersion, Duration::from_millis(100), |_| {})
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

    let outcome = run_dashboard_installer(&linux(home.path()), &spawner, |_| {}).await;

    assert!(outcome.is_err());
}

#[test]
fn the_run_can_be_awaited_on_a_multi_threaded_runtime() {
    fn assert_send<T: Send>(_: &T) {}

    let env = linux(Path::new("/nonexistent"));
    let (spawner, _peers) = DuplexSpawner::new();
    let lines = Arc::new(Mutex::new(Vec::new()));
    let run = run_dashboard_installer(&env, &spawner, move |line| lines.lock().unwrap().push(line));

    assert_send(&run);
}
