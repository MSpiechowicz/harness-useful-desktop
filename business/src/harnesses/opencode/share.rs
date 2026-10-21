//! What "Share setup" ([`crate::sync`]) reads from OpenCode's config: the plugin entries of the
//! global config files, as written (export), the npm plugins of every user layer (import, to find
//! what is already there), and the global file holding its MCP servers (the file of
//! [`crate::ecosystem::mcp_target`]).

use std::fs;
use std::path::PathBuf;

pub(crate) use super::config::PluginEntry;
use super::config::{Place, Step, layers, plugin_entries};
use super::extensions::plugins;
use super::paths::global_config_dir;
use crate::env::Env;
use crate::harness::OpenCodeGeneration;

/// The plugin entries of the global config files of `generation`, in its merge order: `plugin` of
/// `config.json`, `opencode.json` and `opencode.jsonc` on OpenCode 1; `plugin` then `plugins` of
/// `opencode.json` and `opencode.jsonc` on OpenCode 2. Files that are missing or cannot be parsed
/// contribute nothing.
pub(crate) fn global_plugin_entries(env: &Env, generation: OpenCodeGeneration) -> Vec<PluginEntry> {
    let mut entries = Vec::new();

    for step in layers(env, None, generation).steps {
        let Step::File {
            place: Place::Global,
            document,
            ..
        } = step
        else {
            continue;
        };

        if let Some(value) = document.value() {
            entries.extend(plugin_entries(value, generation));
        }
    }

    entries
}

/// The npm plugins of every user layer `generation` reads (OpenCode 1: the global files,
/// `OPENCODE_CONFIG`, `~/.opencode` and `OPENCODE_CONFIG_DIR`; OpenCode 2: `plugin` and `plugins`
/// of its global files and `OPENCODE_CONFIG`, after the removals they list): package name and
/// requested version as written, both redacted. Local plugin files are left out.
pub(crate) fn user_npm_plugins(env: &Env, generation: OpenCodeGeneration) -> Vec<(String, Option<String>)> {
    plugins(&layers(env, None, generation))
        .into_iter()
        .filter(|plugin| plugin.path.is_none())
        .map(|plugin| (plugin.id, plugin.version))
        .collect()
}

/// The global settings file of `generation` the app writes (MCP servers, plugin removal), the one
/// OpenCode itself edits in [`global_config_dir`]: on OpenCode 1 `opencode.jsonc` when present,
/// else `opencode.json`; on OpenCode 2 `opencode.json` when present, else `opencode.jsonc` when
/// present, else `opencode.json` (`resolveConfigPath` of `opencode plugin add` and `opencode mcp
/// add`, 2.1.2). A symlink counts as present, so the app refuses it rather than passing it over.
pub(crate) fn global_settings_file(env: &Env, generation: OpenCodeGeneration) -> PathBuf {
    let dir = global_config_dir(env, generation);
    let json = dir.join("opencode.json");
    let jsonc = dir.join("opencode.jsonc");
    let present = |path: &PathBuf| fs::symlink_metadata(path).is_ok();

    let preferred = match generation {
        OpenCodeGeneration::V1 => [jsonc, json.clone()],
        OpenCodeGeneration::V2 => [json.clone(), jsonc],
    };

    preferred.into_iter().find(present).unwrap_or(json)
}
