//! The OpenCode extensions view: skills, commands, agents, plugins, MCP servers and themes, read
//! from the layers in [`super::config`] the way OpenCode 1.19.3 merges them (later layers win).
//!
//! Nothing is installed or run: npm plugins are listed by their spec, never resolved, and MCP
//! servers are never started.

mod agents;
mod mcp;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::config::{Place, Step, config_dirs, merge_steps, project_ancestors, read_config, tui_files};
use super::paths::{PROJECT_DIR_NAME, config_dir, home_config_dir};
use super::plugin_ids::file_url_drive_path;
use crate::env::Env;
use crate::model::{ExtensionScope, ExtensionsView, HarnessId, PluginInfo, PromptTemplateInfo, SkillInfo, ThemeInfo};
use crate::pi_family::frontmatter::MAX_MARKDOWN_BYTES;
use crate::pi_family::resources::walk_tree;
use crate::pi_family::{
    SkillRoot, builtin_themes, dedupe_skills, list_dir, parse_frontmatter, scan_skills, text_field, themes_in_dir,
};
use crate::security::{SafeReader, redact_text};

/// OpenCode finds `**/SKILL.md` at any depth; this caps the walk.
const SKILL_DEPTH: usize = 4;

/// Caps the `**/*.md` walk for agents and commands.
const MARKDOWN_DEPTH: usize = 4;

/// Themes shipped with OpenCode (`DEFAULT_THEMES` of its terminal UI).
const BUILTIN_THEMES: &[&str] = &[
    "aura",
    "ayu",
    "catppuccin",
    "catppuccin-frappe",
    "catppuccin-macchiato",
    "cobalt2",
    "cursor",
    "dracula",
    "everforest",
    "flexoki",
    "github",
    "gruvbox",
    "kanagawa",
    "material",
    "matrix",
    "mercury",
    "monokai",
    "nightowl",
    "nord",
    "one-dark",
    "osaka-jade",
    "opencode",
    "orng",
    "lucent-orng",
    "palenight",
    "rosepine",
    "solarized",
    "synthwave84",
    "tokyonight",
    "vesper",
    "vercel",
    "zenburn",
    "carbonfox",
];

const DEFAULT_THEME: &str = "opencode";

/// See the module docs.
pub fn extensions_view(env: &Env, project: Option<&Path>) -> ExtensionsView {
    let steps = merge_steps(env, project);

    ExtensionsView {
        harness: HarnessId::OpenCode,
        plugins: plugins(&steps),
        marketplaces: Vec::new(),
        skills: skills(env, project),
        prompt_templates: prompt_templates(&steps),
        agents: agents::agents(&steps),
        mcp_servers: mcp::mcp_servers(&steps),
        themes: themes(env, project),
        packages: Vec::new(),
        extension_modules: Vec::new(),
        notices: Vec::new(),
    }
}

/// OpenCode reads `~/.claude/skills`, `~/.agents/skills`, then `.claude/skills` and
/// `.agents/skills` from the project up to the worktree (see [`project_ancestors`]), then `skill/` and `skills/` of
/// every config directory; a later skill of the same name wins.
fn skills(env: &Env, project: Option<&Path>) -> Vec<SkillInfo> {
    let mut roots = Vec::new();
    let mut add = |dir: PathBuf, scope| roots.push(SkillRoot::labeled(env, project, dir, scope));

    for external in [".claude", ".agents"] {
        add(env.home_join(external).join("skills"), ExtensionScope::User);
    }

    if let Some(project) = project {
        for dir in project_ancestors(project) {
            for external in [".claude", ".agents"] {
                add(dir.join(external).join("skills"), walked_scope(env, &dir));
            }
        }
    }

    for dir in config_dirs(env, project) {
        for name in ["skill", "skills"] {
            add(dir.path.join(name), dir.place.scope());
        }
    }

    // `dedupe_skills` keeps the first skill of each name, so the winners go first.
    roots.reverse();

    let found = roots.iter().flat_map(|root| scan_skills(root, SKILL_DEPTH)).collect();
    dedupe_skills(found)
}

