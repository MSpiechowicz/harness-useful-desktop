//! What the "Recommended setup" step reads of Claude Code's plugin registries (Claude Code
//! 2.1.304), to tell whether the Harness Skills plugin and its marketplace are there.
//!
//! - Plugins: `<config>/plugins/installed_plugins.json`, `{"version": 2, "plugins": {"<name>@<marketplace>":
//!   [{"scope": "user", "version": "…", …}]}}`. Only an entry of the user scope (or one without a
//!   scope) counts; project and local installs are someone else's.
//! - Switched off: `enabledPlugins` of the user settings (`<config>/settings.json`) maps the id to
//!   `false`.
//! - Marketplaces: `<config>/plugins/known_marketplaces.json`, an object keyed by marketplace name
//!   whose `source` object holds a `repo` (GitHub), a `url` or a `path`.
//!
//! Files are read through the shared JSON reader (the [`crate::security::SafeReader`] deny list
//! and size cap apply); a missing or unreadable file counts as empty.

use serde_json::Value;

use super::paths::config_dir;
use crate::env::Env;
use crate::pi_family::config_files::{read_json, text_field};

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

/// A registered marketplace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownMarketplace {
    pub name: String,
    /// Its GitHub `owner/repo`, URL or path, as recorded (empty when none is).
    pub source: String,
}

/// The plugins installed in the user scope, in registry order (see the module docs).
pub fn user_plugins(env: &Env) -> Vec<UserPlugin> {
    let config = config_dir(env);
    let registry = read_json(&config.join("plugins").join("installed_plugins.json"));
    let settings = read_json(&config.join("settings.json"));

    let Some(plugins) = registry.get("plugins").and_then(Value::as_object) else {
        return Vec::new();
    };

    plugins
        .iter()
        .filter_map(|(id, entries)| {
            let user = entries.as_array()?.iter().find(|entry| is_user_entry(entry))?;
            let disabled =
                settings.get("enabledPlugins").and_then(|enabled| enabled.get(id)) == Some(&Value::Bool(false));

            Some(UserPlugin {
                id: id.clone(),
                version: text_field(user, "version"),
                enabled: !disabled,
            })
        })
        .collect()
}

/// The registered marketplaces (see the module docs).
pub fn known_marketplaces(env: &Env) -> Vec<KnownMarketplace> {
    let registry = read_json(&config_dir(env).join("plugins").join("known_marketplaces.json"));

    let Some(marketplaces) = registry.value().and_then(Value::as_object) else {
        return Vec::new();
    };

    marketplaces
        .iter()
        .filter(|(_, entry)| entry.is_object())
        .map(|(name, entry)| {
            let source = entry.get("source");
            let text = ["repo", "url", "path"]
                .iter()
                .find_map(|key| source.and_then(|source| text_field(source, key)))
                .unwrap_or_default();

            KnownMarketplace {
                name: name.clone(),
                source: text,
            }
        })
        .collect()
}

/// An install record of the user scope: an object whose `scope` is `user` or missing.
fn is_user_entry(entry: &Value) -> bool {
    let scope = entry.get("scope").and_then(Value::as_str);

    entry.is_object() && scope.is_none_or(|scope| scope == "user")
}
