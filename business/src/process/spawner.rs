//! Long-lived children with piped stdio (agent RPC sessions).

use std::process::Stdio;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::Child;

#[cfg(unix)]
use super::kill_group;
use super::{BoxFuture, ExecSpec, ExitInfo, build_command, kill_tree, spawn_command};
use crate::error::{BusinessError, Result};

/// Control over a spawned child, independent of how it was created.
pub trait ChildHandle: Send {
    /// OS process id while the child has not been reaped.
    fn id(&self) -> Option<u32>;

    /// Waits for the child to exit.
    fn wait(&mut self) -> BoxFuture<'_, Result<ExitInfo>>;

    /// Starts killing the child (and its process group on Unix, its process tree on Windows)
    /// without waiting.
    /// Succeeds when the child already exited.
    fn start_kill(&mut self) -> Result<()>;

    /// Kills whatever is left of the child's process group, also after the child was reaped
    /// (background processes it left behind). Unix only; a no-op elsewhere and by default.
    ///
    /// The group id stays reserved while any member lives, so this reaches only the child's group;
    /// once every member is gone the id is free and could in theory be taken by a new group.
    fn kill_process_group(&mut self) {}
}

/// The stdio streams and control handle of a spawned child.
pub struct SpawnedProcess {
    /// Writes to the child's stdin; dropping it closes stdin (agents treat EOF as shutdown).
    pub stdin: Box<dyn AsyncWrite + Send + Unpin>,
    /// Reads the child's stdout.
    pub stdout: Box<dyn AsyncRead + Send + Unpin>,
    /// Reads the child's stderr.
    pub stderr: Box<dyn AsyncRead + Send + Unpin>,
    /// Waits for or kills the child.
    pub child: Box<dyn ChildHandle>,
}

impl std::fmt::Debug for SpawnedProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpawnedProcess")
            .field("id", &self.child.id())
            .finish_non_exhaustive()
    }
}

/// Starts long-lived children. Implementations never use a shell.
pub trait ProcessSpawner: Send + Sync {
    /// Spawns `spec` with piped stdin/stdout/stderr. `timeout` and `max_output_bytes` are ignored.
    fn spawn(&self, spec: &ExecSpec) -> Result<SpawnedProcess>;
}

/// [`ProcessSpawner`] backed by `tokio::process`.
///
/// The child is killed when its handle is dropped; on Unix the whole process group is killed, on
/// Windows its process tree (`taskkill /T`) while the child was not reaped yet.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioSpawner;

impl ProcessSpawner for TokioSpawner {
    fn spawn(&self, spec: &ExecSpec) -> Result<SpawnedProcess> {
        let command = build_command(spec, Stdio::piped())?;
        let mut child = spawn_command(command, spec)?;

        let (Some(stdin), Some(stdout), Some(stderr)) = (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            return Err(BusinessError::Internal("child pipes missing".to_owned()));
        };

        // The child leads its own process group, so the group id is its pid.
        let group = child.id();

        Ok(SpawnedProcess {
            stdin: Box::new(stdin),
            stdout: Box::new(stdout),
            stderr: Box::new(stderr),
            child: Box::new(TokioChild { child, group }),
        })
    }
}

/// A `tokio::process::Child` that kills its process group when dropped unreaped.
struct TokioChild {
    child: Child,
    /// The child's process group id, kept for [`ChildHandle::kill_process_group`] after reaping.
    group: Option<u32>,
}

impl ChildHandle for TokioChild {
    fn id(&self) -> Option<u32> {
        self.child.id()
    }

    fn wait(&mut self) -> BoxFuture<'_, Result<ExitInfo>> {
        Box::pin(async move {
            let status = self
                .child
                .wait()
                .await
                .map_err(|error| BusinessError::Process(format!("waiting for child: {error}")))?;

            Ok(ExitInfo::from(status))
        })
    }

    fn start_kill(&mut self) -> Result<()> {
        kill_tree(&mut self.child)
    }

    fn kill_process_group(&mut self) {
        #[cfg(unix)]
        if let Some(group) = self.group {
            kill_group(group);
        }

        // Windows: the tree is found by parent pid, so what the exited child left behind is not
        // reached.
        #[cfg(not(unix))]
        let _ = self.group;
    }
}

impl Drop for TokioChild {
    fn drop(&mut self) {
        // `kill_on_drop` covers only the direct child; this also reaches its process group or tree.
        kill_tree(&mut self.child).ok();
    }
}
