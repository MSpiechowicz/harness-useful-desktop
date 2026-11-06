//! The lock around every read-change-write of `guards/global.json` ([`GlobalLock`]): a mutex for
//! this process and an exclusive file lock on `guards/global.lock` for every other process that
//! writes the file (a second app, a dev build next to a release one).

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::sync::{Mutex, MutexGuard, TryLockError as MutexTryLockError};
use std::time::{Duration, Instant};

use super::ready_guards_dir;
use crate::error::{BusinessError, Result};

/// The lock file, in the guards folder. It holds nothing: the lock is on the file itself.
pub const LOCK_FILE: &str = "global.lock";

/// How long a lock held by another process is waited for.
const LOCK_WAIT: Duration = Duration::from_secs(5);

/// The pause between two tries of a lock another process holds.
const LOCK_RETRY: Duration = Duration::from_millis(25);

/// One change of `guards/global.json` at a time in this process, so two changes made at once both
/// apply.
static GLOBAL_WRITES: Mutex<()> = Mutex::new(());

/// Held for the whole read-change-write of `guards/global.json` and released when dropped.
pub(super) struct GlobalLock {
    _process: MutexGuard<'static, ()>,
    _file: FileLock,
}

impl GlobalLock {
    /// Takes the lock for the guards in `data_dir`, creating the guards folder when it is missing:
    /// `protocol` when this process or another keeps it for more than 5 seconds in all (one
    /// deadline for both), so a stuck holder cannot hang the app and N waiters do not queue
    /// N times 5 seconds.
    pub(super) fn take(data_dir: &Path) -> Result<Self> {
        let deadline = Instant::now() + LOCK_WAIT;
        let process = take_process_lock(deadline)?;
        let guards = ready_guards_dir(data_dir)?;
        let file = FileLock::take(&guards, deadline.saturating_duration_since(Instant::now()))?;

        Ok(Self {
            _process: process,
            _file: file,
        })
    }
}

/// The mutex of this process, tried again until `deadline`.
fn take_process_lock(deadline: Instant) -> Result<MutexGuard<'static, ()>> {
    loop {
        match GLOBAL_WRITES.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(MutexTryLockError::Poisoned(poisoned)) => return Ok(poisoned.into_inner()),
            Err(MutexTryLockError::WouldBlock) if Instant::now() < deadline => std::thread::sleep(LOCK_RETRY),
            Err(MutexTryLockError::WouldBlock) => return Err(busy()),
        }
    }
}

/// The exclusive lock on [`LOCK_FILE`], without the process mutex; released when dropped.
pub(super) struct FileLock {
    _held: Option<File>,
}

impl FileLock {
    /// Takes the lock on `guards/global.lock` in `guards`, trying again for `wait`. `protocol` when
    /// it stays taken. A platform that has no file locks gets a warning and no lock (changes are
    /// then only serialised inside this process).
    pub(super) fn take(guards: &Path, wait: Duration) -> Result<Self> {
        let file = open_lock_file(&guards.join(LOCK_FILE))?;
        let deadline = Instant::now() + wait;

        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _held: Some(file) }),
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => std::thread::sleep(LOCK_RETRY),
                Err(TryLockError::WouldBlock) => return Err(busy()),
                Err(TryLockError::Error(error)) if error.kind() == std::io::ErrorKind::Unsupported => {
                    tracing::warn!("this platform has no file locks: the memory save guards are changed without one");
                    return Ok(Self { _held: None });
                }
                Err(TryLockError::Error(error)) => {
                    return Err(BusinessError::from_io("locking the Harness Memory guards", error));
                }
            }
        }
    }
}

fn busy() -> BusinessError {
    BusinessError::Protocol("the Harness Memory settings are busy, another Harness Desktop is changing them".to_owned())
}

/// Opens (creating it `0600` when missing) the lock file `path`, never through a symbolic link.
fn open_lock_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.mode(0o600).custom_flags(nix::libc::O_NOFOLLOW);
    }

    #[cfg(not(unix))]
    crate::fs_atomic::refuse_link(path)?;

    let file = options
        .open(path)
        .map_err(|error| BusinessError::from_io("opening the Harness Memory guards lock", error))?;

    let is_file = file.metadata().is_ok_and(|metadata| metadata.is_file());

    if !is_file {
        return Err(BusinessError::InvalidInput(format!(
            "{LOCK_FILE} in the Harness Memory guards folder is not a plain file"
        )));
    }

    Ok(file)
}
