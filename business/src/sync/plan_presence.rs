//! What this machine already has, compared with one shared item: used by [`super::plan`] for each
//! item's status and by [`super::apply`] right before each step (so applying is idempotent).
//! OpenCode plugins are found by npm name in the `plugin` lists of every user config layer, its
//! servers by name in the `mcp` object of the global config file and the files OpenCode also reads,
//! and, at apply time only, in the names OpenCode reports merging from all its layers
//! ([`with_merged`]).
//!
//! Everything read here stays in process: local sources, plugin versions that are not display-safe
//! and existing MCP definitions (env values included) are only compared, never returned. The only
//! local values that leave are display-safe versions and package sources that would pass import
//! validation (in [`Presence::DifferentVersion`]).

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::Value;

use super::collect::{UNKNOWN_LABEL, is_safe_text, package_identity};
use super::schema::{Marketplace, McpServer, Package, Plugin, Transport};
use super::validate;
use crate::ecosystem::Shape;
use crate::ecosystem::mcp_merge::{existing_entry, read_only_entry};
use crate::env::Env;
use crate::harness::Harness;
use crate::harnesses::omp::{PluginOrigin, marketplace_records, user_plugin_records};
use crate::harnesses::opencode::user_npm_plugins;
use crate::harnesses::pi::user_package_records;
use crate::model::{HarnessId, McpRefusal};

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
    /// A file that could hold a server of that name cannot be read or parsed: treated as a
    /// conflict (fail closed), reported with [`crate::model::SyncNote::ConfigUnreadable`].
    Unreadable,
}

/// The marketplaces, user plugins and user packages of one harness, read from its files.
#[derive(Debug, Default)]
pub(super) struct Machine {
    /// Registered marketplaces: (name, source as written).
    marketplaces: Vec<(String, Option<String>)>,
    /// User-scope plugins: (id, version). For OpenCode: (npm name, version as written).
    plugins: Vec<(String, Option<String>)>,
    /// User package sources, as written.
    packages: Vec<String>,
}

impl Machine {
    /// Reads the registries of `harness` (missing or unreadable files contribute nothing). OpenCode
    /// has only plugins: the npm plugins of every user config layer.
    pub fn read(env: &Env, harness: &dyn Harness) -> Self {
        if harness.id() == HarnessId::OpenCode {
            return Self {
                plugins: user_npm_plugins(env),
                ..Self::default()
            };
        }

        let paths = harness.paths(env, None);

        let marketplaces = marketplace_records(&paths)
            .into_iter()
            .filter_map(|record| Some((record.name?, record.source_uri)))
            .collect();

        let plugins = user_plugin_records(&paths)
            .into_iter()
            .filter(|record| !matches!(record.origin, PluginOrigin::Marketplace { user_scope: false }))
            .map(|record| (record.id, record.version))
            .collect();

        let packages = user_package_records(&paths)
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
    /// How the files hold their servers (`mcpServers`, or OpenCode's `mcp`).
    pub shape: Shape,
}

/// A server is present when every file holding a server of that name (the target file and the
/// `also_read` files) defines the same transport, command, arguments and URL; env, headers and
/// other fields do not count (the import writes none of them). Any other definition in any of them
/// is a conflict. A refused target file is reported as such. OpenCode files are read as JSONC,
/// comments included.
///
/// `also_read` files (OMP's `.mcp.json`, OpenCode's other config layers) are never written, so a
/// symlink there is followed. One that exists but cannot be read or parsed fails closed as
/// [`Presence::Unreadable`], whatever the other files hold: the harness may merge a server of that
/// name from it (OpenCode deep-merges its layers), and a new entry could then inherit that server's
/// headers or env.
pub(super) fn mcp_presence(files: &McpFiles, shared: &McpServer) -> Result<Presence, McpRefusal> {
    let mut existing: Vec<Value> = existing_entry(&files.target, files.shape, &shared.name)?
        .into_iter()
        .collect();

    for other in &files.also_read {
        match read_only_entry(other, files.shape, &shared.name) {
            Ok(found) => existing.extend(found),
            Err(_) => return Ok(Presence::Unreadable),
        }
    }

    let same = match files.shape {
        Shape::McpServers => same_server,
        Shape::OpenCode => same_opencode_server,
    };

    if existing.is_empty() {
        return Ok(Presence::Absent);
    }

    if existing.iter().all(|entry| same(entry, shared)) {
        return Ok(Presence::Present);
    }

    Ok(Presence::Conflict)
}

/// The presence of an OpenCode server once the names OpenCode merges from all its layers are known
/// (`None`: they were not or could not be read). A server no file the app reads has, but that OpenCode merges
/// from a layer the app cannot read (or might), is a conflict: a new entry would merge with it.
pub(super) fn with_merged(presence: Presence, merged: Option<&BTreeSet<String>>, name: &str) -> Presence {
    let merged_elsewhere = merged.is_none_or(|names| names.contains(name));

    if presence == Presence::Absent && merged_elsewhere {
        return Presence::Conflict;
    }

    presence
}

/// True when the OpenCode entry `existing` defines the same endpoint as `shared`: a `local` server
/// running `[command, ...args]` for stdio, a `remote` one at the same URL for http or sse.
fn same_opencode_server(existing: &Value, shared: &McpServer) -> bool {
    let Some(fields) = existing.as_object() else {
        return false;
    };

    match (fields.get("type").and_then(Value::as_str), shared.transport) {
        (Some("local"), Transport::Stdio) => {
            let words: Option<Vec<&str>> = match fields.get("command") {
                Some(Value::Array(words)) => words.iter().map(Value::as_str).collect(),
                _ => None,
            };
            let shared_words: Vec<&str> = shared.command.iter().chain(&shared.args).map(String::as_str).collect();

            words == Some(shared_words)
        }
        (Some("remote"), Transport::Http | Transport::Sse) => {
            fields.get("url").and_then(Value::as_str) == shared.url.as_deref()
        }
        _ => false,
    }
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
