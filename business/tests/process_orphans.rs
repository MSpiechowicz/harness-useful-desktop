//! Children left running by a crashed app: [`TrackedSpawner`] records them, [`sweep_orphans`]
//! kills the ones still provably the same process once their app is gone. Every process here is
//! a `sleep` this test starts; the sweep reads only the test's own temporary folder.
#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use harness_business::process::{
    ChildRegistry, ExecSpec, ProcessIdentity, ProcessSpawner, TokioSpawner, TrackedSpawner, sweep_orphans,
};
use serde_json::Value;

/// How long a killed process gets to be reported dead.
const DEATH_LIMIT: Duration = Duration::from_secs(10);

fn sleep_binary() -> PathBuf {
    which::which("sleep").expect("sleep on PATH")
}

/// A `sleep 300` leading its own process group, like the app's children.
fn start_sleep() -> Child {
    let mut command = Command::new(sleep_binary());
    command
        .arg("300")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    std::os::unix::process::CommandExt::process_group(&mut command, 0);

    command.spawn().expect("sleep starts")
}

fn sleep_args() -> Vec<OsString> {
    vec![OsString::from("300")]
}

/// A stand-in for the app that recorded the children: a `sleep` the test can let die.
struct FakeApp {
    process: Child,
    identity: ProcessIdentity,
}

impl FakeApp {
    fn start() -> Self {
        let process = start_sleep();
        let identity = ProcessIdentity::capture(process.id(), &sleep_args()).expect("app identity");

        Self { process, identity }
    }

    /// The app crashes: it dies without removing anything it recorded.
    fn crash(mut self) {
        self.process.kill().ok();
        self.process.wait().ok();
    }
}

/// Waits for `child` to exit and returns its terminating signal; `None` when it still runs after
/// [`DEATH_LIMIT`].
fn wait_for_signal(child: &mut Child) -> Option<i32> {
    let deadline = Instant::now() + DEATH_LIMIT;

    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return std::os::unix::process::ExitStatusExt::signal(&status);
        }

        std::thread::sleep(Duration::from_millis(20));
    }

    None
}

fn still_running(child: &mut Child) -> bool {
    std::thread::sleep(Duration::from_millis(100));
    child.try_wait().expect("try_wait").is_none()
}

fn stop(mut child: Child) {
    child.kill().ok();
    child.wait().ok();
}

/// Rewrites the first recorded child in `file` with `edit`.
fn tamper(file: &Path, edit: impl FnOnce(&mut Value)) {
    let mut record: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
    edit(&mut record["children"][0]);
    fs::write(file, serde_json::to_vec(&record).unwrap()).unwrap();
}

#[test]
fn a_crashed_apps_child_is_killed() {
    let dir = tempfile::tempdir().unwrap();
    let app = FakeApp::start();
    let registry = ChildRegistry::with_owner(dir.path(), app.identity.clone());
    let mut child = start_sleep();

    let recorded = registry.record(child.id(), &sleep_args()).expect("recorded");
    assert_eq!(recorded.pid, child.id());

    let mode = std::os::unix::fs::PermissionsExt::mode(&fs::metadata(registry.file()).unwrap().permissions());
    assert_eq!(mode & 0o777, 0o600, "the record is private");

    let file = registry.file().to_path_buf();
    drop(registry);
    app.crash();

    let report = sweep_orphans(dir.path());

    assert_eq!(report.killed, vec![child.id()]);
    assert_eq!(wait_for_signal(&mut child), Some(9), "the orphan was killed");
    assert!(!file.exists(), "the swept record is removed");
}

#[test]
fn the_sweep_reaches_what_the_orphan_started() {
    let dir = tempfile::tempdir().unwrap();
    let app = FakeApp::start();
    let registry = ChildRegistry::with_owner(dir.path(), app.identity.clone());

    // A shell leading its own group, with a `sleep` it started in the background.
    let script = format!("{} 300 & echo $!; wait", sleep_binary().display());
    let args = [OsString::from("-c"), OsString::from(&script)];
    let mut command = Command::new("/bin/sh"); // guard-allow: installer-shell
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let mut shell = command.spawn().unwrap();

    let mut line = String::new();
    BufReader::new(shell.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let grandchild: i32 = line.trim().parse().unwrap();

    registry.record(shell.id(), &args).expect("recorded");
    drop(registry);
    app.crash();

    let report = sweep_orphans(dir.path());

    assert_eq!(report.killed, vec![shell.id()]);
    assert_eq!(wait_for_signal(&mut shell), Some(9));
    assert!(is_gone(grandchild), "the background sleep died with its group");
}

/// True once `pid` no longer exists (an orphaned grandchild is reaped by init), polled for at most
/// [`DEATH_LIMIT`].
fn is_gone(pid: i32) -> bool {
    let deadline = Instant::now() + DEATH_LIMIT;

    while Instant::now() < deadline {
        if nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), None) == Err(nix::errno::Errno::ESRCH) {
            return true;
        }

        std::thread::sleep(Duration::from_millis(20));
    }

    false
}

