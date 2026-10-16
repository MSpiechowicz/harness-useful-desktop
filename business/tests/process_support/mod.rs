//! The helper child shared by the process tests: each test binary re-executes itself and runs its
//! `helper_process` test (which calls [`helper_process`]) in a mode chosen by `HARNESS_TEST_HELPER`.
//! No shell is involved.
#![allow(dead_code)]

use std::io::{BufRead, Write};
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

#[cfg(unix)]
use harness_business::process::ExecOutput;
use harness_business::process::ExecSpec;

const HELPER_ENV: &str = "HARNESS_TEST_HELPER";
const FLOOD_BYTES: usize = 1024 * 1024;

/// The body of the helper child. Without `HARNESS_TEST_HELPER` it does nothing.
pub fn helper_process() {
    let Ok(mode) = std::env::var(HELPER_ENV) else {
        return;
    };
    let mut stdout = std::io::stdout();

    match mode.as_str() {
        "hello" => {
            // Leading newline: libtest prints "test helper_process ... " without one.
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
        "exit3" => {
            eprintln!("helper failing on purpose");
            std::process::exit(3);
        }
        "grandchild" => {
            start_grandchild(true);
            std::thread::sleep(Duration::from_secs(60));
        }
        // Like a daemon: detached from the pipes, left running when this helper exits.
        "orphan" => start_grandchild(false),
        // Like a command that starts a daemon which keeps its output pipes: exits successfully
        // while the grandchild still holds stdout and stderr.
        "exit_leaving_grandchild" => start_grandchild(true),
        other => panic!("unknown helper mode {other}"),
    }
}

/// Starts a grandchild in `sleep` mode, holding this helper's stdout and stderr when
/// `keeps_pipes`, and prints its pid. It is never waited for: the test kills it, or it is left
/// behind on purpose.
#[expect(clippy::zombie_processes, reason = "the grandchild is never waited for")]
fn start_grandchild(keeps_pipes: bool) {
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
}

fn helper_args() -> [&'static str; 5] {
    [
        "helper_process",
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

#[cfg(unix)]
pub fn assert_process_gone(pid: i32) {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    let deadline = Instant::now() + Duration::from_secs(10);
    while kill(Pid::from_raw(pid), None).is_ok() {
        assert!(Instant::now() < deadline, "process {pid} survived");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The pid the `grandchild` and `orphan` helpers print.
#[cfg(unix)]
pub fn find_grandchild_pid(output: &str) -> Option<i32> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("GRANDCHILD="))
        .map(|pid| pid.trim().parse().expect("numeric grandchild pid"))
}

/// Runner timeouts for the process-group tests, shortest first. The helper must start its
/// grandchild and print its pid before the timeout fires; a loaded machine can take longer than
/// the shortest timeout for that, so a run without the pid is retried with the next one.
#[cfg(unix)]
pub const GROUP_KILL_TIMEOUTS: [Duration; 4] = [
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(20),
];

/// Checks one timed-out run of the `grandchild` helper: true once the grandchild's pid arrived
/// and that process is gone, false when the run ended before the pid was printed (retry longer).
#[cfg(unix)]
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
