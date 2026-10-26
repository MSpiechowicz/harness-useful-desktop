//! The helper child shared by the process tests: the test binary re-executes itself and runs the
//! ignored [`helper_process`] test in a mode chosen by `HARNESS_TEST_HELPER`. No shell is involved.
#![allow(dead_code)]

use std::io::{BufRead, Write};
use std::time::{Duration, Instant};

use harness_business::process::{ExecOutput, ExecSpec};

const HELPER_ENV: &str = "HARNESS_TEST_HELPER";

/// The file the `grandchild_pidfile` helper writes its grandchild's pid to.
pub const PID_FILE_ENV: &str = "HARNESS_TEST_PID_FILE";
const FLOOD_BYTES: usize = 1024 * 1024;

/// What the `flood_then_exit` helper writes: more than any default pipe holds.
pub const DROPPED_FLOOD_BYTES: usize = 512 * 1024;

/// Not a real test: the body of the helper child. Without `HARNESS_TEST_HELPER` it does nothing.
#[test]
#[ignore = "helper child process, started by the other tests"]
fn helper_process() {
    let Ok(mode) = std::env::var(HELPER_ENV) else {
        return;
    };
    let mut stdout = std::io::stdout();

    match mode.as_str() {
        "hello" => {
            // Leading newline: libtest prints "test process_support::helper_process ... " without one.
            writeln!(stdout, "\nhello from {}", std::env::var("GREETING_NAME").unwrap()).unwrap();
            eprintln!("a note on stderr");
        }
        "sleep" => std::thread::sleep(Duration::from_secs(60)),
        "flood" => {
            stdout.write_all(&vec![b'x'; FLOOD_BYTES]).unwrap();
            stdout.flush().unwrap();

            std::io::stderr().write_all(&vec![b'y'; FLOOD_BYTES]).unwrap();
        }
        "echo" => {
            let mut line = String::new();
            std::io::stdin().lock().read_line(&mut line).unwrap();
            writeln!(stdout, "echo:{}", line.trim_end()).unwrap();
        }
        // Like a Bun program that exits right after writing: what does not fit in a pipe now is
        // dropped (stdout is non-blocking and a full pipe ends the write).
        #[cfg(unix)]
        "flood_then_exit" => {
            use nix::fcntl::{FcntlArg, OFlag, fcntl};

            fcntl(&stdout, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).unwrap();
            let mut lock = stdout.lock();
            lock.write_all(b"\n").ok();
            lock.write_all(&vec![b'z'; DROPPED_FLOOD_BYTES]).ok();
            lock.flush().ok();
            std::process::exit(0);
        }
        "exit3" => {
            eprintln!("helper failing on purpose");
            std::process::exit(3);
        }
        "grandchild" => {
            start_grandchild(true);
            std::thread::sleep(Duration::from_secs(60));
        }
        // Like `sleep 60 & sleep 60` in a shell: the grandchild's pid goes to a file, so a test can
        // find it while the run is still going.
        "grandchild_pidfile" => {
            let pid = start_grandchild(false);
            let file = std::env::var(PID_FILE_ENV).unwrap();
            std::fs::write(format!("{file}.tmp"), pid.to_string()).unwrap();
            std::fs::rename(format!("{file}.tmp"), file).unwrap();
            std::thread::sleep(Duration::from_secs(60));
        }
        // Like a daemon: detached from the pipes, left running when this helper exits.
        "orphan" => drop(start_grandchild(false)),
        // Like a command that starts a daemon which keeps its output pipes: exits successfully
        // while the grandchild still holds stdout and stderr.
        "exit_leaving_grandchild" => drop(start_grandchild(true)),
        other => panic!("unknown helper mode {other}"),
    }
}

/// Starts a grandchild in `sleep` mode, holding this helper's stdout and stderr when
/// `keeps_pipes`, and prints its pid. It is never waited for: the test kills it, or it is left
/// behind on purpose.
#[expect(clippy::zombie_processes, reason = "the grandchild is never waited for")]
fn start_grandchild(keeps_pipes: bool) -> u32 {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(helper_args()).env(HELPER_ENV, "sleep");

    if !keeps_pipes {
        command
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    }

    let child = command.spawn().unwrap();

    let mut stdout = std::io::stdout();
    writeln!(stdout, "\nGRANDCHILD={}", child.id()).unwrap();
    stdout.flush().unwrap();

    child.id()
}

fn helper_args() -> [&'static str; 5] {
    [
        "process_support::helper_process",
        "--exact",
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ]
}

/// Runs this test binary's helper child in `mode`.
pub fn helper(mode: &str) -> ExecSpec {
    ExecSpec::new(std::env::current_exe().unwrap())
        .args(helper_args())
        .env(HELPER_ENV, mode)
}

/// Waits (at most 10 s) until process `pid` is gone; panics when it survives.
pub fn assert_process_gone(pid: i32) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while process_exists(pid) {
        assert!(Instant::now() < deadline, "process {pid} survived");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// True while process `pid` exists (signal 0 reaches it).
#[cfg(unix)]
pub fn process_exists(pid: i32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    kill(Pid::from_raw(pid), None).is_ok()
}

/// True while process `pid` exists: `tasklist` lists it (CSV, the pid is the second field).
#[cfg(windows)]
pub fn process_exists(pid: i32) -> bool {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let tasklist = std::path::Path::new(&root).join("System32").join("tasklist.exe");
    let output = std::process::Command::new(tasklist)
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .expect("tasklist runs");

    let wanted = pid.to_string();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.split("\",\"").nth(1) == Some(wanted.as_str()))
}

/// The pid the `grandchild` and `orphan` helpers print.
pub fn find_grandchild_pid(output: &str) -> Option<i32> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("GRANDCHILD="))
        .map(|pid| pid.trim().parse().expect("numeric grandchild pid"))
}

/// Runner timeouts for the process-group tests, shortest first. The helper must start its
/// grandchild and print its pid before the timeout fires; a loaded machine can take longer than
/// the shortest timeout for that, so a run without the pid is retried with the next one.
pub const GROUP_KILL_TIMEOUTS: [Duration; 4] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(20),
];

/// Checks one timed-out run of the `grandchild` helper: true once the grandchild's pid arrived
/// and that process is gone, false when the run ended before the pid was printed (retry longer).
pub fn grandchild_was_killed(output: &ExecOutput) -> bool {
    // Every run ends by the timeout: the helper sleeps for 60 s.
    assert!(output.timed_out, "{output:?}");

    // The grandchild holds the stdout pipe; output only arrives because it was killed too.
    let Some(pid) = find_grandchild_pid(&output.stdout) else {
        return false;
    };

    assert_process_gone(pid);
    true
}
