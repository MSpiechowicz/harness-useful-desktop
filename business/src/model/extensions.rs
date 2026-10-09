use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ConfigField, ConfigSource, ConfigValue, HarnessId, Notice};

/// Where an extension is installed or defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ExtensionScope {
    /// The user's global configuration.
    User,
    /// The current project.
    Project,
    /// Shipped with the harness.
    Builtin,
    /// Contributed by an installed plugin.
    Plugin,
}

/// An optional feature of a plugin that can be toggled on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginFeature {
    /// Feature name.
    pub name: String,
    /// Whether the feature is enabled.
    pub enabled: bool,
}

/// An installed plugin (OMP).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PluginInfo {
    /// Plugin identifier, usually `name@marketplace`.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Installed version.
    pub version: Option<String>,
    /// Short description from the plugin manifest.
    pub description: Option<String>,
    /// Whether the plugin is enabled.
    pub enabled: bool,
    /// Installation scope.
    pub scope: ExtensionScope,
    /// The marketplace it was installed from.
    pub marketplace: Option<String>,
    /// Toggleable features.
    pub features: Vec<PluginFeature>,
    /// Absolute install directory.
    pub path: Option<String>,
}

/// One plugin listed in a marketplace catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MarketplaceEntry {
    /// Plugin name.
    pub name: String,
    /// Latest catalog version.
    pub version: Option<String>,
    /// Short description.
    pub description: Option<String>,
    /// True when this plugin is installed.
    pub installed: bool,
}

/// A registered plugin marketplace and its cached catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MarketplaceInfo {
    /// Marketplace name.
    pub name: String,
    /// Source kind as recorded by the harness (e.g. `"github"`, `"local"`).
    pub source_type: String,
    /// Source location with credentials masked.
    pub source: String,
    /// When the marketplace was added (ISO 8601).
    pub added_at: Option<String>,
    /// When the catalog was last refreshed (ISO 8601).
    pub updated_at: Option<String>,
    /// Catalog entries.
    pub entries: Vec<MarketplaceEntry>,
    /// Secret-free reason the catalog could not be read.
    pub error: Option<String>,
}

/// A `SKILL.md` skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SkillInfo {
    /// Skill name from frontmatter (or the directory name).
    pub name: String,
    /// Description from frontmatter.
    pub description: String,
    /// Where the skill is defined.
    pub scope: ExtensionScope,
    /// Human-readable origin (e.g. `"~/.omp/agent/skills"`, `"plugin harness-useful-skills"`).
    pub origin: String,
    /// Absolute path of the `SKILL.md` file.
    pub path: String,
}

/// A Markdown prompt template exposed as a slash command (PI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PromptTemplateInfo {
    /// Template (command) name.
    pub name: String,
    /// Description from frontmatter.
    pub description: Option<String>,
    /// Argument placeholder from frontmatter.
    pub argument_hint: Option<String>,
    /// Where the template is defined.
    pub scope: ExtensionScope,
    /// Absolute path of the template file.
    pub path: String,
}

/// How the harness talks to an MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum McpTransport {
    /// A local child process over stdio.
    Stdio,
    /// Streamable HTTP.
    Http,
    /// Server-sent events.
    Sse,
    /// Not recognised.
    Unknown,
}

/// A configured MCP server. Every value is redacted; secrets show as `secret`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct McpServerInfo {
    /// Server name (its key in `mcpServers`).
    pub name: String,
    /// Transport.
    pub transport: McpTransport,
    /// Executable for stdio servers.
    pub command: Option<String>,
    /// Arguments for stdio servers (redacted).
    pub args: Vec<ConfigValue>,
    /// Endpoint for remote servers (redacted).
    pub url: Option<ConfigValue>,
    /// Whether the harness will start the server.
    pub enabled: bool,
    /// The file that defines the server.
    pub source: ConfigSource,
    /// Remaining settings such as `env`, `headers`, `timeout` (redacted).
    pub details: Vec<ConfigField>,
}

/// A colour theme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThemeInfo {
    /// Theme name.
    pub name: String,
    /// Where the theme is defined.
    pub scope: ExtensionScope,
    /// Absolute path of the theme file (none for built-in themes).
    pub path: Option<String>,
    /// True when the theme is currently selected.
    pub active: bool,
}

/// A package that bundles extensions, skills, prompts or themes (PI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PackageInfo {
    /// Package source as configured (redacted; may be an npm spec, git URL or path).
    pub source: ConfigValue,
    /// Source kind: `"npm"`, `"git"`, `"local"` or `"unknown"`.
    pub kind: String,
    /// Where the package is configured.
    pub scope: ExtensionScope,
    /// Resource kinds the package provides (e.g. `"extensions"`, `"skills"`).
    pub resources: Vec<String>,
}

/// A code extension module loaded by the harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExtensionModuleInfo {
    /// Module name.
    pub name: String,
    /// Absolute path of the module file or directory.
    pub path: String,
    /// Where the module is configured.
    pub scope: ExtensionScope,
    /// Whether the harness loads it.
    pub enabled: bool,
}

/// Everything installed into a harness; lists the harness does not support are empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExtensionsView {
    /// The harness these extensions belong to.
    pub harness: HarnessId,
    /// Installed plugins.
    pub plugins: Vec<PluginInfo>,
    /// Registered marketplaces.
    pub marketplaces: Vec<MarketplaceInfo>,
    /// Skills from every scope.
    pub skills: Vec<SkillInfo>,
    /// Prompt templates.
    pub prompt_templates: Vec<PromptTemplateInfo>,
    /// Configured MCP servers.
    pub mcp_servers: Vec<McpServerInfo>,
    /// Available themes.
    pub themes: Vec<ThemeInfo>,
    /// Installed packages.
    pub packages: Vec<PackageInfo>,
    /// Extension modules.
    pub extension_modules: Vec<ExtensionModuleInfo>,
    /// View-level notices (e.g. unreadable plugin registry).
    pub notices: Vec<Notice>,
}
