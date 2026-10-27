//! "Share setup": the extensions and MCP servers of each harness, exported as a `hud1:` string or a
//! `.json` file and imported on another machine after a review.
//!
//! Only references are shared (marketplace sources, plugin ids, package sources, MCP commands and
//! URLs, env/header key names), never secret values. The webview only ever sends item ids and a plan
//! id: Rust rebuilds every bundle from disk and keeps every import plan, so nothing the webview
//! sends can add a command. Bundles are not signed (zlib's Adler-32 only detects corruption).
//!
//! Item ids: `omp:marketplace:<name>`, `omp:plugin:<id>`, `opencode:plugin:<npm name>`,
//! `pi:package:<identity>`, `<harness>:mcp:<name>`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{HarnessId, McpTransport};

/// What a shared item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SyncItemKind {
    /// A plugin marketplace (OMP), registered with `MarketplaceAdd`.
    Marketplace,
    /// A plugin (OMP; OpenCode npm plugins), installed in the user scope.
    Plugin,
    /// A package (PI), installed globally.
    Package,
    /// An MCP server, merged into the harness's user MCP file.
    McpServer,
}

/// A remark on an item, rendered as a localized sentence by the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SyncNote {
    /// The values of the server's env variables are not shared, only their names.
    EnvValuesNotShared,
    /// The values of the server's headers are not shared, only their names.
    HeaderValuesNotShared,
    /// The server's auth/OAuth settings are not shared.
    AuthNotShared,
    /// The server's working directory (`cwd`) is not shared (it is a path on this machine).
    CwdNotShared,
    /// Arguments that held a path of this machine (and the option name before it) are not shared.
    PathArgsNotShared,
    /// Other settings of the server are not shared (only command, args, url and key names are).
    OtherFieldsNotShared,
    /// The version is informational: importing installs the latest version.
    VersionInformational,
    /// The package's resource filters (`extensions`, `skills`, `prompts`, `themes`) are not
    /// shared: importing installs it with everything it provides.
    FiltersNotShared,
    /// The server needs env or header values: it is written switched off (`"enabled": false`) and
    /// the user fills in the values and switches it on.
    WrittenDisabled,
    /// The plugin comes from a marketplace this bundle registers; unticking that marketplace
    /// unticks the plugin.
    NeedsSharedMarketplace,
    /// An OpenCode MCP server: the review only read OpenCode's config files. When the import runs
    /// (after the user confirms), the app starts OpenCode once (`opencode debug config`) to check
    /// the name against OpenCode's merged configuration, and a name found there is a conflict.
    #[serde(rename = "opencodeMergedCheck")]
    OpenCodeMergedCheck,
    /// An MCP server left alone as a conflict because a config file the harness reads servers from
    /// could not be read or parsed (or its user MCP file cannot be changed safely), so the app
    /// cannot tell what that file defines under this name. Without this note, a conflict means a
    /// server of the same name with another definition is there.
    ConfigUnreadable,
}

// ---------------------------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------------------------

/// What `sync_export_preview` found on this machine: the shareable items per harness (all ticked by
/// default) and what is left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncExportPreview {
    /// One section per installed harness that has something to share, in registry order.
    pub sections: Vec<SyncExportSection>,
    /// Items that are never shared, with the reason (never their values).
    pub excluded: Vec<SyncExcluded>,
}

/// The shareable items of one harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncExportSection {
    /// Which harness.
    pub harness: HarnessId,
    /// Its items: marketplaces, plugins, packages, then MCP servers.
    pub items: Vec<SyncExportItem>,
}

/// One shareable item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncExportItem {
    /// Stable id sent back to `sync_export` (see the module docs).
    pub id: String,
    /// What it is.
    pub kind: SyncItemKind,
    /// Name shown in the list (marketplace name, plugin id, package source, server name).
    pub label: String,
    /// What is shared besides the label: a marketplace source, a stdio command line (`command
    /// args…`) or a server URL.
    pub detail: Option<String>,
    /// Installed version, when known (informational).
    pub version: Option<String>,
    /// True for a stdio MCP server: importing it lets the other machine's harness run a program.
    pub runs_program: bool,
    /// Names of the env variables the server needs (values are never shared).
    pub env_keys: Vec<String>,
    /// Names of the headers the server needs (values are never shared).
    pub header_keys: Vec<String>,
    /// Remarks on what is not shared.
    pub notes: Vec<SyncNote>,
}

