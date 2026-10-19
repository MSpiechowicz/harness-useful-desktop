//! Which team databases the installed Harness Memory can use: `harness-memory kinds` prints one
//! kind per line (`postgres`, `libsql`, `mongodb`, `documentdb`) and exits 0. An older server does
//! not know the command and exits non-zero: it uses PostgreSQL, libSQL and MongoDB only, and
//! refuses a config file with the `database_kind` key.
//!
//! [`MemoryKinds::support`] asks once per binary through a [`LineProbe`] (cached per binary, an
//! older server cached as one without DocumentDB, a failed probe not cached).

use super::memory_line_probe::{LineAnswer, LineProbe};
use crate::env::Env;
use crate::process::ProcessRunner;

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
#[derive(Debug)]
pub struct MemoryKinds {
    probe: LineProbe,
}

impl Default for MemoryKinds {
    fn default() -> Self {
        Self {
            probe: LineProbe::new("kinds"),
        }
    }
}

impl MemoryKinds {
    /// No probe yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the installed server can use Amazon DocumentDB (see the module docs).
    pub async fn support(&self, env: &Env, runner: &dyn ProcessRunner) -> KindsSupport {
        match self.probe.ask(env, runner).await {
            answer @ LineAnswer::Listed(_) => KindsSupport::Listed {
                documentdb: answer.lists(DOCUMENTDB),
            },
            LineAnswer::Unsupported => KindsSupport::Unsupported,
            LineAnswer::NotInstalled => KindsSupport::NotInstalled,
            LineAnswer::Unknown => KindsSupport::Unknown,
        }
    }
}
