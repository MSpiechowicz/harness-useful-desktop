//! Run-to-completion execution with timeout and output cap.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::time::{Instant, timeout_at};

#[cfg(unix)]
use super::kill_group;
use super::{BoxFuture, ExecSpec, build_command, kill_tree, spawn_command};
use crate::error::{BusinessError, Result};

/// After a timeout kill, how long to wait for the output pipes to close before giving up on them.
const DRAIN_GRACE: Duration = Duration::from_secs(2);

/// Read buffer size for capturing output.
const CHUNK: usize = 16 * 1024;

/// Captured result of a finished (or killed) process.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecOutput {
    /// Exit code; `None` when killed by a signal or on timeout.
    pub status: Option<i32>,
    /// Captured stdout (lossy UTF-8), at most `max_output_bytes` bytes before conversion.
    pub stdout: String,
    /// Captured stderr (lossy UTF-8), at most `max_output_bytes` bytes before conversion.
    pub stderr: String,
    /// True when either stream produced more than the cap.
    pub truncated: bool,
    /// True when the process was killed because it exceeded the timeout.
    pub timed_out: bool,
}

impl ExecOutput {
    /// True for a normal exit with code 0.
    pub fn success(&self) -> bool {
        self.status == Some(0) && !self.timed_out
    }
}

/// Runs a command to completion. Implementations never use a shell.
pub trait ProcessRunner: Send + Sync {
    /// Runs `spec` and captures its output. A timeout is reported via [`ExecOutput::timed_out`],
    /// not as an error; errors mean the process could not be started.
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>>;
}

/// [`ProcessRunner`] backed by `tokio::process`.
///
/// stdin is null unless the spec gives bytes for it (written, then closed), each output stream is capped (excess is drained and discarded so the child
/// never blocks on a full pipe), and a timeout kills the child and its process group. Dropping the
/// future before it finished (a cancelled run) kills the process group too on Unix, so nothing the
/// child started in its group outlives it; on Windows only the direct child is killed.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioRunner;

impl ProcessRunner for TokioRunner {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        Box::pin(run_to_completion(spec))
    }
}

async fn run_to_completion(spec: &ExecSpec) -> Result<ExecOutput> {
    let stdin = match spec.stdin {
        Some(_) => Stdio::piped(),
        None => Stdio::null(),
    };
    let command = build_command(spec, stdin)?;
    let mut child = spawn_command(command, spec)?;

    // Until the run finished, dropping this future kills the whole group (`kill_on_drop` alone
    // reaches only the direct child).
    let group = GroupGuard { group: child.id() };

    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(BusinessError::Internal("child pipes missing".to_owned()));
    };

    // Written while the output is read, so a child that answers before it read everything never
    // blocks on a full pipe; a child that exits early just ends the write.
    let input = child.stdin.take();
    let feed = async {
        if let (Some(mut pipe), Some(bytes)) = (input, spec.stdin.as_deref()) {
            pipe.write_all(bytes).await.ok();
            pipe.shutdown().await.ok();
        }
    };

    let deadline = Instant::now() + spec.timeout;
    let capture = Capture {
        cap: spec.max_output_bytes,
        tail: spec.keep_tail,
    };

    let waiter = async {
        match timeout_at(deadline, child.wait()).await {
            Ok(status) => status.map(|status| (status.code(), false)),
            Err(_elapsed) => {
                kill_tree(&mut child).ok();
                child.wait().await.map(|_| (None, true))
            }
        }
    };

    let all = async { tokio::join!(capture.read(stdout), capture.read(stderr), waiter, feed) };

    // The pipes normally close when the process group dies; a descendant that escaped the group
    // could keep them open, so collecting is bounded too.
    let collected = timeout_at(deadline + DRAIN_GRACE, all).await;
    group.disarm();

    let Ok((out, err, waited, ())) = collected else {
        return Ok(ExecOutput {
            timed_out: true,
            ..ExecOutput::default()
        });
    };

    let (status, timed_out) =
        waited.map_err(|error| BusinessError::Process(format!("waiting for {}: {error}", spec.label())))?;

    Ok(ExecOutput {
        status,
        stdout: String::from_utf8_lossy(&out.bytes).into_owned(),
        stderr: String::from_utf8_lossy(&err.bytes).into_owned(),
        truncated: out.truncated || err.truncated,
        timed_out,
    })
}

