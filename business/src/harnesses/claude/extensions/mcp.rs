//! Claude Code MCP servers, in Claude's precedence (the first definition of a name wins):
//!
//! 1. local: the open project's `mcpServers` in the global file (`projects[<project>]`);
//! 2. project: `<project>/.mcp.json`, when approved;
//! 3. user: the global file's top-level `mcpServers`;
//! 4. plugins: `.mcp.json` and the manifest's inline `mcpServers` of each enabled plugin, named
//!    `plugin:<plugin>:<server>`.
//!
//! A `.mcp.json` that resolves outside the project is not read. A `.mcp.json` server is approved
//! when `enabledMcpjsonServers` names it or `enableAllProjectMcpServers` is on, and turned down when `disabledMcpjsonServers` names it
//! (settings layers or the project's entry in the global file). Claude starts neither a turned-down
//! one nor one waiting for an answer, so both are listed off, after the others, and do not hide a
//! server of the same name. A name in the project entry's `disabledMcpServers` is off too.
//!
//! Entries are `{ type?: "stdio" | "http" | "sse", command, args, env, url, headers }` (no type is
//! stdio), redacted like the config view: `env` and `headers` hidden, secret-looking `args` words
//! masked. `${VAR}` references are shown as written, never resolved.

use std::path::Path;

use serde_json::Value;

use super::super::config::GlobalSections;
use super::plugins::EnabledPlugin;
use super::settings::Settings;
use crate::model::{ConfigFormat, ConfigLayer, ExtensionScope, McpServerInfo};
use crate::pi_family::mcp_config::command_words;
use crate::pi_family::{Document, parse_servers, read_json, source, string_list};
use crate::security::canonical_within;

/// The servers key, also the redaction path of every entry.
const SERVERS_KEY: &[&str] = &["mcpServers"];

/// What decides whether Claude starts the servers.
struct Switches {
    approved: Vec<String>,
    turned_down: Vec<String>,
    approve_all: bool,
    disabled: Vec<String>,
}

impl Switches {
    fn load(settings: &Settings, entry: Option<&Value>) -> Self {
        let listed = |key: &str| {
            let mut names = settings.lists(key);
            names.extend(string_list(entry.and_then(|entry| entry.get(key))));
            names
        };

        Self {
            approved: listed("enabledMcpjsonServers"),
            turned_down: listed("disabledMcpjsonServers"),
            approve_all: settings.flag("enableAllProjectMcpServers") == Some(true),
            disabled: string_list(entry.and_then(|entry| entry.get("disabledMcpServers"))),
        }
    }

    fn is_approved(&self, name: &str) -> bool {
        let named = |names: &[String]| names.iter().any(|candidate| candidate == name);

        !named(&self.turned_down) && (self.approve_all || named(&self.approved))
    }
}

/// See the module docs.
pub(super) fn mcp_servers(
    project: Option<&Path>,
    settings: &Settings,
    global: &GlobalSections,
    plugins: &[EnabledPlugin],
) -> Vec<McpServerInfo> {
    let entry = global.project.as_ref().and_then(|(_, document)| document.value());
    let switches = Switches::load(settings, entry);

    let mut ordered: Vec<McpServerInfo> = Vec::new();

    if let Some((file, document)) = &global.project {
        ordered.extend(servers(document.get("mcpServers"), &file.path, ConfigLayer::Project));
    }

    let (approved, waiting): (Vec<McpServerInfo>, Vec<McpServerInfo>) = project
        .map(project_servers)
        .unwrap_or_default()
        .into_iter()
        .partition(|server| switches.is_approved(&server.name));
    ordered.extend(approved);

    if let Some((file, document)) = &global.user {
        ordered.extend(servers(document.get("mcpServers"), &file.path, ConfigLayer::User));
    }

    for plugin in plugins {
        ordered.extend(plugin_servers(plugin));
    }

    let waiting = waiting.into_iter().map(|server| McpServerInfo {
        enabled: false,
        ..server
    });

    let mut kept: Vec<McpServerInfo> = Vec::new();
    for server in ordered.into_iter().chain(waiting) {
        if kept.iter().any(|existing| existing.name == server.name) {
            continue;
        }

        let enabled = server.enabled && !switches.disabled.contains(&server.name);
        kept.push(McpServerInfo { enabled, ..server });
    }

    kept
}

/// The servers of the project's `.mcp.json`; none when it resolves outside the project.
fn project_servers(project: &Path) -> Vec<McpServerInfo> {
    let path = project.join(".mcp.json");

    if !path.exists() || canonical_within(project, &path).is_err() {
        return Vec::new();
    }

    servers(read_json(&path).get("mcpServers"), &path, ConfigLayer::Project)
}

/// The servers of an enabled plugin, `.mcp.json` first: its `mcpServers` object, or the file's
/// top level when it has none. A `.mcp.json` that resolves outside the plugin is not read.
fn plugin_servers(plugin: &EnabledPlugin) -> Vec<McpServerInfo> {
    let layer = match plugin.scope {
        ExtensionScope::Project => ConfigLayer::Project,
        _ => ConfigLayer::User,
    };
    let mut found = Vec::new();

    let file = plugin.dir.join(".mcp.json");
    if let Ok(file) = canonical_within(&plugin.dir, &file)
        && let Document::Parsed(value) = read_json(&file)
    {
        let entries = value.get("mcpServers").unwrap_or(&value);
        found.extend(servers(Some(entries), &file, layer));
    }

    let inline = plugin.manifest.as_ref().and_then(|manifest| manifest.get("mcpServers"));
    let manifest = plugin.dir.join(".claude-plugin").join("plugin.json");
    found.extend(servers(inline, &manifest, layer));

    found
        .into_iter()
        .map(|server| McpServerInfo {
            name: format!("plugin:{}:{}", plugin.name, server.name),
            ..server
        })
        .collect()
}

/// The redacted servers of one `mcpServers` object, with secret-looking `args` words masked.
fn servers(entries: Option<&Value>, path: &Path, layer: ConfigLayer) -> Vec<McpServerInfo> {
    let source = source(path, layer, None, ConfigFormat::Json);

    parse_servers(entries, SERVERS_KEY, &source)
        .into_iter()
        .map(|parsed| McpServerInfo {
            args: command_words(parsed.info.args),
            enabled: true,
            ..parsed.info
        })
        .collect()
}
