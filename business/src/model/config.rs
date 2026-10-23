use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ConfigField, HarnessId};

/// Which configuration layer a file belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ConfigLayer {
    /// The user's global configuration (e.g. `~/.omp/agent`, `~/.pi/agent`).
    User,
    /// Project-local configuration (e.g. `<project>/.omp`, `<project>/.pi`).
    Project,
    /// A named profile (OMP `~/.omp/profiles/<name>/agent`).
    Profile,
}

/// File format of a configuration source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ConfigFormat {
    /// YAML (`config.yml`).
    Yaml,
    /// JSON (`settings.json`, `mcp.json`, ...).
    Json,
    /// JSON with comments and trailing commas (OpenCode `opencode.jsonc`).
    Jsonc,
    /// TOML (Codex `config.toml`).
    Toml,
    /// Markdown with optional frontmatter (`SKILL.md`, prompt templates).
    Markdown,
    /// Plain text.
    Text,
}

/// Where a piece of configuration comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConfigSource {
    /// Absolute path of the file.
    pub path: String,
    /// The layer the file belongs to.
    pub layer: ConfigLayer,
    /// Profile name when `layer` is `profile`.
    pub profile: Option<String>,
    /// File format.
    pub format: ConfigFormat,
    /// True when the file exists; missing files are listed so the UI can show where config would go.
    pub exists: bool,
}

/// Severity of a [`Notice`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NoticeLevel {
    /// Neutral information.
    Info,
    /// Something the user may want to act on.
    Warning,
    /// Something failed; the view may be incomplete.
    Error,
}

/// A short, secret-free message shown alongside a view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Notice {
    /// Severity.
    pub level: NoticeLevel,
    /// Human-readable text; never contains file contents or secret values.
    pub message: String,
}

/// One configuration file rendered as a titled, redacted key/value tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConfigSection {
    /// Stable identifier within the view (e.g. `"user.settings"`).
    pub id: String,
    /// Display title (e.g. `"Settings"`).
    pub title: String,
    /// Optional explanation of what the file controls.
    pub description: Option<String>,
    /// The file the entries were read from.
    pub source: ConfigSource,
    /// Top-level entries in source order, already redacted.
    pub entries: Vec<ConfigField>,
    /// Parse error summary (location and reason only, never file content).
    pub error: Option<String>,
}

/// The readable configuration of one harness, user layer first, then profiles and project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConfigView {
    /// The harness the configuration belongs to.
    pub harness: HarnessId,
    /// Sections in display order.
    pub sections: Vec<ConfigSection>,
    /// View-level notices (e.g. untrusted project layer, skipped files).
    pub notices: Vec<Notice>,
}
