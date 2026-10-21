//! Chat servers and agents must not outlive the app. Each child leads its own process group (see
//! the module docs of [`crate::process`]), so when the app dies abruptly (a crash, SIGKILL, a
//! restarted dev build) nothing stops them, and `opencode serve` does not exit on stdin EOF either.
//!
//! - [`TrackedSpawner`] records each child it starts in a [`ChildRegistry`]: one small JSON file
//!   per running app (`app-<pid>-<nanos>.json` in [`crate::app_dirs::child_processes_dir`]),
//!   naming the app itself and its children by [`ProcessIdentity`]. An entry is removed once its
//!   child was reaped or its handle dropped (which kills it); the file goes once it is empty.
//! - [`sweep_orphans`] runs once at app start: for every file whose app is gone, it kills each
//!   recorded child that still runs and is still provably the same process, then removes the file.
//!   A file of an app that still runs (a dev build next to the installed app) is left alone.
//! - On Unix the registry is trusted only while it is private to the user the app runs as: the
//!   folder is narrowed to `0700` on every write; the sweep refuses a folder that another user
//!   owns or that group or others can write to, and skips (leaves unread) a file that another user
//!   owns or that group or others may open (any mode beyond the owner's bits). Nobody else can plant a record that makes the app kill a
//!   process.
//!
//! Nothing is recorded where identities are unsupported ([`ChildRegistry::open`] gives `None`), so
//! Windows children are not swept. `PR_SET_PDEATHSIG` is not used: it fires when the spawning
//! thread exits, and tokio's blocking pool retires threads.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::identity::{ProcessIdentity, kill_observed};
use super::{BoxFuture, ChildHandle, ExecSpec, ExitInfo, ProcessSpawner, SpawnedProcess};
use crate::error::{BusinessError, Result};
use crate::fs_atomic::{ensure_private_dir, replace_atomically, unix_nanos};
use record::{ReadRecord, read_record};

mod record;

/// The most children one app records; more are started untracked (and logged).
pub const MAX_RECORDED_CHILDREN: usize = 64;

/// The most registry files one sweep reads.
const MAX_FILES: usize = 64;

/// A registry file larger than this is not read (and removed).
const MAX_FILE_BYTES: u64 = 256 * 1024;

/// A temporary file older than this was left by a crash during a write and is removed.
const STALE_TEMP: Duration = Duration::from_secs(10 * 60);

/// The registry file format version.
const VERSION: u32 = 1;

/// One app's registry file.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryFile {
    version: u32,
    owner: ProcessIdentity,
    children: Vec<ProcessIdentity>,
}

/// The children one running app started (see the module docs).
#[derive(Debug)]
pub struct ChildRegistry {
    file: PathBuf,
    owner: ProcessIdentity,
    children: Mutex<Vec<ProcessIdentity>>,
}

impl ChildRegistry {
    /// The current app's registry in `dir`; `None` where process identities are not supported
    /// (Windows). Nothing is written before the first child is recorded.
    pub fn open(dir: impl Into<PathBuf>) -> Option<Arc<Self>> {
        let Some(owner) = ProcessIdentity::current() else {
            tracing::debug!("child processes are not recorded on this system");
            return None;
        };

        Some(Self::with_owner(dir, owner))
    }

    /// A registry in `dir` on behalf of the app `owner` (tests stand in a dummy process for the
    /// app, so they can let it die).
    pub fn with_owner(dir: impl Into<PathBuf>, owner: ProcessIdentity) -> Arc<Self> {
        let file = dir.into().join(format!("app-{}-{}.json", owner.pid, unix_nanos()));

        Arc::new(Self {
            file,
            owner,
            children: Mutex::new(Vec::new()),
        })
    }

    /// This app's registry file.
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Records running child `pid`, started with `args` after its program; `None` (logged) when
    /// it cannot be identified, the registry is full or the file cannot be written.
    pub fn record(&self, pid: u32, args: &[OsString]) -> Option<ProcessIdentity> {
        let identity = ProcessIdentity::capture(pid, args)?;
        let mut children = self.children.lock().unwrap_or_else(PoisonError::into_inner);

        if children.len() >= MAX_RECORDED_CHILDREN {
            tracing::warn!(
                pid,
                "too many child processes to record, this one is not swept after a crash"
            );
            return None;
        }

        children.push(identity.clone());

        if let Err(error) = self.write(&children) {
            tracing::warn!(%error, pid, "could not record a child process");
            children.pop();
            return None;
        }

        Some(identity)
    }

    /// Removes the entry of `identity` (its child was reaped or killed).
    pub fn remove(&self, identity: &ProcessIdentity) {
        let mut children = self.children.lock().unwrap_or_else(PoisonError::into_inner);
        let before = children.len();

        children.retain(|child| child != identity);

        if children.len() == before {
            return;
        }

        if let Err(error) = self.write(&children) {
            tracing::debug!(%error, "could not update the child process record");
        }
    }

