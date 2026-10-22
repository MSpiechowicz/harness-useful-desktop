//! A child the caller talks to line by line on its own thread, for synchronous callers that need a
//! short conversation with a helper process (the Codex Sessions view asks `codex app-server`).
//!
//! - stdin is piped and written a line at a time ([`LineChild::write_line`]).
//! - stdout is read by a thread into lines of at most `max_line_bytes`; a longer line is reported
//!   as [`ReadLine::TooLong`] without its content. At most [`QUEUED_LINES`] lines wait unread, so
//!   a chatty child is held back by the pipe instead of filling memory.
//! - The last [`STDERR_TAIL_BYTES`] of stderr are kept for error messages.
//! - The child leads its own process group (Unix). [`LineChild::finish`] closes stdin and gives it
//!   until a deadline to exit; past it, and whenever a [`LineChild`] that did not exit is dropped,
//!   the child is killed with its group (its process tree on Windows).

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use super::blocking::{kill_tree, read_chunks};
use super::{ExecSpec, build_std_command, spawn_error};
use crate::error::{BusinessError, Result};

/// Lines read ahead of the caller at most.
pub const QUEUED_LINES: usize = 64;

/// How much of the end of stderr is kept.
pub const STDERR_TAIL_BYTES: usize = 8 * 1024;

/// How often [`LineChild::finish`] checks whether the child exited.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// What [`LineChild::read_line`] got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadLine {
    /// One stdout line without its line ending.
    Line(String),
    /// A line longer than the cap; its content was dropped.
    TooLong,
    /// stdout closed: the child ended (or closed it).
    Closed,
    /// Nothing arrived before the deadline.
    TimedOut,
}

/// A running child with piped stdio (see the module docs).
pub struct LineChild {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Option<Receiver<ReadLine>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    /// Gets a message (or disconnects) once stderr closed.
    stderr_closed: Receiver<()>,
    reaped: bool,
}

impl LineChild {
    /// Starts `spec` (its timeout and output cap are not used) with stdout lines capped at
    /// `max_line_bytes`.
    pub fn spawn(spec: &ExecSpec, max_line_bytes: usize) -> Result<Self> {
        let mut command = build_std_command(spec, Stdio::piped())?;
        let mut child = command.spawn().map_err(|error| spawn_error(&error, spec))?;

        let (Some(stdin), Some(stdout), Some(stderr)) = (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            kill_tree(&mut child);
            child.wait().ok();
            return Err(BusinessError::Internal("child pipes missing".to_owned()));
        };

        let (sender, lines) = sync_channel(QUEUED_LINES);
        thread::spawn(move || read_lines(stdout, max_line_bytes, &sender));

        let tail = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&tail);
        let (closed, stderr_closed) = sync_channel(1);
        thread::spawn(move || {
            keep_tail(stderr, &kept);
            closed.send(()).ok();
        });

        Ok(Self {
            child,
            stdin: Some(stdin),
            lines: Some(lines),
            stderr: tail,
            stderr_closed,
            reaped: false,
        })
    }

    /// Writes `line` and a line feed to stdin.
    pub fn write_line(&mut self, line: &str) -> Result<()> {
        let Some(stdin) = self.stdin.as_mut() else {
            return Err(BusinessError::Process("the child's input is closed".to_owned()));
        };

        stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.write_all(b"\n"))
            .and_then(|()| stdin.flush())
            .map_err(|error| BusinessError::Process(format!("writing to the child: {error}")))
    }

    /// The next stdout line, waiting until `deadline` at most.
    pub fn read_line(&mut self, deadline: Instant) -> ReadLine {
        let Some(lines) = &self.lines else {
            return ReadLine::Closed;
        };

        let left = deadline.saturating_duration_since(Instant::now());

        match lines.recv_timeout(left) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => ReadLine::TimedOut,
            Err(RecvTimeoutError::Disconnected) => ReadLine::Closed,
        }
    }

    /// The end of what the child wrote to stderr, once stderr closed or `wait` passed (a child
    /// that ended may still be flushing it).
    pub fn stderr_tail(&self, wait: Duration) -> String {
        self.stderr_closed.recv_timeout(wait).ok();

        let tail = self.stderr.lock().unwrap_or_else(PoisonError::into_inner);

        String::from_utf8_lossy(&tail).into_owned()
    }

    /// Closes stdin, stops reading stdout and waits until `deadline` for the child to exit; then
    /// it is killed with its group. True when it exited by itself.
    pub fn finish(mut self, deadline: Instant) -> bool {
        self.stdin.take();
        self.lines.take();

        loop {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) if Instant::now() >= deadline => {
                    self.kill();
                    return false;
                }
                Ok(None) => thread::sleep(POLL_INTERVAL),
            }
        }

        self.reaped = true;
        true
    }

    /// Kills the child with its group and reaps it.
    fn kill(&mut self) {
        if self.reaped {
            return;
        }

        kill_tree(&mut self.child);
        self.child.wait().ok();
        self.reaped = true;
    }
}

impl Drop for LineChild {
    fn drop(&mut self) {
        self.stdin.take();
        self.lines.take();

        // Reaped already, or exited meanwhile: nothing is left to kill.
        if !self.reaped && !matches!(self.child.try_wait(), Ok(Some(_))) {
            self.kill();
        }
    }
}

/// Sends the lines of `stdout` (see the module docs) until it closes or the receiver is gone.
fn read_lines(stdout: ChildStdout, max_line_bytes: usize, sender: &SyncSender<ReadLine>) {
    let mut reader = BufReader::new(stdout);
    let mut line = Vec::new();
    let mut too_long = false;

    loop {
        let buffer = match reader.fill_buf() {
            Ok([]) => break,
            Ok(buffer) => buffer,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };

        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let chunk = &buffer[..newline.unwrap_or(buffer.len())];

        if line.len() + chunk.len() > max_line_bytes {
            too_long = true;
            line.clear();
        } else if !too_long {
            line.extend_from_slice(chunk);
        }

        let used = newline.map_or(buffer.len(), |position| position + 1);
        reader.consume(used);

        if newline.is_some() && sender.send(finished(&mut line, &mut too_long)).is_err() {
            return;
        }
    }

    if !line.is_empty() || too_long {
        sender.send(finished(&mut line, &mut too_long)).ok();
    }
}

/// The line collected so far, which starts the next one empty.
fn finished(line: &mut Vec<u8>, too_long: &mut bool) -> ReadLine {
    if std::mem::take(too_long) {
        line.clear();
        return ReadLine::TooLong;
    }

    let text = String::from_utf8_lossy(line).trim_end_matches('\r').to_owned();
    line.clear();

    ReadLine::Line(text)
}

/// Keeps the last [`STDERR_TAIL_BYTES`] of `stderr` in `tail` until it closes.
fn keep_tail(stderr: ChildStderr, tail: &Mutex<Vec<u8>>) {
    read_chunks(stderr, |chunk| {
        let mut kept = tail.lock().unwrap_or_else(PoisonError::into_inner);
        kept.extend_from_slice(chunk);

        let excess = kept.len().saturating_sub(STDERR_TAIL_BYTES);
        kept.drain(..excess);
    });
}