#[test]
fn a_child_of_an_app_that_still_runs_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let app = FakeApp::start();
    let registry = ChildRegistry::with_owner(dir.path(), app.identity.clone());
    let mut child = start_sleep();
    registry.record(child.id(), &sleep_args()).expect("recorded");

    let report = sweep_orphans(dir.path());

    assert_eq!(report.live_apps, 1);
    assert!(report.killed.is_empty());
    assert!(still_running(&mut child));
    assert!(registry.file().exists(), "a live app's record is kept");

    stop(child);
    app.crash();
}

#[test]
fn a_pid_whose_identity_changed_is_never_killed() {
    let dir = tempfile::tempdir().unwrap();

    let edits: [fn(&mut Value); 2] = [
        // The pid now names a process started at another time (pid reuse).
        |child| child["started"] = Value::from("0:1"),
        // The pid now names a process with another command line.
        |child| child["args"] = serde_json::json!(["serve", "--port", "0"]),
    ];

    for edit in edits {
        let app = FakeApp::start();
        let registry = ChildRegistry::with_owner(dir.path(), app.identity.clone());
        let mut child = start_sleep();
        registry.record(child.id(), &sleep_args()).expect("recorded");

        tamper(registry.file(), edit);
        let file = registry.file().to_path_buf();
        drop(registry);
        app.crash();

        let report = sweep_orphans(dir.path());

        assert!(report.killed.is_empty(), "{report:?}");
        assert_eq!(report.gone, 1);
        assert!(
            still_running(&mut child),
            "a process that is not provably ours survives"
        );
        assert!(!file.exists());

        stop(child);
    }
}

#[test]
fn a_child_that_exited_is_not_killed_again() {
    let dir = tempfile::tempdir().unwrap();
    let app = FakeApp::start();
    let registry = ChildRegistry::with_owner(dir.path(), app.identity.clone());
    let child = start_sleep();
    registry.record(child.id(), &sleep_args()).expect("recorded");

    drop(registry);
    stop(child);
    app.crash();

    let report = sweep_orphans(dir.path());

    assert!(report.killed.is_empty());
    assert_eq!(report.gone, 1);
}

#[tokio::test]
async fn a_reaped_or_dropped_child_leaves_the_record() {
    let dir = tempfile::tempdir().unwrap();
    let app = FakeApp::start();
    let registry = ChildRegistry::with_owner(dir.path(), app.identity.clone());
    let spawner = TrackedSpawner::new(Arc::new(TokioSpawner), registry.clone());

    // Reaped normally.
    let spec = ExecSpec::new(sleep_binary()).args(["0.2"]);
    let mut short = spawner.spawn(&spec).unwrap();
    let pid = short.child.id().unwrap();
    let recorded = fs::read_to_string(registry.file()).unwrap();
    assert!(recorded.contains(&format!("\"pid\":{pid}")), "{recorded}");

    let exit = short.child.wait().await.unwrap();
    assert_eq!(exit.code, Some(0));
    assert!(!registry.file().exists(), "the reaped child's entry is gone");

    // Dropped (killed by its handle).
    let long = spawner.spawn(&ExecSpec::new(sleep_binary()).args(["300"])).unwrap();
    assert!(registry.file().exists());
    drop(long);
    assert!(!registry.file().exists(), "the dropped child's entry is gone");

    app.crash();
    assert_eq!(sweep_orphans(dir.path()).killed, Vec::<u32>::new());
}

#[test]
fn corrupt_oversized_and_foreign_files_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("app-1-2.json"), b"{not json").unwrap();
    fs::write(dir.path().join("app-3-4.json"), vec![b' '; 512 * 1024]).unwrap();
    fs::write(dir.path().join("notes.txt"), b"kept").unwrap();

    let report = sweep_orphans(dir.path());

    assert_eq!(report.corrupt, 2);
    assert!(report.killed.is_empty());
    assert!(!dir.path().join("app-1-2.json").exists());
    assert!(dir.path().join("notes.txt").exists(), "files that are not records stay");

    assert_eq!(sweep_orphans(&dir.path().join("missing")), Default::default());
}
