//! Installed Claude Code plugins: `<config>/plugins/installed_plugins.json` (the shared version 2
//! registry, see [`plugin_registry`](crate::harnesses::plugin_registry)), switched on by
//! `enabledPlugins` in the merged settings.
//!
//! A plugin installed for the user (or by the organization, `managed`) applies everywhere, one
//! installed for a project (`project`, or `local` for the user alone) only where its
//! `projectPath` names the open project. The most specific install wins: local, project, then
//! user. A plugin directory is read only when its `installPath` lies inside `<config>/plugins/cache`;
//! one outside is listed with nothing read from it. From a plugin directory the view reads its
//! manifest (`.claude-plugin/plugin.json`: name, version, description, inline `mcpServers`; only
//! when it resolves inside the plugin directory, the description redacted) here and, for enabled
//! plugins, its `skills/`, `agents/`, `commands/` and `.mcp.json` elsewhere.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::super::config::names_project;
use super::super::paths::config_dir;
use super::settings::Settings;
use crate::env::Env;
use crate::harnesses::plugin_registry::{Registered, install_path, read_registry, split_id, verified_dir};
use crate::model::{ExtensionScope, Notice, NoticeLevel, PluginInfo};
use crate::pi_family::{read_json, text_field};
use crate::security::{canonical_within, redact_text};

/// The plugin manifest below a plugin directory.
const MANIFEST: &str = ".claude-plugin/plugin.json";

/// An enabled plugin whose directory was verified, for its skills, agents, commands and servers.
#[derive(Debug, Clone)]
pub(super) struct EnabledPlugin {
    /// The plugin name, the namespace of what it contributes (`<name>:<skill>`).
    pub name: String,
    /// The canonical plugin directory.
    pub dir: PathBuf,
    /// The scope it is installed in.
    pub scope: ExtensionScope,
    /// Its manifest, when it has a readable one.
    pub manifest: Option<Value>,
}

/// The plugins view plus the enabled plugins whose directories may be read.
#[derive(Debug, Default)]
pub(super) struct Plugins {
    pub plugins: Vec<PluginInfo>,
    pub enabled: Vec<EnabledPlugin>,
    pub notices: Vec<Notice>,
}

/// The plugins folder below the config directory.
pub(super) fn plugins_dir(env: &Env) -> PathBuf {
    config_dir(env).join("plugins")
}

/// The plugins of `installed_plugins.json` with their install records (see
/// [`read_registry`]); the error is a secret-free summary.
pub(in crate::harnesses::claude) fn registered(env: &Env) -> Result<Vec<Registered>, String> {
    read_registry(&plugins_dir(env).join("installed_plugins.json"))
}

/// The install record that applies everywhere: the user's (scope `user`, or none) and, when
/// `managed`, also the organization's (`managed`).
pub(in crate::harnesses::claude) fn global_install(installs: &[Value], managed: bool) -> Option<&Value> {
    installs.iter().find(|install| {
        let scope = install.get("scope").and_then(Value::as_str);

        install.is_object() && scope.is_none_or(|scope| scope == "user" || (managed && scope == "managed"))
    })
}

/// Whether the merged `enabledPlugins` of the settings layers (`switched`) turn plugin `id` on:
/// only an explicit `true` does. The extensions view and Recommended setup share this rule.
pub(in crate::harnesses::claude) fn is_enabled(switched: &Map<String, Value>, id: &str) -> bool {
    switched.get(id) == Some(&Value::Bool(true))
}

/// See the module docs.
pub(super) fn installed(env: &Env, project: Option<&Path>, settings: &Settings) -> Plugins {
    let dir = plugins_dir(env);
    let mut found = Plugins::default();

    let registered = match registered(env) {
        Ok(registered) => registered,
        Err(summary) => {
            found.notices.push(Notice {
                level: NoticeLevel::Warning,
                message: format!("Installed plugins could not be read: {summary}"),
            });
            return found;
        }
    };

    let switched = settings.merged("enabledPlugins");
    let cache = [dir.join("cache")];

    for plugin in registered {
        let Some((install, scope)) = applicable_install(&plugin.installs, project) else {
            continue;
        };

        let (info, enabled) = plugin_info(env, &plugin.id, install, scope, &switched, &cache);
        found.enabled.extend(enabled);
        found.plugins.push(info);
    }

    found
}

/// The install record that applies to `project`, most specific first, with the scope it is shown
/// in. A plugin installed only for other projects has none.
fn applicable_install<'a>(installs: &'a [Value], project: Option<&Path>) -> Option<(&'a Value, ExtensionScope)> {
    let scoped = |wanted: &str| {
        installs.iter().find(|install| {
            let scope = install.get("scope").and_then(Value::as_str);
            let here = text_field(install, "projectPath")
                .zip(project)
                .is_some_and(|(path, project)| names_project(&path, project));

            scope == Some(wanted) && here
        })
    };

    scoped("local")
        .or_else(|| scoped("project"))
        .map(|install| (install, ExtensionScope::Project))
        .or_else(|| global_install(installs, true).map(|install| (install, ExtensionScope::User)))
}

/// The manifest of the plugin directory `dir`, when it is a JSON object that resolves inside `dir`.
fn read_manifest(dir: &Path) -> Option<Value> {
    let path = canonical_within(dir, &dir.join(MANIFEST)).ok()?;

    read_json(&path).value().cloned().filter(Value::is_object)
}

/// One plugin and, when it is enabled and its directory verified, what may be read from it.
fn plugin_info(
    env: &Env,
    id: &str,
    install: &Value,
    scope: ExtensionScope,
    switched: &Map<String, Value>,
    cache: &[PathBuf],
) -> (PluginInfo, Option<EnabledPlugin>) {
    let (id_name, marketplace) = split_id(id);

    let raw_path = install_path(env, install);
    let verified = raw_path.as_deref().and_then(|dir| verified_dir(cache, dir));
    let manifest = verified.as_deref().and_then(read_manifest);

    let field = |key: &str| manifest.as_ref().and_then(|manifest| text_field(manifest, key));
    let name = field("name").unwrap_or_else(|| id_name.to_owned());
    let enabled = is_enabled(switched, id);

    let info = PluginInfo {
        id: id.to_owned(),
        name: name.clone(),
        version: text_field(install, "version").or_else(|| field("version")),
        description: field("description").map(|text| redact_text(&text)),
        enabled,
        scope,
        marketplace: marketplace.map(str::to_owned),
        features: Vec::new(),
        path: verified
            .clone()
            .or(raw_path)
            .map(|path| path.to_string_lossy().into_owned()),
        latest_version: None,
        update_available: false,
    };

    let readable = verified.filter(|_| enabled).map(|dir| EnabledPlugin {
        name,
        dir,
        scope,
        manifest,
    });

    (info, readable)
}
