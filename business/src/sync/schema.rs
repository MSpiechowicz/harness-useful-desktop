//! Share file schema v1: the serde types of a bundle, its limits, and lenient per-item parsing.
//!
//! ```json
//! { "format": "harness-useful-desktop/setup", "version": 1, "app": "0.15.0", "createdAt": "2026-10-15",
//!   "harnesses": {
//!     "omp": { "marketplaces": [{ "name": "…", "source": "owner/repo" }],
//!              "plugins": [{ "id": "name@marketplace", "marketplace": "…", "version": "…" }],
//!              "mcpServers": [{ "name": "…", "transport": "stdio", "command": "…", "args": ["…"],
//!                               "envKeys": ["…"], "headerKeys": ["…"] }] },
//!     "pi": { "packages": [{ "source": "npm:pkg@1.2.0" }], "mcpServers": [] },
//!     "opencode": { "plugins": [{ "id": "@scope/name", "version": "1.2.3" }], "mcpServers": [] } } }
//! ```
//!
//! An OpenCode plugin is an npm package name with the version it installs (`opencode plugin
//! <id>@<version> -g`), never a marketplace; its servers are the `mcp` entries of the global config
//! file in the same form (`local` as stdio, `remote` as http). A Claude Code section (`claude`)
//! has OMP's shapes, with `name@marketplace` plugins only.
//!
//! Forward compatible: unknown fields anywhere are ignored (no `deny_unknown_fields`), unknown
//! harness sections and item kinds are counted, never shown. Each item is parsed and validated on
//! its own ([`parse_section`]), so one bad item is marked invalid and the rest still import.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::validate::{self, Check};
use crate::model::{HarnessId, McpTransport, SyncInvalidReason};

/// The exact `format` of a share file.
pub const FORMAT: &str = "harness-useful-desktop/setup";
/// The schema version this app writes; newer versions are refused.
pub const VERSION: u32 = 1;
/// Prefix of a share string.
pub const CODE_PREFIX: &str = "hud1:";

/// Longest share string an export returns (64 KiB); larger setups must use the file.
pub const MAX_CODE_BYTES: usize = 64 * 1024;
/// Longest pasted text (128 KiB).
pub const MAX_PASTE_BYTES: usize = 128 * 1024;
/// Largest compressed payload after base64 decoding (96 KiB).
pub const MAX_COMPRESSED_BYTES: usize = 96 * 1024;
/// Largest JSON document, decompressed or read from a file (256 KiB).
pub const MAX_JSON_BYTES: usize = 256 * 1024;
/// Most harness sections in a bundle (known and unknown).
pub const MAX_SECTIONS: usize = 16;
/// Most items in one array (one kind of one harness).
pub const MAX_ITEMS_PER_KIND: usize = 200;
/// Most items in a bundle, across every array of every section.
pub const MAX_ITEMS: usize = 500;
/// Longest text field (names, sources, ids, versions), in characters.
pub const MAX_TEXT_CHARS: usize = 512;
/// Longest MCP command, in characters.
pub const MAX_COMMAND_CHARS: usize = 256;
/// Most MCP arguments.
pub const MAX_ARGS: usize = 64;
/// Longest MCP argument, in characters.
pub const MAX_ARG_CHARS: usize = 1024;
/// Longest MCP URL, in characters.
pub const MAX_URL_CHARS: usize = 2048;
/// Most env keys, and most header keys, of one MCP server.
pub const MAX_KEYS: usize = 64;
/// Longest env or header key name, in characters.
pub const MAX_KEY_CHARS: usize = 128;

/// A whole bundle, as written by an export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bundle {
    /// Always [`FORMAT`].
    pub format: String,
    /// Always [`VERSION`] when written by this app.
    pub version: u32,
    /// The app version that wrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// The export date (`YYYY-MM-DD`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// One section per harness.
    #[serde(default)]
    pub harnesses: BTreeMap<HarnessId, HarnessSection>,
}

impl Bundle {
    /// An empty bundle of the current format and version.
    pub fn new(app: Option<String>, created_at: Option<String>) -> Self {
        Self {
            format: FORMAT.to_owned(),
            version: VERSION,
            app,
            created_at,
            harnesses: BTreeMap::new(),
        }
    }

    /// How many items it holds across every harness.
    pub fn item_count(&self) -> usize {
        self.harnesses.values().map(HarnessSection::item_count).sum()
    }
}

/// The items of one harness. Empty lists are left out when written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessSection {
    /// OMP plugin marketplaces.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marketplaces: Vec<Marketplace>,
    /// OMP and OpenCode plugins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Vec<Plugin>,
    /// PI packages.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<Package>,
    /// MCP servers of the harness's user MCP file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<McpServer>,
}

impl HarnessSection {
    /// How many items it holds.
    pub fn item_count(&self) -> usize {
        self.marketplaces.len() + self.plugins.len() + self.packages.len() + self.mcp_servers.len()
    }
}

/// A registered plugin marketplace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marketplace {
    /// Its name.
    pub name: String,
    /// Its source: a GitHub `owner/repo` or an `https://` URL.
    pub source: String,
}

/// An installed plugin (the user scope).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    /// `name@marketplace` or an npm package name (OpenCode: an npm package name only).
    pub id: String,
    /// The marketplace it comes from, when it has one (never for OpenCode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace: Option<String>,
    /// OMP: the installed version (informational: import installs the latest). OpenCode: the
    /// version, range or tag the config pins, installed as is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// An installed package (PI, global).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    /// `npm:…`, `git:…` or `https://…`, with any pin it had.
    pub source: String,
    /// The installed version (informational).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// An MCP server: its transport, command or URL, and the names of the env variables and headers
