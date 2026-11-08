use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{HarnessId, HarnessSummary};

/// An installed harness CLI and its version, as the UI knows it from `harness_list` (input of
/// `harness_releases`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HarnessInstalled {
    /// Which harness.
    pub harness: HarnessId,
    /// The installed version (e.g. `"2.1.2"`); only compared, never sent anywhere.
    pub version: String,
}

/// The latest published version of an installed harness CLI (output of `harness_releases`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HarnessRelease {
    /// Which harness.
    pub harness: HarnessId,
    /// The latest version without a leading `v` (e.g. `"18.18.1"`).
    pub latest_version: String,
    /// The latest version is newer than the installed one.
    pub update_available: bool,
    /// The command `harness_update` runs, for display (e.g. `pi update --self`).
    pub update_command: String,
}

/// The outcome of `harness_update`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HarnessUpdateResult {
    /// The harness's own updater ran successfully and the installed version changed.
    pub updated: bool,
    /// A fresh detection of the harness after the run.
    pub summary: HarnessSummary,
    /// Why nothing was updated, or the last line of the updater's output. Null after an update.
    pub message: Option<String>,
    /// The command that ran, for display (e.g. `claude update`).
    pub command: String,
}
