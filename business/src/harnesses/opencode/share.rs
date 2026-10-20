//! What "Share setup" ([`crate::sync`]) reads from OpenCode's plugin lists: the `plugin` entries of
//! the global config files, as written (export), and the npm plugins of every user layer (import,
//! to find what is already there). MCP servers are read by `crate::sync` from the file of
//! [`crate::ecosystem::mcp_target`].

use serde_json::Value;

use super::config::{Place, Step, merge_steps};
use super::extensions::plugins;
use crate::env::Env;

/// One entry of the `plugin` list of a global config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PluginEntry {
    /// A spec string: an npm spec, a path or a URL.
    Spec(String),
    /// `[spec, options]`: an entry with options (its spec, when it is a string).
    WithOptions(Option<String>),
    /// Anything else.
    Other,
}

/// The `plugin` entries of the global config files (`config.json`, `opencode.json`,
/// `opencode.jsonc` in the config directory), in OpenCode's merge order. Files that are missing
/// or cannot be parsed contribute nothing.
pub(crate) fn global_plugin_entries(env: &Env) -> Vec<PluginEntry> {
    let mut entries = Vec::new();

    for step in merge_steps(env, None) {
        let Step::File {
            place: Place::Global,
            document,
            ..
        } = step
        else {
            continue;
        };

        let Some(items) = document.get("plugin").and_then(Value::as_array) else {
            continue;
        };

        entries.extend(items.iter().map(|item| match item {
            Value::String(spec) => PluginEntry::Spec(spec.clone()),
            Value::Array(pair) => PluginEntry::WithOptions(pair.first().and_then(Value::as_str).map(str::to_owned)),
            _ => PluginEntry::Other,
        }));
    }

    entries
}

/// The npm plugins of every user layer OpenCode reads (the global files, `OPENCODE_CONFIG`,
/// `~/.opencode` and `OPENCODE_CONFIG_DIR`): package name and requested version as written, both
/// redacted. Local plugin files are left out.
pub(crate) fn user_npm_plugins(env: &Env) -> Vec<(String, Option<String>)> {
    plugins(&merge_steps(env, None))
        .into_iter()
        .filter(|plugin| plugin.path.is_none())
        .map(|plugin| (plugin.id, plugin.version))
        .collect()
}
