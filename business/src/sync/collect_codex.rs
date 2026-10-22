//! Export readers for Codex: the marketplaces, plugins and MCP servers of its `config.toml`. See
//! [`super::collect`]; the records are read by the Codex adapter ([`marketplace_records`],
//! [`plugin_records`]).
//!
//! - Marketplaces: `[marketplaces.<name>]` with `source_type = "git"` whose source another machine
//!   can add the same way: a GitHub `owner/repo` or an `https://` URL. Local folders are machine
//!   paths (Codex's own bundled marketplaces included), and sources pinned to a ref or a sparse
//!   checkout, SSH URLs and unknown source types are not shared. Sources with credentials or
//!   secrets are left out.
//! - Plugins: the `name@marketplace` ids of `[plugins."<id>"]` (all installed for the user), with
//!   the version in the plugin cache (informational: the import installs the latest). Plugins of a
//!   marketplace that is left out are left out for that marketplace's reason.
//! - MCP servers: `[mcp_servers.<name>]` in the shared form: stdio (`command`, `args`) or
//!   streamable HTTP (`url`), the key names of `env` and `env_vars`, and the header names of
//!   `http_headers` and `env_http_headers` (`Authorization` for a `bearer_token_env_var`). Never a
//!   value. A server switched off (`enabled = false`) is left out, so the import never adds it
//!   switched on. The app's own servers are skipped without a row.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use super::collect::{Found, excluded_reason, safe_label, source_problem};
use super::collect_items::{MarketplacePlugin, marketplace_item, marketplace_plugins};
use super::collect_mcp::{collect_servers, endpoint_problem, keys, list, not_shared, server_fields, text};
use super::schema::{Marketplace, McpServer, Transport};
use super::secrets::exportable;
use super::validate;
use crate::env::Env;
use crate::harness::Harness;
use crate::harnesses::codex::{MarketplaceRecord, marketplace_records, plugin_records, user_config_path};
use crate::model::{ConfigFormat, HarnessId, SyncExcludedReason, SyncItemKind, SyncNote};
use crate::pi_family::read_document;

/// Fields of a Codex server that are shared (or deliberately dropped without a note).
const KNOWN_FIELDS: &[&str] = &[
    "command",
    "args",
    "url",
    "env",
    "env_vars",
    "http_headers",
    "env_http_headers",
    "bearer_token_env_var",
    "enabled",
    "cwd",
];

/// The shareable marketplaces (see the module docs).
pub(super) fn codex_marketplaces(env: &Env, found: &mut Found) {
    let harness = HarnessId::Codex;
    let kind = SyncItemKind::Marketplace;

    for record in marketplace_records(env) {
        match marketplace(&record) {
            Ok(marketplace) => found.items.push(marketplace_item(harness, marketplace)),
            Err(reason) => found.exclude(harness, kind, safe_label(Some(&record.name)), reason),
        }
    }
}

fn marketplace(record: &MarketplaceRecord) -> Result<Marketplace, SyncExcludedReason> {
    validate::marketplace_name(&record.name).map_err(excluded_reason)?;

    match record.source_type.as_deref() {
        Some("local") => return Err(SyncExcludedReason::LocalPath),
        Some("git") => {}
        _ => return Err(SyncExcludedReason::Unsupported),
    }

    if record.source.is_empty() {
        return Err(SyncExcludedReason::Invalid);
    }

    if let Some(problem) = source_problem(&record.source) {
        return Err(problem);
    }

    if record.pinned {
        return Err(SyncExcludedReason::Unsupported);
    }

    let marketplace = Marketplace {
        name: record.name.clone(),
        source: record.source.clone(),
    };
    validate::marketplace(&marketplace).map_err(excluded_reason)?;

    Ok(marketplace)
}

/// The user's plugins (see the module docs).
pub(super) fn codex_plugins(env: &Env, found: &mut Found) {
    let left_out: BTreeMap<String, SyncExcludedReason> = marketplace_records(env)
        .iter()
        .filter_map(|record| marketplace(record).err().map(|reason| (record.name.clone(), reason)))
        .collect();
    let plugins = plugin_records(env)
        .into_iter()
        .map(|record| MarketplacePlugin {
            id: record.id,
            version: record.version,
            refused: None,
        })
        .collect();

    marketplace_plugins(HarnessId::Codex, found, plugins, &left_out, validate::codex_plugin);
}

/// The servers of the user's `config.toml`; nothing when it is missing or has no `mcp_servers`
/// (see [`collect_servers`]).
pub(super) fn codex_servers(env: &Env, harness: &dyn Harness, found: &mut Found) {
    let path = user_config_path(env);
    let document = read_document(&path, ConfigFormat::Toml);

    collect_servers(
        env,
        harness,
        found,
        &path,
        &document,
        "mcp_servers",
        shared_codex_server,
    );
}

/// The shared form of one Codex server and the notes on what stays here, or why it is left out.
fn shared_codex_server(name: &str, server: &Value) -> Result<(McpServer, Vec<SyncNote>), SyncExcludedReason> {
    let fields = server_fields(name, server)?;

    if fields.get("enabled") == Some(&Value::Bool(false)) {
        return Err(SyncExcludedReason::SwitchedOff);
    }

    let (transport, command, args, url) = match (fields.contains_key("command"), fields.contains_key("url")) {
        (true, _) => (
            Transport::Stdio,
            Some(text(fields, "command")?),
            list(fields, "args")?,
            None,
        ),
        (false, true) => (Transport::Http, None, Vec::new(), Some(text(fields, "url")?)),
        (false, false) => return Err(SyncExcludedReason::Unsupported),
    };

    endpoint_problem(command.as_deref(), &args, url.as_deref())?;

    let mut env_keys = keys(fields, "env")?;
    for name in variable_names(fields, "env_vars")? {
        if !env_keys.contains(&name) {
            env_keys.push(name);
        }
    }

    let mut header_keys = keys(fields, "http_headers")?;
    for name in keys(fields, "env_http_headers")? {
        if !header_keys.contains(&name) {
            header_keys.push(name);
        }
    }

    if fields.contains_key("bearer_token_env_var")
        && !header_keys.iter().any(|key| key.eq_ignore_ascii_case("authorization"))
    {
        header_keys.push("Authorization".to_owned());
    }

    let server = McpServer {
        name: name.to_owned(),
        transport,
        command,
        args,
        url,
        env_keys,
        header_keys,
    };

    let unused: &[&str] = match transport {
        Transport::Stdio => &["url", "http_headers", "env_http_headers", "bearer_token_env_var"],
        Transport::Http | Transport::Sse => &["args", "env", "env_vars", "cwd"],
    };
    let noted: &[(&[&str], SyncNote)] = &[(&["cwd"], SyncNote::CwdNotShared)];
    let notes = not_shared(fields, &server, unused, KNOWN_FIELDS, noted);

    validate::codex_mcp_server(&server).map_err(excluded_reason)?;

    Ok((server, notes))
}

/// The names of an optional list of variable names (`env_vars`). A name that looks like a secret
/// leaves the server out.
fn variable_names(fields: &Map<String, Value>, key: &str) -> Result<Vec<String>, SyncExcludedReason> {
    let names = list(fields, key)?;

    if !names.iter().all(|name| exportable(name)) {
        return Err(SyncExcludedReason::ContainsSecret);
    }

    Ok(names)
}