/// The scope of what the project walk finds in `dir`: the home directory holds user resources.
fn walked_scope(env: &Env, dir: &Path) -> ExtensionScope {
    if dir == env.home {
        ExtensionScope::User
    } else {
        ExtensionScope::Project
    }
}

/// One command definition: name, description, defining file and its place.
type CommandDefinition = (String, Option<String>, PathBuf, Place);

/// The `command` key of each settings file, and the Markdown files in `command/` and `commands/`
/// of each config directory (named by their path below that directory). A later definition takes
/// over the file and scope and keeps an earlier description it does not set.
fn prompt_templates(steps: &[Step]) -> Vec<PromptTemplateInfo> {
    let mut templates: Vec<PromptTemplateInfo> = Vec::new();

    for step in steps {
        let found: Vec<CommandDefinition> = match step {
            Step::File { path, place, document } => object_entries(document.get("command"))
                .map(|(name, command)| (name.clone(), description(command), path.clone(), *place))
                .collect(),

            Step::Dir(dir) => markdown_files(&dir.path, &["command", "commands"], true)
                .into_iter()
                .filter_map(|(name, file)| {
                    let fields = Value::Object(markdown_fields(&file)?);
                    Some((name, description(&fields), file, dir.place))
                })
                .collect(),
        };

        for (name, description, path, place) in found {
            let template = PromptTemplateInfo {
                name,
                description,
                argument_hint: None,
                scope: place.scope(),
                path: path.to_string_lossy().into_owned(),
            };

            match templates.iter_mut().find(|existing| existing.name == template.name) {
                Some(existing) => {
                    let description = template.description.clone().or(existing.description.take());
                    *existing = PromptTemplateInfo {
                        description,
                        ..template
                    };
                }
                None => templates.push(template),
            }
        }
    }

    templates.sort_by(|left, right| left.name.cmp(&right.name));
    templates
}

/// Plugins named in the `plugin` key of each settings file (npm specs, or paths relative to that
/// file) and the `*.ts` / `*.js` files in `plugin/` and `plugins/` of each config directory. A
/// plugin listed again (same npm package or same file) keeps its place and takes the later scope.
pub(super) fn plugins(steps: &[Step]) -> Vec<PluginInfo> {
    let mut plugins: Vec<PluginInfo> = Vec::new();

    for step in steps {
        let found: Vec<PluginInfo> = match step {
            Step::File { path, place, document } => plugin_specs(document.get("plugin"))
                .into_iter()
                .map(|spec| configured_plugin(spec, path, place.scope()))
                .collect(),

            Step::Dir(dir) => ["plugin", "plugins"]
                .iter()
                .flat_map(|name| list_dir(&dir.path.join(name)))
                .filter(|entry| entry.is_file && (entry.name.ends_with(".ts") || entry.name.ends_with(".js")))
                .map(|entry| local_plugin(&entry.path, dir.place.scope()))
                .collect(),
        };

        for plugin in found {
            match plugins.iter_mut().find(|existing| existing.id == plugin.id) {
                Some(existing) => *existing = plugin,
                None => plugins.push(plugin),
            }
        }
    }

    plugins
}

/// Every place that defines a plugin, in merge order: the plugin's id as [`plugins`] lists it, and
/// the settings file naming it or the plugin file itself.
pub(super) fn plugin_sources(steps: &[Step]) -> Vec<(String, PathBuf)> {
    let mut sources = Vec::new();

    for step in steps {
        match step {
            Step::File { path, document, .. } => sources.extend(
                plugin_specs(document.get("plugin"))
                    .into_iter()
                    .map(|spec| (plugin_entry_id(spec, path), path.clone())),
            ),

            Step::Dir(dir) => sources.extend(
                ["plugin", "plugins"]
                    .iter()
                    .flat_map(|name| list_dir(&dir.path.join(name)))
                    .filter(|entry| entry.is_file && (entry.name.ends_with(".ts") || entry.name.ends_with(".js")))
                    .map(|entry| (local_plugin(&entry.path, dir.place.scope()).id, entry.path)),
            ),
        }
    }

    sources
}

