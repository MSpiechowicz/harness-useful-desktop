//! Run-to-completion execution with timeout and output cap.

use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::time::{Instant, timeout_at};

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
/// stdin is null, each output stream is capped (excess is drained and discarded so the child
/// never blocks on a full pipe), and a timeout kills the child and its process group.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioRunner;

impl ProcessRunner for TokioRunner {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        Box::pin(run_to_completion(spec))
    }
}

async fn run_to_completion(spec: &ExecSpec) -> Result<ExecOutput> {
    let command = build_command(spec, Stdio::null())?;
    let mut child = spawn_command(command, spec)?;

    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(BusinessError::Internal("child pipes missing".to_owned()));
    };

    let deadline = Instant::now() + spec.timeout;
    let cap = spec.max_output_bytes;

    let waiter = async {
        match timeout_at(deadline, child.wait()).await {
            Ok(status) => status.map(|status| (status.code(), false)),
            Err(_elapsed) => {
                kill_tree(&mut child).ok();
                child.wait().await.map(|_| (None, true))
            }
        }
    };

    let all = async { tokio::join!(read_capped(stdout, cap), read_capped(stderr, cap), waiter) };

    // The pipes normally close when the process group dies; a descendant that escaped the group
    // could keep them open, so collecting is bounded too.
    let Ok((out, err, waited)) = timeout_at(deadline + DRAIN_GRACE, all).await else {
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

/// Bytes captured from one stream.
struct Captured {
    bytes: Vec<u8>,
    truncated: bool,
}

/// Reads `reader` to EOF, keeping at most `cap` bytes and discarding the rest.
async fn read_capped(mut reader: impl AsyncRead + Unpin, cap: usize) -> Captured {
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

        let room = cap.saturating_sub(captured.bytes.len());
        let keep = read.min(room);
        captured.bytes.extend_from_slice(&chunk[..keep]);

        if keep < read {
            captured.truncated = true;
        }
    }

    captured
}
