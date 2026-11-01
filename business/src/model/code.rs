//! The code graph of Harness Code (`code_map`, `code_symbol`, `code_impact`, `code_path`): the
//! switch for it (`code_index_get`, `code_index_set`), the setting that registers its
//! server (`code_set_enabled`) and the builds the app tells the user about
//! (`code_builds_listen`).
//!
//! The index itself is the server's, see `crate::ecosystem::code`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{DashboardSetup, ManualRemoval};

/// The code index switch (`harness-code status --json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CodeIndexState {
    /// Whether the `code_*` tools are offered to agents (on unless switched off).
    pub enabled: bool,
    /// Where the switch comes from: `default` (never set), `file` (the user's choice) or `env`
    /// (the server's environment overrides it).
    pub source: String,
}

/// One step of a code graph build, as `code_builds_listen` sends it. Only the first index
/// of a repository (`full`) and a large change (`bulk`) are announced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CodeBuildEvent {
    /// The build; a new number is a new build of the same repository.
    #[ts(type = "number")]
    pub build_id: i64,
    /// The repository root (an absolute path).
    pub root: String,
    /// The last component of the root.
    pub repo_name: String,
    /// `full` (first index) or `bulk` (many files changed).
    pub kind: String,
    /// `building`, `ready` or `error`; `stale` when a build stopped reporting (dismiss it) and
    /// `cancelled` when the agent stopped it (dismiss it, it is not an error).
    pub phase: String,
    /// The files to index (0 until counted).
    #[ts(type = "number")]
    pub files_total: u64,
    /// The files indexed so far.
    #[ts(type = "number")]
    pub files_done: u64,
    /// Why it failed (`phase` is `error`; redacted, clipped).
    pub error: Option<String>,
}

/// What `code_set_enabled` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CodeSwitchResult {
    /// The stored `codeEnabled` pref.
    pub code_enabled: bool,
    /// The `harness-code` binary after the switch. While it is not installed nothing is registered,
    /// and the UI offers its installer (`setup_install_code`).
    pub server: DashboardSetup,
    /// When switched off: the app's own entries left in files it cannot rewrite (they have
    /// comments), which the user removes by hand. Empty otherwise.
    pub manual_removals: Vec<ManualRemoval>,
}
