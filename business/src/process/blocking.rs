//! Run-to-completion execution for synchronous callers.
//!
//! Some harness reads happen inside blocking tasks (session listing and transcripts of a CLI that
//! only exposes them as commands), where an async [`super::ProcessRunner`] cannot be awaited.
//! [`BlockingRunner`] is the same contract without a future.

use std::io::Read;
use std::process::{Child, ChildStderr, ChildStdout, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use super::{ExecOutput, ExecSpec, build_std_command, spawn_error};
use crate::error::{BusinessError, Result};

/// How often a running child is checked for exit.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// After the child ended, how long to wait for the output pipes to close before giving up on them
/// (and again after killing the descendants that still hold them).
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// Read buffer size for capturing output.
const CHUNK: usize = 16 * 1024;

/// Runs a command to completion on the calling thread. Implementations never use a shell.
///
/// The contract matches [`super::ProcessRunner::run`]: `spec.timeout` bounds the run and kills the
/// child (and its process group on Unix) when exceeded, each output stream is capped at
/// `spec.max_output_bytes`, stdin is null, and a timeout is reported via
/// [`ExecOutput::timed_out`], not as an error. Errors mean the process could not be started.
pub trait BlockingRunner: Send + Sync {
    /// Runs `spec` and captures its output.
    fn run(&self, spec: &ExecSpec) -> Result<ExecOutput>;
}

/// [`BlockingRunner`] backed by `std::process`.
///
/// Each output stream is read by its own thread, capped (excess is drained and discarded so the
/// child never blocks on a full pipe); the calling thread polls for exit and kills the child and
/// its process group once the timeout passes. A child that exits normally while a descendant it
/// started still holds the output pipes is a normal exit: the descendants get [`DRAIN_GRACE`] to
/// close them, then the group is killed and the output captured so far is returned.
///
/// The reader threads are deliberately not joined: a descendant that left the group (`setsid`)
/// can hold the pipes past the run, and its readers live on until it exits, keeping at most the
/// capped capture in memory.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdRunner;

impl BlockingRunner for StdRunner {
    fn run(&self, spec: &ExecSpec) -> Result<ExecOutput> {
        let mut command = build_std_command(spec, Stdio::null())?;
        let mut child = command.spawn().map_err(|error| spawn_error(&error, spec))?;

        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            kill_tree(&mut child);
            return Err(BusinessError::Internal("child pipes missing".to_owned()));
        };

        let pid = child.id();
        let mut readers = Readers::start(stdout, stderr, spec.max_output_bytes);
        let deadline = Instant::now() + spec.timeout;

        let (status, timed_out) = wait_until(&mut child, deadline)
            .map_err(|error| BusinessError::Process(format!("waiting for {}: {error}", spec.label())))?;

        // The pipes close once every process holding them has ended. After a timeout the group is
        // dead already; after a normal exit a descendant (a daemon the command started) may still
        // hold them, so it gets a grace period and then its group is killed too.
        if !readers.wait_closed(Instant::now() + DRAIN_GRACE) && !timed_out {
            kill_survivors(pid);
            readers.wait_closed(Instant::now() + DRAIN_GRACE);
        }

        // A descendant that left the group can keep the pipes open: what arrived so far is the
        // output, and the reader threads (not joined) end with that process.
        let (out, err) = readers.take();

        Ok(ExecOutput {
            status,
            stdout: String::from_utf8_lossy(&out.bytes).into_owned(),
            stderr: String::from_utf8_lossy(&err.bytes).into_owned(),
            truncated: out.truncated || err.truncated,
            timed_out,
        })
    }
}

/// Polls `child` until it exits or `deadline` passes; then it is killed (with its group) and
/// reaped. Returns the exit code and whether the timeout ended it.
fn wait_until(child: &mut Child, deadline: Instant) -> std::io::Result<(Option<i32>, bool)> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((status.code(), false));
        }

        if Instant::now() >= deadline {
            kill_tree(child);
            child.wait()?;

            return Ok((None, true));
        }

        thread::sleep(POLL_INTERVAL);
    }
}

/// Kills the child and, on Unix, its whole process group. Only called before the child was reaped,
/// so its pid (and group id) cannot have been reused yet.
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    super::kill_group(child.id());

    // Fails only when the child already exited, which is the outcome wanted.
    child.kill().ok();
}

/// Kills what is left of the process group of the exited (and reaped) child `pid`. The group id
/// stays reserved while a member lives, so the group is signalled only when a probe finds a member:
/// an empty group's id may have been reused by an unrelated group.
fn kill_survivors(pid: u32) {
    #[cfg(unix)]
    if group_has_members(pid) {
        super::kill_group(pid);
    }

    #[cfg(not(unix))]
    let _ = pid;
}

/// True when the process group `pgid` still has a member (signal 0 reaches one).
#[cfg(unix)]
fn group_has_members(pgid: u32) -> bool {
    use nix::sys::signal::killpg;
    use nix::unistd::Pid;

    let Ok(raw) = i32::try_from(pgid) else {
        return false;
    };

    killpg(Pid::from_raw(raw), None).is_ok()
}

/// Bytes captured from one stream.
#[derive(Default)]
struct Captured {
    bytes: Vec<u8>,
    truncated: bool,
}

/// The reader threads of both streams. Each appends to its shared capture as data arrives, so
/// the output is there even when a stream never closes, and reports when its stream closed.
struct Readers {
    out: Arc<Mutex<Captured>>,
    err: Arc<Mutex<Captured>>,
    closed: Receiver<()>,
    open: usize,
}

impl Readers {
    /// Starts one reader thread per stream, each keeping at most `cap` bytes.
    fn start(stdout: ChildStdout, stderr: ChildStderr, cap: usize) -> Self {
        let (sender, closed) = channel();
        let out = Arc::new(Mutex::new(Captured::default()));
        let err = Arc::new(Mutex::new(Captured::default()));

        let (out_capture, out_closed) = (Arc::clone(&out), sender.clone());
        thread::spawn(move || {
            read_capped(stdout, cap, &out_capture);
            out_closed.send(()).ok();
        });

        let err_capture = Arc::clone(&err);
        thread::spawn(move || {
            read_capped(stderr, cap, &err_capture);
            sender.send(()).ok();
        });

        Self {
            out,
            err,
            closed,
            open: 2,
        }
    }

    /// Waits until both streams closed or `deadline` passes; true when they closed.
    fn wait_closed(&mut self, deadline: Instant) -> bool {
        while self.open > 0 {
            let left = deadline.saturating_duration_since(Instant::now());

            if self.closed.recv_timeout(left).is_err() {
                return false;
            }

            self.open -= 1;
        }

        true
    }

    /// The captures of stdout and stderr so far.
    fn take(&self) -> (Captured, Captured) {
        let take =
            |capture: &Mutex<Captured>| std::mem::take(&mut *capture.lock().unwrap_or_else(PoisonError::into_inner));

        (take(&self.out), take(&self.err))
    }
}

/// Reads `reader` to EOF into `captured`, keeping at most `cap` bytes and discarding the rest.
fn read_capped(mut reader: impl Read, cap: usize, captured: &Mutex<Captured>) {
    let mut chunk = vec![0_u8; CHUNK];

    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };

        let mut captured = captured.lock().unwrap_or_else(PoisonError::into_inner);
        let room = cap.saturating_sub(captured.bytes.len());
        let keep = read.min(room);
        captured.bytes.extend_from_slice(&chunk[..keep]);

        if keep < read {
            captured.truncated = true;
        }
    }
}