/// Kills the process group of a child when dropped before [`GroupGuard::disarm`]: the future of
/// a run was dropped (cancelled) while its child, or something it started, may still run.
struct GroupGuard {
    /// The child's pid, which is its process group id (it leads its own group).
    group: Option<u32>,
}

impl GroupGuard {
    /// The run finished: what it left behind is the caller's business, as before.
    fn disarm(mut self) {
        self.group = None;
    }
}

impl Drop for GroupGuard {
    fn drop(&mut self) {
        // The group id stays reserved while any member lives, so this reaches only the child's
        // group (see `ChildHandle::kill_process_group`). Windows: `kill_on_drop` kills the child.
        #[cfg(unix)]
        if let Some(group) = self.group.take() {
            kill_group(group);
        }
    }
}

/// How one stream is captured: at most `cap` bytes, the first ones or (`tail`) the last ones.
#[derive(Debug, Clone, Copy)]
struct Capture {
    cap: usize,
    tail: bool,
}

/// Bytes captured from one stream.
struct Captured {
    bytes: Vec<u8>,
    truncated: bool,
}

impl Capture {
    /// Reads `reader` to EOF, keeping at most `cap` bytes and discarding the rest.
    async fn read(self, mut reader: impl AsyncRead + Unpin) -> Captured {
        let mut captured = Captured {
            bytes: Vec::new(),
            truncated: false,
        };
        let mut chunk = vec![0_u8; CHUNK];

        loop {
            let read = match reader.read(&mut chunk).await {
                Ok(0) | Err(_) => break,
                Ok(read) => read,
            };

            if self.tail {
                captured.bytes.extend_from_slice(&chunk[..read]);
                let excess = captured.bytes.len().saturating_sub(self.cap);

                if excess > 0 {
                    captured.bytes.drain(..excess);
                    captured.truncated = true;
                }

                continue;
            }

            let room = self.cap.saturating_sub(captured.bytes.len());
            let keep = read.min(room);
            captured.bytes.extend_from_slice(&chunk[..keep]);

            if keep < read {
                captured.truncated = true;
            }
        }

        if self.tail && captured.truncated {
            drop_cut_word(&mut captured.bytes);
        }

        captured
    }
}

/// Drops the first word of a tail that was cut off at its start: the bytes up to and including
/// the first ASCII whitespace, or everything when there is none. What is cut off could be the end
/// of a secret, which redaction would no longer recognize.
fn drop_cut_word(bytes: &mut Vec<u8>) {
    let cut = bytes
        .iter()
        .position(u8::is_ascii_whitespace)
        .map_or(bytes.len(), |space| space + 1);

    bytes.drain(..cut);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured(bytes: &[u8], capture: Capture) -> Captured {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("a runtime")
            .block_on(capture.read(bytes))
    }

    #[test]
    fn the_tail_keeps_the_last_bytes_without_the_cut_off_word() {
        let output = format!("{}ghp_cutoffsecret last line\n", "x".repeat(40));
        let tail = captured(output.as_bytes(), Capture { cap: 26, tail: true });

        assert!(tail.truncated);
        assert_eq!(tail.bytes, b"last line\n", "the cut-off word is dropped");

        let whole = captured(b"short\n", Capture { cap: 26, tail: true });
        assert!(!whole.truncated);
        assert_eq!(whole.bytes, b"short\n");

        let head = captured(output.as_bytes(), Capture { cap: 26, tail: false });
        assert!(head.truncated);
        assert_eq!(head.bytes, &output.as_bytes()[..26], "hooks keep the head");
    }

    #[test]
    fn a_tail_with_one_long_word_keeps_nothing() {
        let tail = captured("y".repeat(100).as_bytes(), Capture { cap: 10, tail: true });

        assert!(tail.truncated);
        assert!(tail.bytes.is_empty());
    }
}