/// An item that is never shared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncExcluded {
    /// The harness it belongs to.
    pub harness: HarnessId,
    /// What it is.
    pub kind: SyncItemKind,
    /// Its name; never a source, command or value. `"unknown"` when the name itself is unsafe to show.
    pub label: String,
    /// Why it is left out.
    pub reason: SyncExcludedReason,
}

/// Why an item is left out of an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SyncExcludedReason {
    /// Its command, arguments, URL or source contains something that looks like a secret.
    ContainsSecret,
    /// Its source or URL carries credentials (`https://user:token@host`, `git:token@host`).
    Credentials,
    /// It points to a path on this machine (a local package, a local marketplace, `file:`/`link:`).
    LocalPath,
    /// It is configured for a project, not for the user.
    ProjectScope,
    /// Its source or transport cannot be shared (an unknown package kind, an unknown transport).
    Unsupported,
    /// A field would be refused on import (unsafe characters, too long, a shell as command, …).
    Invalid,
    /// More items of this kind than a bundle may hold.
    TooMany,
    /// It is switched off here (a Codex server with `enabled = false`), so another machine would
    /// get a server running that the user does not use.
    SwitchedOff,
}

/// The `hud1:` string of an export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncExported {
    /// The string to copy (`hud1:` + base64url of the zlib-compressed JSON).
    pub code: String,
    /// How many items it holds.
    pub item_count: u32,
    /// Its length in bytes.
    pub bytes: u32,
}

// ---------------------------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------------------------

/// What `sync_import_preview` reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum SyncInput {
    /// Pasted text: a `hud1:` string or the JSON of a share file.
    Text {
        /// The pasted text.
        text: String,
    },
    /// A share file the user picks in a native dialog (Rust opens the dialog and reads the file).
    File,
}

/// A reviewed import: what each shared item would do on this machine. Kept by Rust under `planId`
/// until it is applied or replaced by a newer preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncImportPlan {
    /// Id to pass to `sync_import_apply`.
    pub plan_id: String,
    /// The app version that made the bundle, when it was safe to show.
    pub app: Option<String>,
    /// When the bundle was made (`YYYY-MM-DD`), when it was safe to show.
    pub created_at: Option<String>,
    /// One section per supported harness in the bundle, in registry order.
    pub sections: Vec<SyncImportSection>,
    /// Harness sections and item kinds this app does not know; their contents are never shown.
    pub unsupported: Vec<SyncUnsupported>,
    /// How many items are `invalid` across all sections.
    pub invalid_count: u32,
}

/// The shared items of one harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncImportSection {
    /// Which harness.
    pub harness: HarnessId,
    /// True when the harness CLI was found on this machine (otherwise every item is blocked).
    pub installed: bool,
    /// Its items: marketplaces, plugins, packages, then MCP servers.
    pub items: Vec<SyncImportItem>,
}

/// One shared item and what importing it would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncImportItem {
    /// Stable id within the plan, sent back to `sync_import_apply`.
    pub id: String,
    /// What it is.
    pub kind: SyncItemKind,
    /// Name shown in the list; `"#<n>"` (its position) when the item had no displayable name.
    pub label: String,
    /// A marketplace source, a stdio command line or a server URL, when valid.
    pub detail: Option<String>,
    /// The shared version (informational; the latest version is installed).
    pub version: Option<String>,
    /// What importing would do here.
    pub status: SyncItemStatus,
    /// True when the user may tick it (`new` items, and `differentDefinition` MCP servers).
    pub selectable: bool,
    /// True when it starts ticked (`new` items, except stdio MCP servers; never a
    /// `differentDefinition` one, which replaces something).
    pub selected_by_default: bool,
    /// The exact steps that run when it is applied, in order (empty when not selectable).
    pub steps: Vec<SyncStep>,
    /// True for a stdio MCP server: the harness will run this program on this computer.
    pub runs_program: bool,
    /// Env variables the server needs; the user fills them in (nothing is written for them).
    pub env_keys: Vec<String>,
    /// Headers the server needs; the user fills them in (nothing is written for them).
    pub header_keys: Vec<String>,
    /// Remarks shown under the item.
    pub notes: Vec<SyncNote>,
    /// The id of the bundle's marketplace this plugin needs; unticking it unticks this item.
    pub depends_on: Option<String>,
}

