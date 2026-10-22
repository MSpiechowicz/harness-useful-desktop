//! What the "Recommended setup" step and "Share setup" ([`crate::sync`]) read of Claude Code's
//! plugin registries (Claude Code 2.1.304), through the readers of the extensions view (files are
//! read through [`crate::security::SafeReader`]; a missing or unreadable file counts as empty).
//!
//! - Plugins: `<config>/plugins/installed_plugins.json` (the shared version 2 registry). A plugin
//!   counts for the user when it has an install record of the user scope (or one without a scope).
//!   One the organization installed (`managed`) applies everywhere too, and one installed only for
//!   projects (`project`, `local`) is someone else's ([`PluginScope`]).
//! - Switched off: `enabledPlugins` of the user settings (`<config>/settings.json`) maps the id to
//!   `false`.
//! - Marketplaces: `<config>/plugins/known_marketplaces.json`, an object keyed by marketplace name
//!   whose `source` object has a `source` type and a `repo` (GitHub), a `url` (git, catalog URL), a
//!   `path` (folder, file) or a `package` (npm).

use serde_json::Value;

use super::extensions::{Settings, global_install, known_entries, registered, source_location};
use crate::env::Env;
use crate::pi_family::text_field;

/// A plugin installed in the user scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserPlugin {
    /// `name@marketplace`.
    pub id: String,
    /// The installed version, when recorded.
    pub version: Option<String>,
    /// False when the user settings switch it off.
    pub enabled: bool,
}

/// Where a registered plugin applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginScope {
    /// Installed for the user (scope `user`, or none).
    User,
    /// Installed by the organization (`managed`), not by the user.
    Managed,
    /// Installed only for projects (`project`, `local`).
    Project,
}

/// One plugin of the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRecord {
    /// `name@marketplace` as written.
    pub id: String,
    /// The version of the install record that decides [`PluginRecord::scope`], when recorded.
    pub version: Option<String>,
    pub scope: PluginScope,
}

/// A registered marketplace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownMarketplace {
    pub name: String,
    /// Its GitHub `owner/repo`, URL, path or npm package, as recorded (empty when none is).
    pub source: String,
    /// The `source` type (`github`, `git`, `url`, `directory`, `file`, `npm`, …), when recorded.
    pub source_type: Option<String>,
    /// True when the source names a branch or tag (`ref`), a folder inside the repository (`path`
    /// of a `github` or `git` source) or a sparse checkout, which adding it by its source alone
    /// would not repeat.
    pub pinned: bool,
}

/// Every plugin of the registry, in registry order, with the scope it applies in (see the module
/// docs). A plugin without an install record is left out.
pub fn plugin_records(env: &Env) -> Vec<PluginRecord> {
    registered(env)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|plugin| {
            let installs = &plugin.installs;
            let (install, scope) = match (global_install(installs, false), global_install(installs, true)) {
                (Some(user), _) => (user, PluginScope::User),
                (None, Some(managed)) => (managed, PluginScope::Managed),
                (None, None) => (
                    installs.iter().find(|install| install.is_object())?,
                    PluginScope::Project,
                ),
            };

            Some(PluginRecord {
                version: text_field(install, "version"),
                id: plugin.id.clone(),
                scope,
            })
        })
        .collect()
}

/// The plugins installed in the user scope, in registry order (see the module docs).
pub fn user_plugins(env: &Env) -> Vec<UserPlugin> {
    let switched = Settings::user(env).merged("enabledPlugins");

    plugin_records(env)
        .into_iter()
        .filter(|plugin| plugin.scope == PluginScope::User)
        .map(|plugin| UserPlugin {
            enabled: switched.get(&plugin.id) != Some(&Value::Bool(false)),
            id: plugin.id,
            version: plugin.version,
        })
        .collect()
}

/// The registered marketplaces (see the module docs).
pub fn known_marketplaces(env: &Env) -> Vec<KnownMarketplace> {
    known_entries(env)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, entry)| entry.is_object())
        .map(|(name, entry)| {
            let source = entry.get("source").cloned().unwrap_or(Value::Null);
            let source_type = text_field(&source, "source");
            let in_repository = matches!(source_type.as_deref(), Some("github" | "git"));
            let pinned = source.get("ref").is_some()
                || source.get("sparsePaths").is_some()
                || (in_repository && source.get("path").is_some());

            KnownMarketplace {
                name,
                source: source_location(&source).unwrap_or_default(),
                source_type,
                pinned,
            }
        })
        .collect()
}
