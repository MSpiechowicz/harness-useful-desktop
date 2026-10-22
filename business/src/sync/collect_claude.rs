//! Export readers for Claude Code: its marketplaces, its user plugins and the MCP servers of its
//! user scope. See [`super::collect`]; the registries are read by the Claude adapter
//! ([`known_marketplaces`], [`plugin_records`]).
//!
//! - Marketplaces: the entries of `known_marketplaces.json` whose source another machine can add
//!   the same way: a GitHub `owner/repo` (`github`) or an `https://` URL (`git`, `url`). Folders
//!   and files (`directory`, `file`) are machine paths, and other sources (npm, `ssh` or `http` git
//!   URLs) and sources pinned to a ref, a folder inside the repository or a sparse checkout are not
//!   shared. Sources with credentials or secrets are left out.
//! - Plugins: `name@marketplace` ids installed in the user scope, with the recorded version
//!   (informational: the import installs the latest). Left out: plugins installed only for projects
//!   or by the organization, Claude's own pseudo marketplaces (`inline` and `skills-dir` are local
//!   folders, `synced` comes from the account), and plugins of a marketplace that is left out (for
//!   that marketplace's reason).
//! - MCP servers: the top-level `mcpServers` of the global file ([`mcp_target`]), in the shared
//!   form of [`shared_server`]: the transport, the command and arguments or the URL, the key names
//!   of `env` and `headers`. A server whose command, arguments or URL hold a `${…}` substitution
//!   is left out ([`validate::claude_mcp_server`]); the app's own servers are skipped without a
//!   row.

use std::collections::BTreeMap;

use serde_json::Value;

use super::collect::{Found, excluded_reason, safe_label, source_problem};
use super::collect_items::{marketplace_item, plugin_item};
use super::collect_mcp::{collect_servers, shared_server};
use super::schema::{Marketplace, McpServer, Plugin};
use super::secrets::exportable;
use super::validate;
use crate::ecosystem::mcp_target;
use crate::env::Env;
use crate::harness::Harness;
use crate::harnesses::claude::{KnownMarketplace, PluginRecord, PluginScope, known_marketplaces, plugin_records};
use crate::harnesses::plugin_registry::split_id;
use crate::model::{HarnessId, SyncExcludedReason, SyncItemKind, SyncNote};
use crate::pi_family::read_json;

/// Pseudo marketplaces Claude Code names plugins after that are local folders.
const LOCAL_MARKETPLACES: &[&str] = &["inline", "skills-dir"];

/// The pseudo marketplace of plugins synced from the Claude account.
const SYNCED_MARKETPLACE: &str = "synced";

/// The shareable marketplaces (see the module docs).
pub(super) fn claude_marketplaces(env: &Env, found: &mut Found) {
    let harness = HarnessId::Claude;
    let kind = SyncItemKind::Marketplace;

    for known in known_marketplaces(env) {
        match marketplace(&known) {
            Ok(marketplace) => found.items.push(marketplace_item(harness, marketplace)),
            Err(reason) => found.exclude(harness, kind, safe_label(Some(&known.name)), reason),
        }
    }
}

fn marketplace(known: &KnownMarketplace) -> Result<Marketplace, SyncExcludedReason> {
    validate::marketplace_name(&known.name).map_err(excluded_reason)?;

    match known.source_type.as_deref() {
        Some("directory" | "file") => return Err(SyncExcludedReason::LocalPath),
        Some("github" | "git" | "url") => {}
        _ => return Err(SyncExcludedReason::Unsupported),
    }

    if known.source.is_empty() {
        return Err(SyncExcludedReason::Invalid);
    }

    if let Some(problem) = source_problem(&known.source) {
        return Err(problem);
    }

    if known.pinned {
        return Err(SyncExcludedReason::Unsupported);
    }

    let marketplace = Marketplace {
        name: known.name.clone(),
        source: known.source.clone(),
    };
    validate::marketplace(&marketplace).map_err(excluded_reason)?;

    Ok(marketplace)
}

/// The user plugins (see the module docs).
pub(super) fn claude_plugins(env: &Env, found: &mut Found) {
    let harness = HarnessId::Claude;
    let kind = SyncItemKind::Plugin;

    let left_out: BTreeMap<String, SyncExcludedReason> = known_marketplaces(env)
        .iter()
        .filter_map(|known| marketplace(known).err().map(|reason| (known.name.clone(), reason)))
        .collect();

    for record in plugin_records(env) {
        let label = safe_label(Some(&record.id));

        match plugin(record, &left_out) {
            Ok(plugin) => found.items.push(plugin_item(harness, plugin)),
            Err(reason) => found.exclude(harness, kind, label, reason),
        }
    }
}

fn plugin(record: PluginRecord, left_out: &BTreeMap<String, SyncExcludedReason>) -> Result<Plugin, SyncExcludedReason> {
    match record.scope {
        PluginScope::User => {}
        PluginScope::Project => return Err(SyncExcludedReason::ProjectScope),
        PluginScope::Managed => return Err(SyncExcludedReason::Unsupported),
    }

    let marketplace = match split_id(&record.id) {
        (_, Some(marketplace)) => marketplace.to_owned(),
        (_, None) => return Err(SyncExcludedReason::Invalid),
    };

    if LOCAL_MARKETPLACES.contains(&marketplace.as_str()) {
        return Err(SyncExcludedReason::LocalPath);
    }

    if marketplace == SYNCED_MARKETPLACE {
        return Err(SyncExcludedReason::Unsupported);
    }

    if let Some(reason) = left_out.get(&marketplace) {
        return Err(*reason);
    }

    if !exportable(&record.id) {
        return Err(SyncExcludedReason::ContainsSecret);
    }

    let plugin = Plugin {
        id: record.id,
        marketplace: Some(marketplace),
        version: record
            .version
            .filter(|version| validate::version(Some(version)).is_ok()),
    };
    validate::claude_plugin(&plugin).map_err(excluded_reason)?;

    Ok(plugin)
}

/// The servers of the user scope; nothing when the global file is missing or has no
/// `mcpServers` (see [`collect_servers`]).
pub(super) fn claude_servers(env: &Env, harness: &dyn Harness, found: &mut Found) {
    let Some(file) = mcp_target(env, harness) else {
        return;
    };

    let document = read_json(&file.path);
    collect_servers(
        env,
        harness,
        found,
        &file.path,
        &document,
        "mcpServers",
        shared_claude_server,
    );
}

/// [`shared_server`], also refusing what Claude Code would substitute.
fn shared_claude_server(name: &str, server: &Value) -> Result<(McpServer, Vec<SyncNote>), SyncExcludedReason> {
    let (server, notes) = shared_server(name, server)?;
    validate::claude_mcp_server(&server).map_err(excluded_reason)?;

    Ok((server, notes))
}
