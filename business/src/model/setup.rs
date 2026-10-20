//! The "Recommended setup" onboarding step (`setup_status`, the dashboard, memory and Harness
//! Skills for OpenCode installers and the MCP registration), and the "Project memory" switch
//! (`memory_set_enabled`).
//!
//! Unlike the Ecosystem tab, this step runs the companions' official installers and edits the
//! harnesses' user MCP files, but only after the user confirms each action in the UI.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ExtensionAction, HarnessId};

/// What the setup step offers on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecommendedSetup {
    /// The Harness Dashboard binary.
    pub dashboard: DashboardSetup,
    /// The Harness Memory binary (`harness-memory`), which project memory registers with the
    /// harnesses.
    pub memory: DashboardSetup,
    /// One entry per registered harness, in registry order.
    pub harnesses: Vec<HarnessSetup>,
}

/// A companion binary the setup step installs (the dashboard's or the memory server's) and how it
/// would be installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DashboardSetup {
    /// True when the binary (`harness-dashboard` or `harness-memory`) was found; `harness-memory`
    /// also has to report its version.
    pub installed: bool,
    /// Its version (`harness-dashboard version`, `harness-memory --version`), when it reported one.
    pub version: Option<String>,
    /// False when the installer does not run on this operating system.
    pub supported_on_this_os: bool,
    /// The installer command line, for display only (the app runs a fixed equivalent).
    pub command: String,
    /// What the installer changes besides placing the binary, one sentence each.
    pub side_effects: Vec<String>,
}

/// The skills and the dashboard's, the memory server's and the app's own MCP entries of one
/// harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HarnessSetup {
    /// Which harness.
    pub harness: HarnessId,
    /// True when the harness CLI was detected.
    pub installed: bool,
    /// The Harness Skills for this harness.
    pub skills: SkillsSetup,
    /// The `harness-dashboard` server in the harness's user MCP file.
    pub mcp: McpState,
    /// The `harness-memory` server in the harness's user MCP file (`setup_register_memory`).
    pub memory_mcp: McpState,
    /// The app's own `harness-desktop mcp` server (hooks for agents) in the harness's user MCP file
    /// (`setup_register_desktop_mcp`).
    pub desktop_mcp: McpState,
}

/// Whether the Harness Skills are installed for a harness, and how to install them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum SkillsSetup {
    /// Already installed.
    Installed {
        /// The installed version, when known.
        version: Option<String>,
    },
    /// Installable by running these extension actions in order (through `extension_action`).
    Install {
        /// The actions, in order.
        actions: Vec<ExtensionAction>,
    },
    /// Only a command the user runs themselves.
    Manual {
        /// The command to copy.
        command: String,
        /// A remark shown next to it.
        note: String,
    },
    /// Installable by the Harness Skills' own installer, which the app runs once the user confirms
    /// (`setup_install_opencode_skills`).
    Script {
        /// The installer as a copyable command for this OS.
        command: String,
        /// What the installer changes on this machine, one line each.
        side_effects: Vec<String>,
    },
    /// The skills do not exist for this harness.
    Unavailable {
        /// Why, for display.
        reason: String,
    },
}

/// A companion's MCP server (`harness-dashboard`, `harness-memory` or the app's own
/// `harness-desktop`) in a harness's user MCP file.
/// `file` is the file's home-relative display path (e.g. `"~/.omp/agent/mcp.json"`); contents are
/// never returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum McpState {
    /// A server runs `harness-dashboard mcp` (`harness-memory mcp`).
    Configured {
        /// The file that defines it.
        file: String,
        /// The server's name there.
        name: String,
    },
    /// No such server; registering would add it to `file`.
    Missing {
        /// The file that would be written.
        file: String,
        /// True when the file exists (registration merges into it), false when it would be created.
        exists: bool,
    },
    /// A server runs `harness-dashboard mcp` (`harness-memory mcp`) but is switched off.
    Disabled {
        /// The file that defines it.
        file: String,
        /// The server's name there.
        name: String,
    },
    /// The file cannot be changed safely; nothing was or would be written.
    Refused {
        /// The file in question.
        file: String,
        /// Why.
        reason: McpRefusal,
    },
    /// The file has comments a rewrite would lose (JSONC), so the app leaves it alone; the user
    /// pastes `snippet` into it themselves.
    ManualEdit {
        /// The file to edit.
        file: String,
        /// The server entry to add, as text ready to paste.
        snippet: String,
    },
}

/// Why an MCP file is left alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum McpRefusal {
    /// Not valid JSON.
    Unparsable,
    /// The JSON root is not an object.
    NotAnObject,
    /// The servers key (`mcpServers`, or `mcp` in an OpenCode config file) is present but not an
    /// object.
    ServersNotAnObject,
    /// The file is a symlink.
    Symlink,
    /// The path is not a regular file (a directory, a FIFO, ...).
    NotRegularFile,
    /// The file is larger than 2 MiB.
    TooLarge,
    /// The file could not be read (permissions, an I/O error, ...).
    Unreadable,
    /// Registering failed: the file or its directory could not be created, written or replaced
    /// (permissions, a full disk, ...), or the file kept changing while it was being updated.
    /// Nothing was changed.
    WriteFailed,
}

/// Outcome of running a companion installer (`setup_install_dashboard`, `setup_install_memory`).
///
/// UIs must decide success from `ok`, never from `exitCode`: `curl … | sh` exits with 0 when the
/// download failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DashboardInstallResult {
    /// True only when the installer exited with 0 and the binary (`harness-dashboard` or
    /// `harness-memory`) now resolves and reports a version.
    pub ok: bool,
    /// A short, redacted line on the outcome.
    pub summary: Option<String>,
    /// Exit code; null when the installer was killed (timeout) or ended by a signal.
    pub exit_code: Option<i32>,
    /// True when the run exceeded its time limit and was killed.
    pub timed_out: bool,
    /// True when log lines were cut or dropped (per-line or total cap).
    pub truncated: bool,
    /// The binary's version after the run, when it reported one.
    pub version: Option<String>,
    /// The installer command line, for display only.
    pub command: String,
}

/// Which output stream a log line came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InstallStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// One line of installer output, streamed while it runs (ANSI escapes stripped, redacted).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InstallLogLine {
    /// The stream.
    pub stream: InstallStream,
    /// The line, without its terminator.
    pub line: String,
}

/// Result of the "Project memory" switch (`memory_set_enabled`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemorySwitchResult {
    /// The stored `memoryEnabled` pref.
    pub memory_enabled: bool,
    /// The `harness-memory` binary after the switch. While it is not installed nothing is
    /// registered; the UI offers its installer (`setup_install_memory`).
    pub server: DashboardSetup,
    /// When switched off: the app's own entries left in files it cannot rewrite (they have
    /// comments), which the user removes by hand. Empty otherwise.
    pub manual_removals: Vec<ManualRemoval>,
}

/// An MCP server entry the user removes by hand, because the file has comments a rewrite would
/// lose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ManualRemoval {
    /// Which harness reads the file.
    pub harness: HarnessId,
    /// The file, `~`-shortened for display.
    pub file: String,
    /// The entry's name.
    pub name: String,
}
