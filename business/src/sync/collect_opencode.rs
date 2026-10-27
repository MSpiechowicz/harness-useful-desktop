//! Export readers for OpenCode: the npm plugins and the MCP servers of its global config. See
//! [`super::collect`]. OpenCode 2 differs where noted.
//!
//! - Plugins: the `plugin` entries of the global config files (`config.json`, `opencode.json`,
//!   `opencode.jsonc` in the config directory). OpenCode 2: `plugin` then `plugins` of
//!   `opencode.json` and `opencode.jsonc` in its global directory (`OPENCODE_CONFIG_DIR` when
//!   set); `{package, options}` counts as an entry with options, and `-<package>` drops the
//!   package an earlier entry listed. Only npm registry specs are shared, as the package
//!   name and the version, x-range or tag the spec pins (the import installs exactly that). Local
//!   files (`file://`, paths), git, `github:` and URL specs, and `[spec, options]` entries (options
//!   may hold secrets and are not shared) are left out, named by their package or file name. A
//!   package listed in more than one file is shared as the last file lists it (OpenCode's later
//!   file wins).
//! - MCP servers: the `mcp` object of the file of [`mcp_target`] (`opencode.jsonc`, else
//!   `opencode.json`; JSON with comments). A `local` server is shared as stdio (the first word of
//!   its `command` array is the command, the rest its arguments), a `remote` one as http, with the
//!   key names of `environment` and `headers`. Their values, OAuth settings, `timeout` and other
//!   fields stay here (noted on the item). `enabled` is not shared: like OMP's and PI's, a server
//!   that is switched off is shared, and the import writes it switched on unless it needs values.
//!   Servers whose command, arguments or URL hold a secret, credentials, a local path or a
//!   `{env:…}` / `{file:…}` substitution are left out whole; the app's own servers are skipped
//!   without a row. OpenCode 2: the `mcp.servers` object and the legacy `mcp.<name>` servers of
//!   its global file ([`global_settings_file`]; a nested server wins over a legacy one of the same
//!   name, legacy entries that only set `enabled` are skipped), and `disabled` is not shared either.

use serde_json::{Map, Value};

use super::collect::{
    CollectedItem, Entry, Found, UNKNOWN_LABEL, excluded_reason, item_id, preview_row, safe_label, source_problem,
};
use super::collect_mcp::{checked_server, collect_servers, list, not_shared, text};
use super::path_args::without_machine_paths;
use super::schema::{McpServer, Plugin, Transport};
use super::validate;
use super::validate_source::{is_npm_registry_spec, split_npm_spec};
use crate::ecosystem::mcp_target;
use crate::env::Env;
use crate::harness::Harness;
use crate::harness::OpenCodeGeneration;
use crate::harnesses::opencode::{
    PluginEntry, global_plugin_entries, global_settings_file, mcp_servers as file_servers,
};
use crate::model::{ConfigFormat, HarnessId, SyncExcludedReason, SyncExportItem, SyncItemKind, SyncNote};
use crate::pi_family::{Document, read_document};

/// Server fields that are shared (or deliberately dropped without a note).
const KNOWN_FIELDS: &[&str] = &["type", "command", "url", "environment", "headers", "enabled", "oauth"];

/// [`KNOWN_FIELDS`] of OpenCode 2, which switches a server off with `disabled`.
const KNOWN_FIELDS_V2: &[&str] = &[
    "type",
    "command",
    "url",
    "environment",
    "headers",
    "enabled",
    "disabled",
    "oauth",
];

/// The npm plugins of the global config files (see the module docs).
pub(super) fn opencode_plugins(env: &Env, opencode: &dyn Harness, found: &mut Found) {
    let harness = HarnessId::OpenCode;
    let kind = SyncItemKind::Plugin;

    let generation = opencode.opencode_generation().unwrap_or_default();

    for entry in global_plugin_entries(env, generation) {
        let spec = match entry {
            PluginEntry::Spec(spec) => spec,
            PluginEntry::WithOptions(spec) => {
                let label = plugin_label(spec.as_deref().unwrap_or_default());
                found.exclude(harness, kind, label, SyncExcludedReason::Unsupported);
                continue;
            }
            PluginEntry::Remove(target) => {
                // OpenCode 2 drops the package an earlier entry listed.
                if let Ok(removed) = plugin(&target) {
                    let id = item_id(harness, kind, &removed.id);
                    found.items.retain(|item| item.preview.id != id);
                }
                continue;
            }
            PluginEntry::Other => {
                found.exclude(harness, kind, UNKNOWN_LABEL.to_owned(), SyncExcludedReason::Invalid);
                continue;
            }
        };

        let plugin = match plugin(&spec) {
            Ok(plugin) => plugin,
            Err(reason) => {
                found.exclude(harness, kind, plugin_label(&spec), reason);
                continue;
            }
        };

        // A later file wins in OpenCode's merge, so its entry replaces an earlier one.
        let id = item_id(harness, kind, &plugin.id);
        found.items.retain(|item| item.preview.id != id);

        found.items.push(CollectedItem {
            preview: SyncExportItem {
                id,
                version: plugin.version.clone(),
                ..preview_row(kind, plugin.id.clone())
            },
            entry: Entry::Plugin(plugin),
        });
    }
}

