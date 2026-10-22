//! Shared pieces of the "Recommended setup" tests: a probe runner that always fails, a scripted
//! installer peer, and the fixed installer commands per OS.
//!
//! Each helper is used by some tests only, hence the `dead_code` allowance.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use harness_business::Result;
use harness_business::ecosystem::{Companion, InstallerSpec, installer};
use harness_business::env::{Env, OsKind};
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ExitInfo, ProcessRunner};
use harness_business::testing::FakePeer;
use pretty_assertions::assert_eq;
use tokio::io::AsyncWriteExt;

pub const EXITED_0: ExitInfo = ExitInfo {
    code: Some(0),
    signal: None,
};

/// Counts the probes it is asked to run; every one fails (no version).
#[derive(Default)]
pub struct CountingRunner(Mutex<usize>);

impl CountingRunner {
    pub fn probes(&self) -> usize {
        *self.0.lock().unwrap()
    }
}

impl ProcessRunner for CountingRunner {
    fn run<'a>(&'a self, _spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        *self.0.lock().unwrap() += 1;

        Box::pin(async {
            Ok(ExecOutput {
                status: Some(1),
                ..ExecOutput::default()
            })
        })
    }
}

/// Plays one installer run: writes `stdout` and `stderr`, closes them, exits with `exit`.
pub async fn play(mut peer: FakePeer, stdout: &str, stderr: &str, exit: ExitInfo) -> ExecSpec {
    peer.stdout.write_all(stdout.as_bytes()).await.unwrap();
    peer.stderr.write_all(stderr.as_bytes()).await.unwrap();
    peer.stdout.shutdown().await.unwrap();
    peer.stderr.shutdown().await.unwrap();

    let spec = peer.spec.clone();
    peer.exit(exit);
    spec
}

/// The installer of `companion` on Linux and macOS: the system shell runs `script`, which starts
/// with the HTTPS download `download`.
pub fn assert_unix_installer(home: &Path, companion: Companion, script: &str, download: &str) {
    for os in [OsKind::Linux, OsKind::MacOs] {
        let spec = installer(&Env::for_tests(home).with_os(os), companion);

        assert_eq!(spec.program, PathBuf::from("/bin/sh")); // guard-allow: installer-shell
        assert_eq!(spec.args, vec!["-c".to_owned(), script.to_owned()]);
        assert_eq!(spec.command, script);
        assert!(
            spec.args[1].starts_with(download),
            "the download is held to HTTPS: {}",
            spec.args[1]
        );
    }
}

/// Windows PowerShell below the system `root`.
pub fn powershell(root: &str) -> PathBuf {
    PathBuf::from(root)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe") // guard-allow: installer-shell
}

/// `spec` runs `script` in Windows PowerShell below `C:\Windows`, shown as `command`.
pub fn assert_windows_installer(spec: &InstallerSpec, script: &str, command: &str) {
    assert_eq!(spec.program, powershell(r"C:\Windows"));
    assert_eq!(
        spec.args,
        ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script]
    );
    assert_eq!(spec.command, command);
}

/// An executable `harness-dashboard` (never run) in `<home>/.local/bin`, joined part by part and
/// with `.exe` on Windows, as the PATH search names the program it finds, and `env` with that
/// folder as its `PATH`.
pub fn dashboard_on_path(home: &Path, env: &Env) -> (PathBuf, Env) {
    let dashboard = home
        .join(".local")
        .join("bin")
        .join(format!("harness-dashboard{}", std::env::consts::EXE_SUFFIX));
    std::fs::create_dir_all(dashboard.parent().unwrap()).unwrap();
    std::fs::write(&dashboard, "never run").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dashboard, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let path = dashboard.parent().unwrap().to_string_lossy().into_owned();

    (dashboard.clone(), env.clone().with_var("PATH", path))
}