/// The id [`plugins`] lists the `plugin` entry `spec` of `config_file` under.
pub(super) fn plugin_entry_id(spec: &str, config_file: &Path) -> String {
    configured_plugin(spec, config_file, ExtensionScope::User).id
}

/// The spec strings of a `plugin` list; an entry may be `[spec, options]` (options are not shown).
fn plugin_specs(value: Option<&Value>) -> Vec<&str> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| match item {
            Value::String(spec) => Some(spec.as_str()),
            Value::Array(pair) => pair.first().and_then(Value::as_str),
            _ => None,
        })
        .collect()
}

/// A `plugin` entry: a local file for `file://`, relative and absolute paths, else an npm package.
/// `components` drops the `.` of `./plugin.ts` and, on Windows, turns `C:/x/plugin.ts` into
/// `C:\x\plugin.ts`, so the file matches its directory scan.
fn configured_plugin(spec: &str, config_file: &Path, scope: ExtensionScope) -> PluginInfo {
    let local: Option<PathBuf> = match spec.strip_prefix("file://") {
        Some(path) if cfg!(windows) => Some(Path::new(file_url_drive_path(path)).components().collect()),
        Some(path) => Some(Path::new(path).components().collect()),
        None if spec.starts_with('.') => config_file.parent().map(|dir| dir.join(spec).components().collect()),
        None if Path::new(spec).is_absolute() => Some(Path::new(spec).components().collect()),
        None => None,
    };

    if let Some(path) = local {
        return local_plugin(&path, scope);
    }

    let (name, version) = npm_package(spec);
    let name = redact_text(name);

    PluginInfo {
        id: name.clone(),
        name,
        version: version.map(redact_text),
        description: None,
        enabled: true,
        scope,
        marketplace: None,
        features: Vec::new(),
        path: None,
        latest_version: None,
        update_available: false,
    }
}

/// Package name and requested version of an npm spec (`name`, `name@1.2`, `@scope/name@^1`).
/// Other specs (git URLs, aliases) are the name as a whole.
fn npm_package(spec: &str) -> (&str, Option<&str>) {
    if spec.contains(':') {
        return (spec, None);
    }

    let name_start = usize::from(spec.starts_with('@'));
    match spec[name_start..].split_once('@') {
        Some((name, version)) if !name.is_empty() && !version.is_empty() => {
            (&spec[..name_start + name.len()], Some(version))
        }
        _ => (spec, None),
    }
}

/// A plugin file, named after its file stem.
fn local_plugin(path: &Path, scope: ExtensionScope) -> PluginInfo {
    let path = path.to_string_lossy().into_owned();
    let name = Path::new(&path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());

    PluginInfo {
        id: path.clone(),
        name,
        version: None,
        description: None,
        enabled: true,
        scope,
        marketplace: None,
        features: Vec::new(),
        path: Some(path),
        latest_version: None,
        update_available: false,
    }
}

/// Built-in themes plus `themes/*.json` in the global config directory, every project `.opencode/`
/// and `~/.opencode` (a later theme of the same name wins). The active theme is the last `theme`
/// set in a terminal UI settings file, else `opencode`.
fn themes(env: &Env, project: Option<&Path>) -> Vec<ThemeInfo> {
    let active = active_theme(env, project);
    let is_active = |name: &str| name == active;

    let mut dirs = vec![(config_dir(env), ExtensionScope::User)];
    if let Some(project) = project {
        let project_dirs = project_ancestors(project).into_iter();
        dirs.extend(project_dirs.map(|dir| (dir.join(PROJECT_DIR_NAME), walked_scope(env, &dir))));
    }
    dirs.push((home_config_dir(env), ExtensionScope::User));

    let mut themes = builtin_themes(BUILTIN_THEMES, &is_active);

    for (dir, scope) in dirs {
        for theme in themes_in_dir(&dir.join("themes"), scope, &is_active) {
            match themes.iter_mut().find(|existing| existing.name == theme.name) {
                Some(existing) => *existing = theme,
                None => themes.push(theme),
            }
        }
    }

    themes
}