/// The shared form of an npm registry spec, or why it is left out.
fn plugin(spec: &str) -> Result<Plugin, SyncExcludedReason> {
    if spec.starts_with('.') || validate::is_local_path(spec) {
        return Err(SyncExcludedReason::LocalPath);
    }

    if let Some(problem) = source_problem(spec) {
        return Err(problem);
    }

    if !is_npm_registry_spec(spec) {
        return Err(SyncExcludedReason::Unsupported);
    }

    let (name, version) = split_npm_spec(spec);
    let plugin = Plugin {
        id: name.to_owned(),
        marketplace: None,
        version: version.map(str::to_owned),
    };
    validate::opencode_plugin(&plugin).map_err(excluded_reason)?;

    Ok(plugin)
}

/// A name for a plugin entry, never its spec: the npm package name, else the last segment of the
/// path or URL (without `#ref`, `?query`, `@ref` and `.git`); `unknown` when that is not safe.
fn plugin_label(spec: &str) -> String {
    if is_npm_registry_spec(spec) {
        let (name, _) = split_npm_spec(spec);
        return safe_label(Some(name));
    }

    let base = spec.split(['#', '?']).next().unwrap_or_default();
    let base = base.trim_end_matches(['/', '\\']);
    let last = base.rsplit(['/', '\\', ':']).next().unwrap_or_default();
    let last = last.split('@').next().unwrap_or_default();
    let last = last.strip_suffix(".git").unwrap_or(last);

    safe_label(Some(last))
}

/// The servers of the global config file; nothing when it is missing or has no `mcp`. A file that
/// cannot be read or parsed, or whose `mcp` (on OpenCode 2 also `mcp.servers`) is not an object,
/// is listed as excluded (`invalid`, labelled with its home-relative path).
pub(super) fn opencode_servers(env: &Env, harness: &dyn Harness, found: &mut Found) {
    if harness.opencode_generation() == Some(OpenCodeGeneration::V2) {
        let path = global_settings_file(env, OpenCodeGeneration::V2);
        let document = v2_servers(read_document(&path, ConfigFormat::Jsonc));

        collect_servers(env, harness, found, &path, &document, "mcp", shared_server_v2);
        return;
    }

    let Some(file) = mcp_target(env, harness) else {
        return;
    };

    let document = read_document(&file.path, ConfigFormat::Jsonc);
    collect_servers(env, harness, found, &file.path, &document, "mcp", shared_server);
}

/// The OpenCode 2 servers of a parsed file as one `mcp` object; `mcp` is `false` when the file's
/// servers are not an object, so the file is reported as invalid.
fn v2_servers(document: Document) -> Document {
    let Document::Parsed(value) = &document else {
        return document;
    };

    if value.get("mcp").is_none() {
        return document;
    }

    let servers = match file_servers(value, OpenCodeGeneration::V2) {
        Some(servers) => Value::Object(servers.into_iter().collect()),
        None => Value::Bool(false),
    };

    Document::Parsed(serde_json::json!({ "mcp": servers }))
}

/// The shared form of one OpenCode 1 server and the notes on what stays here, or why it is left
/// out.
fn shared_server(name: &str, server: &Value) -> Result<(McpServer, Vec<SyncNote>), SyncExcludedReason> {
    shared(name, server, KNOWN_FIELDS)
}

/// [`shared_server`] for OpenCode 2.
fn shared_server_v2(name: &str, server: &Value) -> Result<(McpServer, Vec<SyncNote>), SyncExcludedReason> {
    shared(name, server, KNOWN_FIELDS_V2)
}

fn shared(name: &str, server: &Value, known: &[&str]) -> Result<(McpServer, Vec<SyncNote>), SyncExcludedReason> {
    validate::mcp_name(name).map_err(excluded_reason)?;

    let fields = server.as_object().ok_or(SyncExcludedReason::Invalid)?;

    let (transport, command, args, url) = match fields.get("type").and_then(Value::as_str) {
        Some("local") => {
            let words = list(fields, "command")?;
            let (command, args) = words.split_first().ok_or(SyncExcludedReason::Invalid)?;
            (Transport::Stdio, Some(command.clone()), args.to_vec(), None)
        }
        Some("remote") => (Transport::Http, None, Vec::new(), Some(text(fields, "url")?)),
        _ => return Err(SyncExcludedReason::Unsupported),
    };

    let (args, trimmed) = without_machine_paths(args)?;
    let server = checked_server(name, fields, transport, command, args, url, "environment")?;
    let mut notes = notes(fields, &server, known);
    notes.extend(trimmed.then_some(SyncNote::PathArgsNotShared));
    validate::opencode_mcp_server(&server).map_err(excluded_reason)?;

    Ok((server, notes))
}

/// What is not shared: environment and header values, OAuth settings, and fields outside the
/// allowlist (also a `url` on a local server or a `command` on a remote one).
fn notes(fields: &Map<String, Value>, server: &McpServer, known: &[&str]) -> Vec<SyncNote> {
    let unused: &[&str] = match server.transport {
        Transport::Stdio => &["url"],
        Transport::Http | Transport::Sse => &["command"],
    };

    not_shared(fields, server, unused, known, &[(&["oauth"], SyncNote::AuthNotShared)])
}
