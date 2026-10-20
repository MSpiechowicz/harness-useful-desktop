//! Extension mutations run through the harness CLI (`extension_action`).
//!
//! [`ExtensionAction`] is a closed whitelist: the webview never sends a command line. Each harness
//! maps an action onto its own CLI arguments (OMP `omp plugin …`, PI `pi install|remove|update`)
//! or rejects it as unsupported.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Largest captured `stdout` or `stderr` of a CLI action, in bytes (256 KiB); the rest is dropped
/// and [`CliResult::truncated`] is set.
pub const MAX_CLI_OUTPUT_BYTES: usize = 256 * 1024;

/// Where a plugin is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InstallScope {
    /// The user's global configuration.
    User,
    /// The project passed to `extension_action` (the CLI runs in that directory).
    Project,
}

/// A change to the installed extensions of a harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum ExtensionAction {
    /// Install a plugin (OMP), e.g. `name@marketplace`.
    PluginInstall {
        /// What to install, as the harness CLI accepts it.
        target: String,
        /// Installation scope.
        scope: InstallScope,
    },
    /// Uninstall a plugin (OMP).
    PluginUninstall {
        /// Installed plugin name or id.
        name: String,
    },
    /// Enable an installed plugin (OMP).
    PluginEnable {
        /// Installed plugin name or id.
        name: String,
    },
    /// Disable an installed plugin (OMP).
    PluginDisable {
        /// Installed plugin name or id.
        name: String,
    },
    /// Upgrade one plugin, or every marketplace plugin when `name` is `None` (OMP).
    PluginUpgrade {
        /// Installed plugin name or id.
        name: Option<String>,
    },
    /// Register a plugin marketplace (OMP), e.g. `owner/repo`.
    MarketplaceAdd {
        /// Marketplace source as the harness CLI accepts it.
        source: String,
    },
    /// Remove a registered marketplace (OMP).
    MarketplaceRemove {
        /// Marketplace name.
        name: String,
    },
    /// Refresh one marketplace catalog, or all when `name` is `None` (OMP).
    MarketplaceUpdate {
        /// Marketplace name.
        name: Option<String>,
    },
    /// Install a package (PI), e.g. `npm:@scope/pkg` or `git:github.com/owner/repo`.
    PackageInstall {
        /// Package source as the harness CLI accepts it.
        source: String,
    },
    /// Remove an installed package (PI).
    PackageRemove {
        /// Package source as listed by the harness.
        source: String,
    },
    /// Update one package, or all when `source` is `None` (PI).
    PackageUpdate {
        /// Package source as listed by the harness.
        source: Option<String>,
    },
}

/// Outcome of an `extension_action` CLI run (or of its dry-run preview).
///
/// UIs must decide success from `ok`, never from `exitCode`: some harness CLIs exit with code 0
/// when the action failed (OMP prints `✘ Failed to …` and still exits 0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CliResult {
    /// True only when the process exited with code 0 and the harness-specific output check found
    /// no failure (for OMP: no line starting with `✘` or containing `Failed to`). Also true for a
    /// dry run of an action the CLI cannot preview: nothing ran (`exitCode` is null) and `argv`
    /// equals `command`.
    pub ok: bool,
    /// The first failure line, or a short success line; already redacted.
    pub summary: Option<String>,
    /// Process exit code; null when the process was killed (timeout), ended by a signal, or did
    /// not run (a dry run the CLI cannot preview).
    pub exit_code: Option<i32>,
    /// Captured standard output, redacted, at most [`MAX_CLI_OUTPUT_BYTES`].
    pub stdout: String,
    /// Captured standard error, redacted, at most [`MAX_CLI_OUTPUT_BYTES`].
    pub stderr: String,
    /// True when `stdout` or `stderr` was cut at [`MAX_CLI_OUTPUT_BYTES`].
    pub truncated: bool,
    /// True when the run exceeded its time limit and was killed.
    pub timed_out: bool,
    /// The command line that ran (program name first), for display only. A dry run with a preview
    /// flag ends with that flag (OMP `--dry-run`); a dry run the CLI cannot preview equals
    /// `command`.
    pub argv: Vec<String>,
    /// The command line that applies the action once the user confirms (program name first, never
    /// the preview flag), for display only. The same for a dry run and for the real run, so a
    /// confirm dialog shows this, not `argv`. Empty, like `argv`, for a config edit.
    pub command: Vec<String>,
    /// Set when the harness applies the action by editing its config file instead of running its
    /// CLI (OpenCode plugin removal): nothing ran, `argv` and `command` are empty and `summary`
    /// says what the edit does, did, or what the user has to edit themselves. Absent for CLI runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub config_edit: Option<ConfigEdit>,
}

/// A config file edit that applies an extension action (see [`CliResult::config_edit`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConfigEdit {
    /// The file edited, or that would be or has to be edited, for display (`~/…` below the home
    /// directory).
    pub file: String,
    /// True when the app left the file alone and the user edits it (`ok` is false, `summary` says
    /// what to change): the file has comments a rewrite would lose, the entry carries options, or
    /// the plugin is defined in another file.
    pub manual_edit: bool,
}