    /// Writes `children` to the file (`0600`, atomically), or removes it when there are none.
    fn write(&self, children: &[ProcessIdentity]) -> Result<()> {
        if children.is_empty() {
            return match fs::remove_file(&self.file) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(BusinessError::from_io("removing the child process record", error)),
            };
        }

        if let Some(dir) = self.file.parent() {
            ensure_private_dir(dir, "creating the child process folder")?;
        }

        let record = RegistryFile {
            version: VERSION,
            owner: self.owner.clone(),
            children: children.to_vec(),
        };
        let bytes = serde_json::to_vec(&record).map_err(|error| BusinessError::Internal(error.to_string()))?;

        replace_atomically(&self.file, &bytes, "saving the child process record")
    }
}

/// A [`ProcessSpawner`] that records each child of `inner` in a [`ChildRegistry`] until it is
/// reaped or its handle dropped (see the module docs).
pub struct TrackedSpawner {
    inner: Arc<dyn ProcessSpawner>,
    registry: Arc<ChildRegistry>,
}

impl TrackedSpawner {
    /// Children of `inner`, recorded in `registry`.
    pub fn new(inner: Arc<dyn ProcessSpawner>, registry: Arc<ChildRegistry>) -> Self {
        Self { inner, registry }
    }
}

impl ProcessSpawner for TrackedSpawner {
    fn spawn(&self, spec: &ExecSpec) -> Result<SpawnedProcess> {
        let mut process = self.inner.spawn(spec)?;

        let mut args = spec.args.clone();
        args.extend(spec.raw_arg.clone());

        let recorded = process.child.id().and_then(|pid| self.registry.record(pid, &args));

        if let Some(identity) = recorded {
            process.child = Box::new(TrackedChild {
                inner: process.child,
                registry: self.registry.clone(),
                identity: Some(identity),
            });
        }

        Ok(process)
    }
}

/// A child whose registry entry goes once it was reaped or its handle dropped.
struct TrackedChild {
    inner: Box<dyn ChildHandle>,
    registry: Arc<ChildRegistry>,
    identity: Option<ProcessIdentity>,
}

impl TrackedChild {
    fn release(&mut self) {
        if let Some(identity) = self.identity.take() {
            self.registry.remove(&identity);
        }
    }
}

impl ChildHandle for TrackedChild {
    fn id(&self) -> Option<u32> {
        self.inner.id()
    }

    fn wait(&mut self) -> BoxFuture<'_, Result<ExitInfo>> {
        Box::pin(async move {
            let result = self.inner.wait().await;

            if result.is_ok() {
                self.release();
            }

            result
        })
    }

    fn start_kill(&mut self) -> Result<()> {
        self.inner.start_kill()
    }

    fn kill_process_group(&mut self) {
        self.inner.kill_process_group();
    }

    fn terminate(&mut self) {
        self.inner.terminate();
    }
}

impl Drop for TrackedChild {
    fn drop(&mut self) {
        // The inner handle kills the child when dropped too; killing first means the entry never
        // goes while the child could still outlive the app.
        self.inner.start_kill().ok();
        self.release();
    }
}

/// What [`sweep_orphans`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SweepReport {
    /// The pids of the left-over children it killed.
    pub killed: Vec<u32>,
    /// Recorded children that were gone or now name another process (left alone).
    pub gone: usize,
    /// Registry files of apps that still run (left alone).
    pub live_apps: usize,
    /// Unreadable, oversized or corrupt registry files (removed unread).
    pub corrupt: usize,
    /// Registry files that are not private to this user (left alone, unread).
    pub untrusted: usize,
}

/// Kills the children recorded by apps that are gone, when they still run and are still the
/// same process, and removes those apps' registry files (see the module docs). Blocks (it reads
/// files and, on macOS, runs `ps`); a missing `dir`, or one that is not private to this user, is
/// an empty sweep.
pub fn sweep_orphans(dir: &Path) -> SweepReport {
    let mut report = SweepReport::default();

    let Ok(metadata) = fs::symlink_metadata(dir) else {
        return report;
    };

    if !metadata.is_dir() || !private_dir(&metadata) {
        tracing::warn!(
            dir = %dir.display(),
            "the child process folder is not private to this user, nothing left by an earlier run is stopped"
        );
        return report;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return report;
    };

    // Only the registry's own files count towards the limit, in a stable order.
    let mut files: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();

            (is_temp_file(&name) || is_record_file(&name)).then(|| (name, entry.path()))
        })
        .collect();
    files.sort();

    for (name, path) in files.into_iter().take(MAX_FILES) {
        if is_temp_file(&name) {
            remove_stale_temp(&path);
            continue;
        }

        let record = match read_record(&path) {
            ReadRecord::Record(record) => record,
            ReadRecord::Untrusted => {
                tracing::warn!(file = %name, "skipped a child process record that is not private to this user");
                report.untrusted += 1;
                continue;
            }
            ReadRecord::Unavailable => {
                tracing::warn!(file = %name, "skipped a child process record that could not be read now");
                continue;
            }
            ReadRecord::Corrupt => {
                tracing::warn!(file = %name, "removed an unreadable child process record");
                report.corrupt += 1;
                fs::remove_file(&path).ok();
                continue;
            }
        };

        if record.owner.is_alive() {
            report.live_apps += 1;
            continue;
        }

        for child in record.children {
            sweep_child(&child, &mut report);
        }

        fs::remove_file(&path).ok();
    }

    report
}