/// What importing an item would do on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum SyncItemStatus {
    /// Not here yet: it would be installed or added.
    New,
    /// Already here with the same definition; nothing to do.
    AlreadyPresent,
    /// Installed here in another version; left alone (never upgraded or downgraded).
    DifferentVersion {
        /// The version installed here.
        installed: String,
        /// The version in the bundle.
        shared: String,
    },
    /// Something with the same name but a different definition is here, or (note
    /// `configUnreadable`) a file that could hold one cannot be read; left alone and not selectable.
    Conflict,
    /// An MCP server with this name but another definition is in a file or scope this app writes:
    /// the user may tick it to replace it. For an HTTP or SSE server at the same scheme, host and
    /// port only its URL changes and its other settings are kept (`mcpReplace`); for anything else
    /// (a stdio server, another endpoint, another transport) the whole entry is replaced by what a
    /// new add writes (`mcpMerge`), keeping none of the old one.
    DifferentDefinition,
    /// Cannot be imported on this machine.
    Blocked {
        /// Why.
        reason: SyncBlockReason,
    },
    /// The shared item failed validation; it is never applied.
    Invalid {
        /// Why.
        reason: SyncInvalidReason,
    },
}

/// Why an item cannot be imported here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SyncBlockReason {
    /// The harness is not installed on this machine.
    HarnessNotInstalled,
    /// The plugin's marketplace is neither registered here nor in the bundle.
    MarketplaceMissing,
    /// This harness has no such kind of item (e.g. packages in OMP).
    NotSupported,
}

/// Why a shared item was rejected. Values are never echoed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SyncInvalidReason {
    /// Not an object of the expected shape (a missing field, a wrong type).
    Malformed,
    /// A field is longer than allowed, or a list has too many entries.
    TooLong,
    /// A field contains control or bidirectional-override characters.
    UnsafeCharacters,
    /// A value could be read as a CLI flag or uses characters a CLI target may not contain.
    UnsafeValue,
    /// A value points to a local path (`/…`, `~…`, `..`, `file:`, `link:`, a drive letter, `\`).
    LocalPath,
    /// A source or URL carries credentials.
    Credentials,
    /// A value looks like a secret.
    ContainsSecret,
    /// The source kind is not accepted (PI: `npm:`, `git:`, `https://`; OMP marketplaces:
    /// `owner/repo` or `https://`).
    UnsupportedSource,
    /// The URL is not `http://` or `https://`.
    UnsupportedScheme,
    /// The MCP command is a shell interpreter.
    ShellCommand,
    /// A name does not have the required form (MCP server name, plugin id, marketplace name).
    BadName,
    /// An env or header key name does not have the required form.
    BadKeyName,
    /// The transport needs a field the item lacks (stdio: `command`; http/sse: `url`).
    MissingField,
}

/// One step of an import, shown before it runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum SyncStep {
    /// A harness CLI command (program name first), for display only; Rust runs a fixed equivalent.
    Cli {
        /// The command line.
        command: Vec<String>,
    },
    /// An entry merged into the harness's user MCP file (other entries are kept unchanged).
    McpMerge {
        /// The file's home-relative display path (e.g. `"~/.omp/agent/mcp.json"`).
        file: String,
        /// The exact entry written.
        server: SyncMcpPreview,
    },
    /// The managed fields of an existing entry of the same name (HTTP or SSE, same endpoint) replaced in the
    /// harness's user MCP file; every other key of the entry (env, headers, on/off switch, ...)
    /// keeps its value.
    McpReplace {
        /// The file's home-relative display path (e.g. `"~/.omp/agent/mcp.json"`).
        file: String,
        /// The fields written: `enabled` is always `true` here and is not written (the entry keeps
        /// its own switch).
        server: SyncMcpPreview,
    },
}

/// The MCP server entry an import writes: only these fields, nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncMcpPreview {
    /// The key under `mcpServers` (`mcp` in OpenCode's config file, where a stdio server is written
    /// as `{"type": "local", "command": [command, ...args], "enabled": …}` and an http/sse one as
    /// `{"type": "remote", "url": …, "enabled": …}`).
    pub name: String,
    /// `stdio`, `http` or `sse`.
    pub transport: McpTransport,
    /// The program (stdio).
    pub command: Option<String>,
    /// Its arguments (stdio).
    pub args: Vec<String>,
    /// The server URL (http/sse).
    pub url: Option<String>,
    /// False when the entry is written with `"enabled": false` (it needs env or header values).
    pub enabled: bool,
}

/// A harness section or an item kind of the bundle that this app does not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncUnsupported {
    /// The harness, for an unknown item kind of a known harness; null for an unknown harness.
    pub harness: Option<HarnessId>,
    /// The section or kind key when it is `^[A-Za-z0-9_-]{1,32}$`, otherwise `"unknown"`.
    pub name: String,
    /// How many items it holds (its contents are never shown).
    pub item_count: u32,
}
