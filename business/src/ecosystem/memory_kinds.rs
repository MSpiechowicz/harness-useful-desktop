//! Which team databases the installed Harness Memory can use: `harness-memory kinds` prints one
//! kind per line (`postgres`, `libsql`, `mongodb`, `documentdb`) and exits 0. An older server does
//! not know the command and exits non-zero: it uses PostgreSQL, libSQL and MongoDB only, and
//! refuses a config file with the `database_kind` key.
//!
//! [`MemoryKinds::support`] asks once per binary: the answer is cached until the binary at that
//! path changes (its modification time, length, or on Unix its device and inode). A usage error (exit 2, or a line naming an unknown command) is cached as an older
//! server, any other failure answers [`KindsSupport::Unsupported`] without being cached, and a
//! server that does not start or answer in time is [`KindsSupport::Unknown`] (not cached either).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use super::memory_binary::resolve_memory;
use super::memory_cli::{CliAnswer, CliCall, USAGE_EXIT, run_memory_cli};
use crate::env::Env;
use crate::process::ProcessRunner;

/// How long `kinds` may take.
pub const KINDS_TIMEOUT: Duration = Duration::from_secs(10);

/// The output kept of the probe (the answer is a few short lines).
const KINDS_OUTPUT_BYTES: usize = 16 * 1024;

/// The line `kinds` prints for Amazon DocumentDB.
const DOCUMENTDB: &str = "documentdb";

/// Whether the installed server can use Amazon DocumentDB, which decides whether the
/// `database_kind` key may be saved and a DocumentDB database tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindsSupport {
    /// It answers `kinds`: whether `documentdb` is one of the lines.
    Listed {
        /// `documentdb` is listed.
        documentdb: bool,
    },
    /// It predates `kinds` (or failed it): no DocumentDB.
    Unsupported,
    /// Harness Memory is not installed.
    NotInstalled,
    /// The server could not be asked (it did not start or answer in time).
    Unknown,
}

impl KindsSupport {
    /// Whether the server can use Amazon DocumentDB.
    pub fn documentdb(self) -> bool {
        self == Self::Listed { documentdb: true }
    }
}

/// The probe of the installed server's database kinds. See the module docs.
#[derive(Debug, Default)]
pub struct MemoryKinds {
    probe: Mutex<Option<Probe>>,
}

/// A cached answer of one binary, as it was when probed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Probe {
    path: PathBuf,
    stamp: Stamp,
    answer: KindsSupport,
}

/// What tells one binary at a path from another: a replaced file gets a new inode (Unix) even when
/// its time and length match the old one's.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    modified: SystemTime,
    len: u64,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

impl Stamp {
    /// The stamp of the file at `path`; `None` when it cannot be read.
    fn of(path: &Path) -> Option<Self> {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        let metadata = std::fs::metadata(path).ok()?;

        Some(Self {
            modified: metadata.modified().ok()?,
            len: metadata.len(),
            #[cfg(unix)]
            dev: metadata.dev(),
            #[cfg(unix)]
            ino: metadata.ino(),
        })
    }
}

/// What one run of `kinds` says, and whether it may be cached.
struct Answer {
    support: KindsSupport,
    cache: bool,
}

impl MemoryKinds {
    /// No probe yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the installed server can use Amazon DocumentDB (see the module docs).
    pub async fn support(&self, env: &Env, runner: &dyn ProcessRunner) -> KindsSupport {
        let Some(binary) = resolve_memory(env) else {
            return KindsSupport::NotInstalled;
        };

        let stamp = Stamp::of(&binary.path);

        if let Some(stamp) = &stamp
            && let Some(answer) = self.cached(&binary.path, stamp)
        {
            return answer;
        }

        let answer = ask(env, runner, &binary.path).await;

        if answer.cache
            && let Some(stamp) = stamp
        {
            self.remember(Probe {
                path: binary.path,
                stamp,
                answer: answer.support,
            });
        }

        answer.support
    }

    fn cached(&self, path: &Path, stamp: &Stamp) -> Option<KindsSupport> {
        let probe = self.probe.lock().unwrap_or_else(PoisonError::into_inner);

        probe
            .as_ref()
            .filter(|probe| probe.path == path && probe.stamp == *stamp)
            .map(|probe| probe.answer)
    }

    fn remember(&self, probe: Probe) {
        *self.probe.lock().unwrap_or_else(PoisonError::into_inner) = Some(probe);
    }
}

/// `kinds` on `server`.
async fn ask(env: &Env, runner: &dyn ProcessRunner, server: &Path) -> Answer {
    let call = CliCall {
        name: "kinds",
        args: vec![OsString::from("kinds")],
        timeout: KINDS_TIMEOUT,
        output_bytes: KINDS_OUTPUT_BYTES,
        stdin: None,
    };

    match run_memory_cli(env, runner, server, call).await {
        Ok(CliAnswer::Done(stdout)) => {
            let documentdb = stdout.lines().any(|line| line.trim().eq_ignore_ascii_case(DOCUMENTDB));

            Answer {
                support: KindsSupport::Listed { documentdb },
                cache: true,
            }
        }
        Ok(CliAnswer::Failed {
            status: Some(code),
            message,
            ..
        }) => {
            let older = code == USAGE_EXIT || names_unknown_command(&message);

            if !older {
                tracing::warn!(code, "harness-memory kinds failed");
            }

            Answer {
                support: KindsSupport::Unsupported,
                cache: older,
            }
        }
        Ok(CliAnswer::Failed { status: None, .. }) => {
            tracing::warn!("harness-memory kinds was stopped");
            unknown()
        }
        Err(error) => {
            tracing::warn!(code = ?error.code(), "harness-memory kinds did not run");
            unknown()
        }
    }
}

fn unknown() -> Answer {
    Answer {
        support: KindsSupport::Unknown,
        cache: false,
    }
}

/// True when the server's line says it does not know the command (a server whose usage errors do
/// not exit 2).
fn names_unknown_command(message: &str) -> bool {
    let message = message.to_lowercase();

    ["unrecognized subcommand", "unknown subcommand", "unknown command"]
        .iter()
        .any(|marker| message.contains(marker))
}
