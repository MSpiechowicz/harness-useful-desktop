//! What "Share setup" ([`crate::sync`]) reads from OpenCode's config: the plugin entries of the
//! global config files, as written (export), the npm plugins of every user layer (import, to find
//! what is already there), and on OpenCode 2 the global file holding its MCP servers. On
//! OpenCode 1, MCP servers are read by `crate::sync` from the file of
//! [`crate::ecosystem::mcp_target`].

use std::fs;
use std::path::PathBuf;

pub(crate) use super::config::PluginEntry;
use super::config::{Place, Step, layers, merge_steps, plugin_entries};
use super::extensions::plugins;
use super::paths::global_config_dir;
use crate::env::Env;
use crate::harness::{ChatProtocol, Harness, OpenCodeGeneration};

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

/// The npm plugins of every user layer OpenCode 1 reads (the global files, `OPENCODE_CONFIG`,
/// `~/.opencode` and `OPENCODE_CONFIG_DIR`): package name and requested version as written, both
/// redacted. Local plugin files are left out.
pub(crate) fn user_npm_plugins(env: &Env) -> Vec<(String, Option<String>)> {
    plugins(&merge_steps(env, None))
        .into_iter()
        .filter(|plugin| plugin.path.is_none())
        .map(|plugin| (plugin.id, plugin.version))
        .collect()
}

/// The global settings file of `generation` that holds its MCP servers: `opencode.jsonc` when
/// present (a symlink counts), else `opencode.json`, in [`global_config_dir`].
pub(crate) fn global_settings_file(env: &Env, generation: OpenCodeGeneration) -> PathBuf {
    let dir = global_config_dir(env, generation);
    let jsonc = dir.join("opencode.jsonc");

    if fs::symlink_metadata(&jsonc).is_ok() {
        jsonc
    } else {
        dir.join("opencode.json")
    }
}

/// The OpenCode generation `harness` last detected: OpenCode 2 is the only harness that chats
/// over [`ChatProtocol::OpenCodeServerV2`].
pub(crate) fn generation_of(harness: &dyn Harness) -> OpenCodeGeneration {
    match harness.chat_protocol() {
        ChatProtocol::OpenCodeServerV2 => OpenCodeGeneration::V2,
        _ => OpenCodeGeneration::V1,
    }
}