/// `theme` (or `tui.theme`) of the last terminal UI settings file that sets one.
fn active_theme(env: &Env, project: Option<&Path>) -> String {
    tui_files(env, project)
        .iter()
        .filter_map(|path| {
            let document = read_config(path);
            let nested = document.get("tui").and_then(|tui| tui.get("theme"));

            document.get("theme").or(nested)?.as_str().map(str::to_owned)
        })
        .next_back()
        .unwrap_or_else(|| DEFAULT_THEME.to_owned())
}

/// The entries of an object value (none for anything else).
fn object_entries(value: Option<&Value>) -> impl Iterator<Item = (&String, &Value)> {
    value.and_then(Value::as_object).into_iter().flatten()
}

/// The `description` of a definition, with secrets in the text masked.
fn description(definition: &Value) -> Option<String> {
    text_field(definition, "description").map(|text| redact_text(&text))
}

/// `(name, path)` of the `*.md` files in `<dir>/<sub>` for each of `subdirs`, named by their path
/// below `<sub>` without the extension (`agents/review/deep.md` is `review/deep`). Subdirectories
/// are walked only when `nested`, and symlink loops are not followed (see [`walk_tree`]).
fn markdown_files(dir: &Path, subdirs: &[&str], nested: bool) -> Vec<(String, PathBuf)> {
    let depth = if nested { MARKDOWN_DEPTH } else { 1 };
    let mut found = Vec::new();

    for sub in subdirs {
        let root = dir.join(sub);

        walk_tree(&root, depth, &mut |entry| {
            if entry.is_dir {
                return true;
            }

            if entry.is_file
                && entry.name.ends_with(".md")
                && let Some(name) = markdown_name(&root, &entry.path)
            {
                found.push((name, entry.path.clone()));
            }

            false
        });
    }

    found
}

/// The path of `file` below `root` without `.md`, `/`-separated on every platform.
fn markdown_name(root: &Path, file: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = file
        .strip_prefix(root)
        .ok()?
        .components()
        .map(|part| part.as_os_str().to_str())
        .collect();

    parts?.join("/").strip_suffix(".md").map(str::to_owned)
}

/// The frontmatter fields of a Markdown definition: empty without a frontmatter block, `None` when
/// the file is unreadable or the block is not a YAML mapping (OpenCode skips those files).
fn markdown_fields(path: &Path) -> Option<Map<String, Value>> {
    let text = SafeReader::read_to_string(path, MAX_MARKDOWN_BYTES).ok()?;

    if !text.trim_start_matches('\u{feff}').starts_with("---") {
        return Some(Map::new());
    }

    match parse_frontmatter(&text)? {
        Value::Object(fields) => Some(fields),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_specs_split_into_package_and_version() {
        assert_eq!(npm_package("opencode-wakatime"), ("opencode-wakatime", None));
        assert_eq!(
            npm_package("opencode-wakatime@1.2.0"),
            ("opencode-wakatime", Some("1.2.0"))
        );
        assert_eq!(npm_package("@acme/plugin"), ("@acme/plugin", None));
        assert_eq!(npm_package("@acme/plugin@^2"), ("@acme/plugin", Some("^2")));
        assert_eq!(npm_package("github:acme/plugin"), ("github:acme/plugin", None));
    }

    #[test]
    fn plugin_lists_accept_specs_with_options() {
        let value = serde_json::json!(["a", ["b", { "apiKey": "x" }], 3, []]);

        assert_eq!(plugin_specs(Some(&value)), ["a", "b"]);
    }
}
