//! In-memory process doubles for tests (enabled by `cfg(test)` or the `test-support` feature).
//!
//! [`DuplexSpawner`] implements [`ProcessSpawner`] over `tokio::io::duplex`, so session-loop tests
//! can script an agent without starting a real process.

use std::sync::Mutex;

use tokio::io::{DuplexStream, duplex};
use tokio::sync::{mpsc, oneshot, watch};

use crate::error::{BusinessError, Result};
use crate::process::{BoxFuture, ChildHandle, ExecSpec, ExitInfo, ProcessSpawner, SpawnedProcess};

/// Buffer size of each in-memory pipe.
const PIPE_BYTES: usize = 64 * 1024;

/// Exit reported when a [`FakePeer`] is killed (`SIGKILL`).
pub const KILLED: ExitInfo = ExitInfo {
    code: None,
    signal: Some(9),
};

/// The "agent" side of a process spawned by [`DuplexSpawner`].
#[derive(Debug)]
pub struct FakePeer {
    /// The spec the app asked to spawn.
    pub spec: ExecSpec,
    /// Reads what the app writes to the child's stdin (EOF when the app drops stdin).
    pub stdin: DuplexStream,
    /// Writes what the app reads from the child's stdout.
    pub stdout: DuplexStream,
    /// Writes what the app reads from the child's stderr.
    pub stderr: DuplexStream,
    exit: oneshot::Sender<ExitInfo>,
    killed: watch::Receiver<bool>,
}

impl FakePeer {
    /// Ends the fake process with `info`; the app's `wait()` resolves to it.
    pub fn exit(self, info: ExitInfo) {
        self.exit.send(info).ok();
    }

    /// Ends the fake process with `info` like [`FakePeer::exit`], keeping a probe that tells
    /// whether the app killed it or its process group afterwards.
    pub fn exit_watched(self, info: ExitInfo) -> KillProbe {
        self.exit.send(info).ok();

        KillProbe { killed: self.killed }
    }

    /// True once the app called `start_kill` or `kill_process_group`.
    pub fn is_killed(&self) -> bool {
        *self.killed.borrow()
    }
}

/// Tells whether the app killed an exited [`FakePeer`] or its process group.
#[derive(Debug)]
pub struct KillProbe {
    killed: watch::Receiver<bool>,
}

impl KillProbe {
    /// True once the app called `start_kill` or `kill_process_group`.
    pub fn is_killed(&self) -> bool {
        *self.killed.borrow()
    }
}

/// A [`ProcessSpawner`] that hands each spawned process's other end to the test.
#[derive(Debug)]
pub struct DuplexSpawner {
    peers: mpsc::UnboundedSender<FakePeer>,
    fail_next: Mutex<Option<String>>,
}

impl DuplexSpawner {
    /// The spawner plus the receiver that yields one [`FakePeer`] per `spawn` call.
    pub fn new() -> (Self, mpsc::UnboundedReceiver<FakePeer>) {
        let (peers, receiver) = mpsc::unbounded_channel();
        let spawner = Self {
            peers,
            fail_next: Mutex::new(None),
        };

        (spawner, receiver)
    }

    /// Makes the next `spawn` fail with a process error carrying `message`.
    pub fn fail_next_spawn(&self, message: impl Into<String>) {
        if let Ok(mut slot) = self.fail_next.lock() {
            *slot = Some(message.into());
        }
    }
}

impl ProcessSpawner for DuplexSpawner {
    fn spawn(&self, spec: &ExecSpec) -> Result<SpawnedProcess> {
        if let Some(message) = self.fail_next.lock().ok().and_then(|mut slot| slot.take()) {
            return Err(BusinessError::Process(message));
        }

        let (app_stdin, peer_stdin) = duplex(PIPE_BYTES);
        let (app_stdout, peer_stdout) = duplex(PIPE_BYTES);
        let (app_stderr, peer_stderr) = duplex(PIPE_BYTES);
        let (exit_tx, exit_rx) = oneshot::channel();
        let (kill_tx, kill_rx) = watch::channel(false);

        let peer = FakePeer {
            spec: spec.clone(),
            stdin: peer_stdin,
            stdout: peer_stdout,
            stderr: peer_stderr,
            exit: exit_tx,
            killed: kill_rx,
        };

        self.peers
            .send(peer)
            .map_err(|_| BusinessError::Process("test peer receiver dropped".to_owned()))?;

        Ok(SpawnedProcess {
            stdin: Box::new(app_stdin),
            stdout: Box::new(app_stdout),
            stderr: Box::new(app_stderr),
            child: Box::new(FakeChild {
                exit: Some(exit_rx),
                kill: kill_tx,
                done: None,
            }),
        })
    }
}

/// The app's handle on a fake process.
struct FakeChild {
    exit: Option<oneshot::Receiver<ExitInfo>>,
    kill: watch::Sender<bool>,
    done: Option<ExitInfo>,
}

impl ChildHandle for FakeChild {
    fn id(&self) -> Option<u32> {
        None
    }

    fn wait(&mut self) -> BoxFuture<'_, Result<ExitInfo>> {
        Box::pin(async move {
            if let Some(done) = self.done {
                return Ok(done);
            }

            let mut killed = self.kill.subscribe();
            let info = match self.exit.as_mut() {
                // A dropped peer counts as a crash without exit code.
                Some(exit) => tokio::select! {
                    result = exit => result.unwrap_or_default(),
                    _ = killed.wait_for(|killed| *killed) => KILLED,
                },
                None => KILLED,
            };

            self.exit = None;
            self.done = Some(info);
            Ok(info)
        })
    }

    fn start_kill(&mut self) -> Result<()> {
        self.kill.send_replace(true);
        Ok(())
    }

    fn kill_process_group(&mut self) {
        self.kill.send_replace(true);
    }
}
