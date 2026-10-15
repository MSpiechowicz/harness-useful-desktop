//! What this machine already has, compared with one shared item: used by [`super::plan`] for each
//! item's status and by [`super::apply`] right before each step (so applying is idempotent).
//!
//! Everything read here stays in process: local sources, plugin versions that are not display-safe
//! and existing MCP definitions (env values included) are only compared, never returned. The only
//! local values that leave are display-safe versions and package sources that would pass import
//! validation (in [`Presence::DifferentVersion`]).

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::collect::{UNKNOWN_LABEL, is_safe_text, package_identity};
use super::schema::{Marketplace, McpServer, Package, Plugin, Transport};
use super::validate;
use crate::ecosystem::existing_server;
use crate::env::Env;
use crate::harness::HarnessPaths;
use crate::harnesses::omp::{PluginOrigin, marketplace_records, user_plugin_records};
use crate::harnesses::pi::user_package_records;
use crate::model::McpRefusal;

/// How a shared item relates to what is installed here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Presence {
    /// Not here.
    Absent,
    /// Here with the same definition.
    Present,
    /// Here in another version (`installed` is display-safe, or `unknown`).
    DifferentVersion { installed: String, shared: String },
    /// Something else with the same name is here.
    Conflict,
}

/// The marketplaces, user plugins and user packages of one harness, read from its files.
#[derive(Debug, Default)]
pub(super) struct Machine {
    /// Registered marketplaces: (name, source as written).
    marketplaces: Vec<(String, Option<String>)>,
    /// User-scope plugins: (id, version).
    plugins: Vec<(String, Option<String>)>,
    /// User package sources, as written.
    packages: Vec<String>,
}

impl Machine {
    /// Reads the registries of `paths` (missing or unreadable files contribute nothing).
    pub fn read(paths: &HarnessPaths) -> Self {
        let marketplaces = marketplace_records(paths)
            .into_iter()
            .filter_map(|record| Some((record.name?, record.source_uri)))
            .collect();

        let plugins = user_plugin_records(paths)
            .into_iter()
            .filter(|record| !matches!(record.origin, PluginOrigin::Marketplace { user_scope: false }))
            .map(|record| (record.id, record.version))
            .collect();

        let packages = user_package_records(paths)
            .into_iter()
            .map(|record| record.source)
            .collect();

        Self {
            marketplaces,
            plugins,
            packages,
        }
    }

    /// True when a marketplace named `name` is registered.
    pub fn has_marketplace(&self, name: &str) -> bool {
        self.marketplaces.iter().any(|(registered, _)| registered == name)
    }

    /// A marketplace is present when its name is registered with the same source; the same name
    /// with another source is a conflict.
    pub fn marketplace(&self, shared: &Marketplace) -> Presence {
        let Some((_, source)) = self.marketplaces.iter().find(|(name, _)| *name == shared.name) else {
            return Presence::Absent;
        };

        match source {
            Some(source) if same_source(source, &shared.source) => Presence::Present,
            _ => Presence::Conflict,
        }
    }

    /// A plugin is present when its id is installed for the user; with both versions known and
    /// different, it is installed in another version (never upgraded or downgraded).
    pub fn plugin(&self, shared: &Plugin) -> Presence {
        let Some((_, installed)) = self.plugins.iter().find(|(id, _)| *id == shared.id) else {
            return Presence::Absent;
        };

        match (installed, &shared.version) {
            (Some(installed), Some(version)) if installed != version => Presence::DifferentVersion {
                installed: display_or_unknown(installed, is_safe_text),
                shared: version.clone(),
            },
            _ => Presence::Present,
        }
    }

    /// A package is present when a user package has the same [`package_identity`] and the same
    /// source; the same identity with another source (another pin) is another version.
    pub fn package(&self, shared: &Package) -> Presence {
        let identity = package_identity(&shared.source);
        let Some(installed) = self.packages.iter().find(|source| package_identity(source) == identity) else {
            return Presence::Absent;
        };

        if *installed == shared.source {
            return Presence::Present;
        }

        Presence::DifferentVersion {
            installed: display_or_unknown(installed, |source| validate::package_source(source).is_ok()),
            shared: shared.source.clone(),
        }
    }
}

/// `value` when `safe` accepts it, else `unknown`.
fn display_or_unknown(value: &str, safe: impl Fn(&str) -> bool) -> String {
    if safe(value) {
        value.to_owned()
    } else {
        UNKNOWN_LABEL.to_owned()
    }
}

/// Marketplace sources compared without a trailing `/` or `.git`, and GitHub repositories also
/// without `https://github.com/` and case.
fn same_source(installed: &str, shared: &str) -> bool {
    normalize_source(installed) == normalize_source(shared)
}

fn normalize_source(source: &str) -> String {
    let trimmed = source.trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);

    match trimmed.strip_prefix("https://github.com/") {
        Some(repo) => repo.to_ascii_lowercase(),
        None if !trimmed.contains(':') => trimmed.to_ascii_lowercase(),
        None => trimmed.to_owned(),
    }
}

/// The user MCP file of a harness and the other files it reads servers from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct McpFiles {
    /// The file a merge writes.
    pub target: PathBuf,
    /// Files only read, to find an existing server of the same name.
    pub also_read: Vec<PathBuf>,
}

/// A server is present when a server of that name in the target file (or, failing that, in an
/// `also_read` file) has the same transport, command, arguments and URL; env, headers and other
/// fields do not count (the import writes none of them). Any other definition is a conflict. A
/// refused target file is reported as such; refused `also_read` files are ignored.
pub(super) fn mcp_presence(env: &Env, files: &McpFiles, shared: &McpServer) -> Result<Presence, McpRefusal> {
    let existing = match existing_server(env, &files.target, &shared.name)? {
        Some(existing) => Some(existing),
        None => files
            .also_read
            .iter()
            .find_map(|other| also_read(env, other, &shared.name)),
    };

    Ok(match existing {
        None => Presence::Absent,
        Some(existing) if same_server(&existing, shared) => Presence::Present,
        Some(_) => Presence::Conflict,
    })
}

fn also_read(env: &Env, path: &Path, name: &str) -> Option<Value> {
    existing_server(env, path, name).ok().flatten()
}

/// True when `existing` defines the same endpoint as `shared`.
fn same_server(existing: &Value, shared: &McpServer) -> bool {
    let Some(fields) = existing.as_object() else {
        return false;
    };

    let transport = match fields.get("type").map(Value::as_str) {
        Some(Some("stdio")) => Transport::Stdio,
        Some(Some("http" | "streamable-http")) => Transport::Http,
        Some(Some("sse")) => Transport::Sse,
        Some(_) => return false,
        None if fields.contains_key("command") => Transport::Stdio,
        None if fields.contains_key("url") => Transport::Http,
        None => return false,
    };

    if transport != shared.transport {
        return false;
    }

    let text = |key: &str| fields.get(key).and_then(Value::as_str);

    match transport {
        Transport::Stdio => {
            let args: Option<Vec<&str>> = match fields.get("args") {
                None => Some(Vec::new()),
                Some(Value::Array(args)) => args.iter().map(Value::as_str).collect(),
                Some(_) => None,
            };
            let shared_args: Vec<&str> = shared.args.iter().map(String::as_str).collect();

            text("command") == shared.command.as_deref() && args == Some(shared_args)
        }
        Transport::Http | Transport::Sse => text("url") == shared.url.as_deref(),
    }
}
