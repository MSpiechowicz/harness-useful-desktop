//! The Harness Useful ecosystem: the companion tools shown on the Ecosystem tab (`ecosystem_status`).
//!
//! The Ecosystem tab only detects these tools and shows how to install them; its install steps are
//! copyable text and are never executed. The one exception is the "Recommended setup" step (see
//! [`super::setup`]), which runs the dashboard's installer once the user confirms it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A companion tool of the ecosystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EcosystemId {
    /// Harness Dashboard: token usage and cost across AI coding tools (`harness-dashboard`).
    Dashboard,
    /// Harness Skills: the `us-*` skills plugin for OMP.
    Skills,
    /// Harness Sidebar: live token charts and session history for OMP in a tmux sidebar.
    Sidebar,
}

/// One copyable step of an installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallStep {
    /// What the step does (e.g. `"Add the marketplace"`).
    pub label: String,
    /// The shell command to copy; shown only, never run by the app (the setup step runs its own
    /// fixed dashboard installer, not this text).
    pub command: String,
}

/// A companion tool with its detection result on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EcosystemItem {
    /// Which tool this is.
    pub id: EcosystemId,
    /// Display name (e.g. `"Harness Dashboard"`).
    pub name: String,
    /// One-line summary.
    pub tagline: String,
    /// A short paragraph on what the tool does and how it relates to the app.
    pub description: String,
    /// Source repository URL.
    pub repo_url: String,
    /// Documentation URL.
    pub docs_url: String,
    /// True when the tool was found on this machine (in any of the `via` forms).
    pub installed: bool,
    /// Installed version, when known.
    pub version: Option<String>,
    /// The newest version a cached OMP marketplace catalog offers (skills and sidebar only; the
    /// dashboard binary has no local source for it).
    pub latest_version: Option<String>,
    /// The catalog offers a newer version than the installed one.
    pub update_available: bool,
    /// How the tool was found, one entry per form, with home-relative paths (e.g.
    /// `"binary ~/.local/bin/harness-dashboard"`,
    /// `"OMP plugin harness-useful-skills@harness-useful-skills v0.2.36"`).
    pub via: Vec<String>,
    /// False when the tool does not run on this operating system (the sidebar on Windows).
    pub supported_on_this_os: bool,
    /// Installation steps for this operating system, in order; empty when unsupported.
    pub install_steps: Vec<InstallStep>,
    /// Extra remarks for the card (requirements, restarts, what the app reads).
    pub notes: Vec<String>,
}

/// Result of `ecosystem_status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EcosystemStatus {
    /// One item per [`EcosystemId`], in display order (dashboard, skills, sidebar).
    pub items: Vec<EcosystemItem>,
}

/// An installed companion and its version, as the UI knows it from `ecosystem_status`
/// (input of `ecosystem_releases`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EcosystemInstalled {
    /// Which tool.
    pub id: EcosystemId,
    /// The installed version (e.g. `"0.2.36"`); only compared, never sent anywhere.
    pub version: String,
}

/// The latest GitHub release of an installed companion (output of `ecosystem_releases`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EcosystemRelease {
    /// Which tool.
    pub id: EcosystemId,
    /// The latest release's version without a leading `v` (e.g. `"1.25.0"`).
    pub latest_version: String,
    /// The latest release is newer than the installed version.
    pub update_available: bool,
}
