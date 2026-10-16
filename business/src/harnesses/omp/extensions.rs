//! OMP extensions view: plugins, marketplaces, skills, MCP servers, extension modules and themes.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::config::{McpFile, UserLayer, mcp_files, project_settings, user_settings};
use super::marketplaces::{mark_updates, marketplaces};
use super::paths::{PROJECT_DIR_NAME, omp_paths};
use super::plugins::installed_plugins;
use super::themes::BUILTIN_THEMES;
use crate::env::Env;
use crate::harness::HarnessPaths;
use crate::model::{
    ExtensionModuleInfo, ExtensionScope, ExtensionsView, HarnessId, McpServerInfo, SkillInfo, ThemeInfo,
};
use crate::pi_family::{
    Document, ParsedServer, SkillRoot, builtin_themes, dedupe_skills, extension_module, extension_module_paths,
    origin_label, parse_servers, project_ancestors, read_json, scan_skills, string_list, themes_in_dir,
};

/// OMP scans `<dir>/<skill>/SKILL.md` one level deep.
const SKILL_DEPTH: usize = 1;

const DEFAULT_DARK_THEME: &str = "titanium";
const DEFAULT_LIGHT_THEME: &str = "light";

/// Prefix of extension-module ids in the `disabledExtensions` setting.
const MODULE_ID_PREFIX: &str = "extension-module:";

/// User and project settings documents, project first (it overrides the user layer).
struct Settings {
    layers: Vec<Document>,
}

impl Settings {
    fn load(paths: &HarnessPaths) -> Self {
        let mut layers = Vec::new();
        layers.extend(project_settings(paths).map(|(_, document)| document));
        layers.push(user_settings(paths).1);

        Self { layers }
    }

    /// The nearest layer's value at `path` (e.g. `["theme", "dark"]`).
    fn value(&self, path: &[&str]) -> Option<&Value> {
        self.layers
            .iter()
            .find_map(|layer| path.iter().try_fold(layer.value()?, |value, key| value.get(*key)))
    }

    /// Every layer's string list at `key`, combined.
    fn lists(&self, key: &str) -> Vec<String> {
        self.layers
            .iter()
            .flat_map(|layer| string_list(layer.get(key)))
            .collect()
    }
}

/// See the module docs.
pub fn omp_extensions_view(env: &Env, project: Option<&Path>) -> ExtensionsView {
    let paths = omp_paths(env, project);
    let settings = Settings::load(&paths);
    let mut plugins = installed_plugins(env, &paths);
    let (marketplaces, marketplace_notices) = marketplaces(env, &paths, &plugins.plugins);
    mark_updates(&mut plugins.plugins, &marketplaces);

    let mut notices = plugins.notices;
    notices.extend(marketplace_notices);

    ExtensionsView {
        harness: HarnessId::Omp,
        skills: skills(env, &paths, project, &plugins.enabled_roots),
        plugins: plugins.plugins,
        marketplaces,
        prompt_templates: Vec::new(),
        agents: Vec::new(),
        mcp_servers: mcp_servers(&paths, project),
        themes: themes(&paths, &settings),
        packages: Vec::new(),
        extension_modules: extension_modules(env, &paths, project, &settings),
        notices,
    }
}

/// Precedence (first name wins): project `.omp/skills` from the project up to the repository
/// root, user skills, skills bundled with enabled plugins, then auto-learned managed skills.
fn skills(
    env: &Env,
    paths: &HarnessPaths,
    project: Option<&Path>,
    plugin_roots: &[(String, PathBuf)],
) -> Vec<SkillInfo> {
    let mut roots = Vec::new();

    if let Some(project) = project {
        for dir in project_ancestors(env, project) {
            let skills_dir = dir.join(PROJECT_DIR_NAME).join("skills");
            roots.push(root(
                &skills_dir,
                ExtensionScope::Project,
                origin_label(env, Some(project), &skills_dir),
            ));
        }
    }

    let user_dir = paths.agent_dir.join("skills");
    roots.push(root(
        &user_dir,
        ExtensionScope::User,
        origin_label(env, None, &user_dir),
    ));

    for (name, dir) in plugin_roots {
        roots.push(root(
            &dir.join("skills"),
            ExtensionScope::Plugin,
            format!("plugin {name}"),
        ));
    }

    let managed_dir = paths.agent_dir.join("managed-skills");
    roots.push(root(
        &managed_dir,
        ExtensionScope::User,
        origin_label(env, None, &managed_dir),
    ));

    let found = roots.iter().flat_map(|root| scan_skills(root, SKILL_DEPTH)).collect();
    dedupe_skills(found)
}

fn root(dir: &Path, scope: ExtensionScope, origin: String) -> SkillRoot {
    SkillRoot {
        dir: dir.to_path_buf(),
        scope,
        origin,
    }
}

