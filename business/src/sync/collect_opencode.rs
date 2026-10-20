//! Export readers for OpenCode: the npm plugins and the MCP servers of its global config. See
//! [`super::collect`].
//!
//! - Plugins: the `plugin` entries of the global config files (`config.json`, `opencode.json`,
//!   `opencode.jsonc` in the config directory). Only npm registry specs are shared, as the package
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
//!   without a row.

use serde_json::{Map, Value};

use super::collect::{
    CollectedItem, Entry, Found, UNKNOWN_LABEL, excluded_reason, item_id, preview_row, safe_label, source_problem,
};
use super::collect_mcp::{endpoint_problem, keys, list, preview, text};
use super::schema::{McpServer, Plugin, Transport};
use super::validate;
use super::validate_mcp::is_reserved_server;
use super::validate_source::{is_npm_registry_spec, split_npm_spec};
use crate::ecosystem::mcp_target;
use crate::env::Env;
use crate::harness::Harness;
use crate::harnesses::opencode::{PluginEntry, global_plugin_entries};
use crate::model::{ConfigFormat, HarnessId, SyncExcludedReason, SyncExportItem, SyncItemKind, SyncNote};
use crate::pi_family::{Document, read_document, tilde};

/// Server fields that are shared (or deliberately dropped without a note).
const KNOWN_FIELDS: &[&str] = &["type", "command", "url", "environment", "headers", "enabled", "oauth"];

/// The npm plugins of the global config files (see the module docs).
pub(super) fn opencode_plugins(env: &Env, found: &mut Found) {
    let harness = HarnessId::OpenCode;
    let kind = SyncItemKind::Plugin;

    for entry in global_plugin_entries(env) {
        let spec = match entry {
            PluginEntry::Spec(spec) => spec,
            PluginEntry::WithOptions(spec) => {
                let label = plugin_label(spec.as_deref().unwrap_or_default());
                found.exclude(harness, kind, label, SyncExcludedReason::Unsupported);
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
/// cannot be read or parsed, or whose `mcp` is not an object, is listed as excluded (`invalid`,
/// labelled with its home-relative path).
pub(super) fn opencode_servers(env: &Env, harness: &dyn Harness, found: &mut Found) {
    let Some(file) = mcp_target(env, harness) else {
        return;
    };

    let kind = SyncItemKind::McpServer;
    let document = read_document(&file.path, ConfigFormat::Jsonc);
    let servers = match &document {
        Document::Missing => return,
        Document::Invalid(_) => None,
        Document::Parsed(value) => match value.get("mcp") {
            None => return,
            Some(servers) => servers.as_object(),
        },
    };

    let Some(servers) = servers else {
        let label = safe_label(Some(&tilde(env, &file.path)));
        found.exclude(harness.id(), kind, label, SyncExcludedReason::Invalid);
        return;
    };

    for (name, server) in servers {
        if is_reserved_server(name) {
            continue;
        }

        match shared_server(name, server) {
            Ok((server, notes)) => found.items.push(CollectedItem {
                preview: preview(item_id(harness.id(), kind, name), &server, notes),
                entry: Entry::McpServer(server),
            }),
            Err(reason) => found.exclude(harness.id(), kind, safe_label(Some(name)), reason),
        }
    }
}

/// The shared form of one OpenCode server and the notes on what stays here, or why it is left out.
fn shared_server(name: &str, server: &Value) -> Result<(McpServer, Vec<SyncNote>), SyncExcludedReason> {
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

    endpoint_problem(command.as_deref(), &args, url.as_deref())?;

    let env_keys = keys(fields, "environment")?;
    let header_keys = keys(fields, "headers")?;
    let notes = notes(fields, transport, &env_keys, &header_keys);

    let server = McpServer {
        name: name.to_owned(),
        transport,
        command,
        args,
        url,
        env_keys,
        header_keys,
    };
    validate::opencode_mcp_server(&server).map_err(excluded_reason)?;

    Ok((server, notes))
}

/// What is not shared: environment and header values, OAuth settings, and fields outside the
/// allowlist (also a `url` on a local server or a `command` on a remote one).
fn notes(
    fields: &Map<String, Value>,
    transport: Transport,
    env_keys: &[String],
    header_keys: &[String],
) -> Vec<SyncNote> {
    let unused_endpoint = match transport {
        Transport::Stdio => fields.contains_key("url"),
        Transport::Http | Transport::Sse => fields.contains_key("command"),
    };
    let other = unused_endpoint || fields.keys().any(|key| !KNOWN_FIELDS.contains(&key.as_str()));

    [
        (!env_keys.is_empty(), SyncNote::EnvValuesNotShared),
        (!header_keys.is_empty(), SyncNote::HeaderValuesNotShared),
        (fields.contains_key("oauth"), SyncNote::AuthNotShared),
        (other, SyncNote::OtherFieldsNotShared),
    ]
    .into_iter()
    .filter_map(|(applies, note)| applies.then_some(note))
    .collect()
}