/// it needs (never their values).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    /// The key under `mcpServers`.
    pub name: String,
    /// How the harness talks to it.
    pub transport: Transport,
    /// The program (stdio).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Its arguments (stdio).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// The server URL (http/sse).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Env variable names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_keys: Vec<String>,
    /// Header names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub header_keys: Vec<String>,
}

impl McpServer {
    /// The line shown for this server: `command args…` for stdio, else the URL.
    pub(crate) fn detail(&self) -> String {
        match self.transport {
            Transport::Stdio => {
                let mut line = vec![self.command.clone().unwrap_or_default()];
                line.extend(self.args.iter().cloned());
                line.join(" ")
            }
            Transport::Http | Transport::Sse => self.url.clone().unwrap_or_default(),
        }
    }
}

/// The MCP transports a bundle may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    /// A local program over stdio.
    Stdio,
    /// Streamable HTTP.
    Http,
    /// Server-sent events.
    Sse,
}

impl From<Transport> for McpTransport {
    fn from(transport: Transport) -> Self {
        match transport {
            Transport::Stdio => McpTransport::Stdio,
            Transport::Http => McpTransport::Http,
            Transport::Sse => McpTransport::Sse,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Lenient parsing
// ---------------------------------------------------------------------------------------------

/// The item-kind keys of a harness section this app reads.
pub const KNOWN_KINDS: [&str; 4] = ["marketplaces", "plugins", "packages", "mcpServers"];

/// One array element of a received bundle, parsed and validated on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed<T> {
    /// Its position in its array, from 0.
    pub index: usize,
    /// A name that is safe to show even when the item is invalid (its `name`, `id` or `source`
    /// when that is a display-safe, non-secret string of at most [`MAX_TEXT_CHARS`]).
    pub label: Option<String>,
    /// The item, or why it was refused (the value is never echoed).
    pub item: Result<T, SyncInvalidReason>,
}

/// The known item kinds of one known harness, parsed item by item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSection {
    /// Which harness.
    pub harness: HarnessId,
    /// `marketplaces`.
    pub marketplaces: Vec<Parsed<Marketplace>>,
    /// `plugins`.
    pub plugins: Vec<Parsed<Plugin>>,
    /// `packages`.
    pub packages: Vec<Parsed<Package>>,
    /// `mcpServers`.
    pub mcp_servers: Vec<Parsed<McpServer>>,
}

impl ParsedSection {
    /// How many items were refused.
    pub fn invalid_count(&self) -> usize {
        fn invalid<T>(items: &[Parsed<T>]) -> usize {
            items.iter().filter(|parsed| parsed.item.is_err()).count()
        }

        invalid(&self.marketplaces) + invalid(&self.plugins) + invalid(&self.packages) + invalid(&self.mcp_servers)
    }
}

/// Parses the [`KNOWN_KINDS`] of a harness section, item by item. `section` must already have
/// passed the item limits (see `codec`); a non-array value of a known kind counts as no items, and
/// other keys are left to the caller.
///
/// OpenCode plugins and servers pass [`validate::opencode_plugin`] and
/// [`validate::opencode_mcp_server`], Claude Code's [`validate::claude_plugin`] and
/// [`validate::claude_mcp_server`]. Codex shares nothing yet ([`ShareSupport::NONE`]), so its items
/// pass the generic checks and are then blocked.
///
/// [`ShareSupport::NONE`]: crate::harness::ShareSupport::NONE
pub fn parse_section(harness: HarnessId, section: &Map<String, Value>) -> ParsedSection {
    let plugin: fn(&Plugin) -> Check = match harness {
        HarnessId::OpenCode => validate::opencode_plugin,
        HarnessId::Claude => validate::claude_plugin,
        HarnessId::Omp | HarnessId::Pi | HarnessId::Codex => validate::plugin,
    };
    let mcp_server: fn(&McpServer) -> Check = match harness {
        HarnessId::OpenCode => validate::opencode_mcp_server,
        HarnessId::Claude => validate::claude_mcp_server,
        HarnessId::Omp | HarnessId::Pi | HarnessId::Codex => validate::mcp_server,
    };

    ParsedSection {
        harness,
        marketplaces: parse_items(section.get("marketplaces"), "name", validate::marketplace),
        plugins: parse_items(section.get("plugins"), "id", plugin),
        packages: parse_items(section.get("packages"), "source", validate::package),
        mcp_servers: parse_items(section.get("mcpServers"), "name", mcp_server),
    }
}

fn parse_items<T: DeserializeOwned>(value: Option<&Value>, label_key: &str, check: fn(&T) -> Check) -> Vec<Parsed<T>> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };

    items
        .iter()
        .enumerate()
        .map(|(index, value)| Parsed {
            index,
            label: safe_label(value.get(label_key)),
            item: parse_item(value, check),
        })
        .collect()
}

fn parse_item<T: DeserializeOwned>(value: &Value, check: fn(&T) -> Check) -> Result<T, SyncInvalidReason> {
    if !value.is_object() {
        return Err(SyncInvalidReason::Malformed);
    }

    let item = T::deserialize(value).map_err(|_| SyncInvalidReason::Malformed)?;
    check(&item)?;

    Ok(item)
}

fn safe_label(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?;
    let safe = validate::text(text, MAX_TEXT_CHARS).is_ok() && super::secrets::exportable(text);

    (safe && !text.is_empty()).then(|| text.to_owned())
}
