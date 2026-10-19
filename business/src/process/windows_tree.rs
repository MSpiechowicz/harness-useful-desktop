//! Windows: killing a child together with every process it started.
//!
//! Windows has no process group to signal, and a job object needs `unsafe` code, which the
//! workspace forbids. `taskkill /T /F` walks the tree by parent process id instead, so it must run
//! while the child is alive or at least not reaped: the caller's open handle then keeps its pid
//! from being reused, and the processes naming that pid as their parent are its descendants. What
//! the child started and left behind after it exited is not reached.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// `CREATE_NO_WINDOW`: taskkill is a console program and must not open a console window.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// How long taskkill gets before it is killed and the caller kills the direct child alone.
const TASKKILL_LIMIT: Duration = Duration::from_secs(5);

/// How often taskkill is checked for exit.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Kills process `pid` and its descendants with `taskkill /T /F /PID <pid>` (no shell, absolute
/// path, no window) and waits for it, at most [`TASKKILL_LIMIT`]. `pid` must name a child the
/// caller holds unreaped. A tree that is gone already is the outcome wanted, so taskkill's exit
/// code is not checked; the caller still kills the direct child itself.
pub(super) fn kill_process_tree(pid: u32) {
    let mut command = Command::new(taskkill());
    command
        .args(["/T", "/F", "/PID"])
        .arg(pid.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    std::os::windows::process::CommandExt::creation_flags(&mut command, CREATE_NO_WINDOW);

    let mut taskkill = match command.spawn() {
        Ok(taskkill) => taskkill,
        Err(error) => {
            tracing::debug!(%error, "taskkill did not start");
            return;
        }
    };

    let deadline = Instant::now() + TASKKILL_LIMIT;

    loop {
        match taskkill.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            Ok(None) | Err(_) => {
                tracing::debug!("taskkill did not finish, the direct child is killed alone");
                taskkill.kill().ok();
                taskkill.wait().ok();
                return;
            }
        }
    }
}

/// `<SystemRoot>\System32\taskkill.exe`; `SystemRoot` defaults to `C:\Windows` when it is unset
/// or not absolute.
fn taskkill() -> PathBuf {
    let root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .filter(|root| root.is_absolute())
        .unwrap_or_else(|| Path::new(r"C:\Windows").to_path_buf());

    root.join("System32").join("taskkill.exe")
}