/// OMP rules: the first file defining a name wins; a name in the user file's `disabledServers` is
/// off; `enabled: false` is off unless the name is in the user file's `enabledServers`.
fn mcp_servers(paths: &HarnessPaths, project: Option<&Path>) -> Vec<McpServerInfo> {
    let user_file = read_json(&paths.agent_dir.join("mcp.json"));
    let disabled = string_list(user_file.get("disabledServers"));
    let forced_on = string_list(user_file.get("enabledServers"));
    let user = UserLayer::of(paths);

    let mut servers: Vec<McpServerInfo> = Vec::new();

    for file in mcp_files(paths, project) {
        for parsed in servers_in(&file, &user) {
            if servers.iter().any(|existing| existing.name == parsed.info.name) {
                continue;
            }

            let name = &parsed.info.name;
            let switched_off = parsed.enabled_flag == Some(false) && !forced_on.contains(name);
            let enabled = !disabled.contains(name) && !switched_off;

            servers.push(McpServerInfo { enabled, ..parsed.info });
        }
    }

    servers
}

fn servers_in(file: &McpFile, user: &UserLayer) -> Vec<ParsedServer> {
    let document = read_json(&file.path);
    let Some(value) = document.value() else {
        return Vec::new();
    };

    let servers = file.servers_key.iter().try_fold(value, |value, key| value.get(*key));
    parse_servers(servers, file.servers_key, &file.source(user))
}

/// Built-in themes plus `<agent>/themes/*.json`; active are `theme.dark` and `theme.light`.
fn themes(paths: &HarnessPaths, settings: &Settings) -> Vec<ThemeInfo> {
    let selected = |key: &str, default: &str| {
        settings
            .value(&["theme", key])
            .and_then(Value::as_str)
            .unwrap_or(default)
            .to_owned()
    };
    let active = [
        selected("dark", DEFAULT_DARK_THEME),
        selected("light", DEFAULT_LIGHT_THEME),
    ];
    let is_active = |name: &str| active.iter().any(|theme| theme == name);

    let mut themes = builtin_themes(BUILTIN_THEMES, &is_active);
    themes.extend(themes_in_dir(
        &paths.agent_dir.join("themes"),
        ExtensionScope::User,
        &is_active,
    ));
    themes
}

/// Modules in `<agent>/extensions` and `<project>/.omp/extensions`, plus paths listed in the
/// `extensions` setting (YAML settings and legacy `settings.json`). A module is off when
/// `disabledExtensions` lists `extension-module:<name>`.
fn extension_modules(
    env: &Env,
    paths: &HarnessPaths,
    project: Option<&Path>,
    settings: &Settings,
) -> Vec<ExtensionModuleInfo> {
    let disabled: Vec<String> = settings
        .lists("disabledExtensions")
        .into_iter()
        .filter_map(|id| id.strip_prefix(MODULE_ID_PREFIX).map(str::to_owned))
        .collect();

    let mut config_dirs = Vec::new();
    if let Some(dir) = &paths.project_dir {
        config_dirs.push((dir.clone(), ExtensionScope::Project));
    }
    config_dirs.push((paths.agent_dir.clone(), ExtensionScope::User));

    let mut found: Vec<(PathBuf, ExtensionScope)> = Vec::new();

    for (dir, scope) in &config_dirs {
        found.extend(
            extension_module_paths(&dir.join("extensions"))
                .into_iter()
                .map(|path| (path, *scope)),
        );

        let legacy = read_json(&dir.join("settings.json"));
        for listed in string_list(legacy.get("extensions")) {
            found.extend(
                listed_modules(env, project, &listed)
                    .into_iter()
                    .map(|path| (path, *scope)),
            );
        }
    }

    for listed in settings.lists("extensions") {
        found.extend(
            listed_modules(env, project, &listed)
                .into_iter()
                .map(|path| (path, ExtensionScope::User)),
        );
    }

    let mut modules: Vec<ExtensionModuleInfo> = Vec::new();
    for (path, scope) in found {
        let module = extension_module(&path, scope, true);
        if modules.iter().any(|existing| existing.path == module.path) {
            continue;
        }

        let enabled = !disabled.contains(&module.name);
        modules.push(ExtensionModuleInfo { enabled, ..module });
    }

    modules
}

/// A settings-listed module path: `~` expanded, relative paths resolved against the project.
fn listed_modules(env: &Env, project: Option<&Path>, listed: &str) -> Vec<PathBuf> {
    let expanded = env.expand_home(listed);
    let path = match (expanded.is_absolute(), project) {
        (true, _) => expanded,
        (false, Some(project)) => project.join(expanded),
        (false, None) => return Vec::new(),
    };

    if path.is_dir() {
        return extension_module_paths(&path);
    }

    if path.is_file() { vec![path] } else { Vec::new() }
}