/// A temporary file of an atomic registry write.
fn is_temp_file(name: &str) -> bool {
    name.starts_with(".app-") && name.contains(".tmp-")
}

/// An app's registry file.
fn is_record_file(name: &str) -> bool {
    name.starts_with("app-") && name.ends_with(".json")
}

/// Kills `child` when it still runs as recorded.
fn sweep_child(child: &ProcessIdentity, report: &mut SweepReport) {
    let Some(observed) = child.observe_matching() else {
        report.gone += 1;
        return;
    };

    kill_observed(child.pid, &observed);
    tracing::info!(pid = child.pid, "stopped a child process left by an earlier run");
    report.killed.push(child.pid);
}

/// Removes the temporary file at `path` once it is old enough not to be a write in progress.
fn remove_stale_temp(path: &Path) {
    let age = fs::symlink_metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok());

    if age.is_some_and(|age| age > STALE_TEMP) {
        fs::remove_file(path).ok();
    }
}

/// True when the registry folder with `metadata` is private to this user (always on Windows).
#[cfg(unix)]
fn private_dir(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    private_dir_mode(metadata.uid(), metadata.mode(), nix::unistd::geteuid().as_raw())
}

#[cfg(not(unix))]
fn private_dir(_metadata: &fs::Metadata) -> bool {
    true
}

/// True when the registry file with `metadata` is private to this user (always on Windows).
#[cfg(unix)]
fn private_file(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    private_file_mode(metadata.uid(), metadata.mode(), nix::unistd::geteuid().as_raw())
}

#[cfg(not(unix))]
fn private_file(_metadata: &fs::Metadata) -> bool {
    true
}

/// A folder owned by `owner` with mode `mode` is private to user `me`: theirs, and neither group
/// nor others may write to it.
#[cfg(unix)]
fn private_dir_mode(owner: u32, mode: u32, me: u32) -> bool {
    owner == me && mode & 0o022 == 0
}

/// A file owned by `owner` with mode `mode` is private to user `me`: theirs, and neither group
/// nor others may touch it. Any owner-only mode counts, so a strict umask's `0400` still does.
#[cfg(unix)]
fn private_file_mode(owner: u32, mode: u32, me: u32) -> bool {
    owner == me && mode & 0o077 == 0
}

#[cfg(all(test, unix))]
mod tests {
    use super::{private_dir_mode, private_file_mode};

    const ME: u32 = 1000;
    const OTHER: u32 = 1001;

    #[test]
    fn only_the_users_own_private_folder_is_trusted() {
        assert!(private_dir_mode(ME, 0o40700, ME));
        assert!(private_dir_mode(ME, 0o40755, ME), "others may read, not write");

        assert!(!private_dir_mode(OTHER, 0o40700, ME), "another user's folder");
        assert!(!private_dir_mode(0, 0o40700, ME), "root's folder");
        assert!(!private_dir_mode(ME, 0o40770, ME), "group may write");
        assert!(!private_dir_mode(ME, 0o40777, ME), "others may write");
    }

    #[test]
    fn only_the_users_own_owner_only_file_is_trusted() {
        assert!(private_file_mode(ME, 0o100600, ME));
        assert!(private_file_mode(ME, 0o100400, ME), "read only, from a strict umask");
        assert!(private_file_mode(ME, 0o100700, ME), "owner-only executable bit");

        assert!(!private_file_mode(OTHER, 0o100600, ME), "another user's file");
        assert!(!private_file_mode(0, 0o100400, ME), "root's file");
        assert!(!private_file_mode(ME, 0o100644, ME), "others may read it");
        assert!(!private_file_mode(ME, 0o100640, ME), "group may read it");
        assert!(!private_file_mode(ME, 0o100604, ME), "others may read it");
        assert!(!private_file_mode(ME, 0o100620, ME), "group may write it");
    }
}
