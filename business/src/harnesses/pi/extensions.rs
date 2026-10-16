//! PI extensions view: skills, prompt templates, themes, packages, extension modules and MCP.
//!
//! Locations follow the PI docs (configuration, skills, prompt templates, themes, extensions,
//! MCP). Project resources are listed even though PI loads them only for trusted projects; the
//! view carries the trust notice.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::config::{TRUST_NOTICE, info};
use super::layers::{Layer, Listed, included, layers, listed};
use super::packages::packages;
use super::pi_paths;
use crate::env::Env;
use crate::harness::HarnessPaths;
use crate::model::{
    ConfigFormat, ConfigLayer, ExtensionModuleInfo, ExtensionScope, ExtensionsView, HarnessId, McpServerInfo,
    PromptTemplateInfo, SkillInfo, ThemeInfo,
};
use crate::pi_family::{
    SKILL_FILE, SkillRoot, builtin_themes, dedupe_skills, extension_module, extension_module_paths, list_dir,
    load_skill, origin_label, parse_servers, project_ancestors, read_frontmatter, read_json, scan_skills, source,
    text_field, themes_in_dir,
};

/// PI discovers skill directories recursively; this caps the walk.
const SKILL_DEPTH: usize = 4;

/// Themes shipped with PI.
const BUILTIN_THEMES: &[&str] = &["system", "dark", "light"];

const DEFAULT_THEME: &str = "system";

/// See the module docs.
pub fn pi_extensions_view(env: &Env, project: Option<&Path>) -> ExtensionsView {
    let paths = pi_paths(env, project);
    let layers = layers(&paths);

    let notices = match project {
        Some(_) => vec![info(TRUST_NOTICE)],
        None => Vec::new(),
    };

    ExtensionsView {
        harness: HarnessId::Pi,
        plugins: Vec::new(),
        marketplaces: Vec::new(),
        skills: skills(env, &paths, project, &layers),
        prompt_templates: prompt_templates(env, &paths, &layers),
        agents: Vec::new(),
        mcp_servers: mcp_servers(&paths),
        themes: themes(env, &paths, &layers),
        packages: packages(env, &layers),
        extension_modules: extension_modules(env, &paths, &layers),
        notices,
    }
}

/// Precedence (first name wins): project `.pi/skills`, project `.agents/skills` up to the
/// repository root, user skills, `~/.agents/skills`, then paths listed in settings.
fn skills(env: &Env, paths: &HarnessPaths, project: Option<&Path>, layers: &[Layer]) -> Vec<SkillInfo> {
    let mut roots = Vec::new();
    let mut add = |dir: PathBuf, scope| roots.push(SkillRoot::labeled(env, project, dir, scope));

    if let (Some(project), Some(dir)) = (project, &paths.project_dir) {
        add(dir.join("skills"), ExtensionScope::Project);

        for ancestor in project_ancestors(env, project) {
            add(ancestor.join(".agents").join("skills"), ExtensionScope::Project);
        }
    }

    add(paths.agent_dir.join("skills"), ExtensionScope::User);
    add(env.home_join(".agents/skills"), ExtensionScope::User);

    let mut found: Vec<SkillInfo> = roots.iter().flat_map(|root| scan_skills(root, SKILL_DEPTH)).collect();

    for layer in layers {
        for path in included(env, layer, "skills") {
            found.extend(listed_skills(env, project, layer.scope, &path));
        }
    }

    dedupe_skills(found)
}

/// A settings-listed skill directory, skill directory or `SKILL.md` file.
fn listed_skills(env: &Env, project: Option<&Path>, scope: ExtensionScope, path: &Path) -> Vec<SkillInfo> {
    let is_skill_file = path.file_name().is_some_and(|name| name == SKILL_FILE);
    let skill_file = if is_skill_file {
        path.to_path_buf()
    } else {
        path.join(SKILL_FILE)
    };
    let skill_dir = skill_file.parent().unwrap_or(path);

    let root = SkillRoot {
        dir: path.to_path_buf(),
        scope,
        origin: origin_label(env, project, skill_dir),
    };

    if skill_file.is_file() {
        let dir_name = skill_dir.file_name().map(|name| name.to_string_lossy().into_owned());
        return load_skill(&root, &dir_name.unwrap_or_default(), &skill_file)
            .into_iter()
            .collect();
    }

    scan_skills(&root, SKILL_DEPTH)
}

/// Direct `*.md` children of `.pi/prompts` and `<agent>/prompts`, plus listed files/directories.
fn prompt_templates(env: &Env, paths: &HarnessPaths, layers: &[Layer]) -> Vec<PromptTemplateInfo> {
    let mut templates = Vec::new();

    for layer in layers {
        let conventional = match layer.scope {
            ExtensionScope::Project => paths.project_dir.as_ref().map(|dir| dir.join("prompts")),
            _ => Some(paths.agent_dir.join("prompts")),
        };

        let dirs = conventional.into_iter().chain(included(env, layer, "prompts"));
        for path in dirs {
            templates.extend(markdown_files(&path).iter().map(|file| template(file, layer.scope)));
        }
    }

    templates
}

/// `path` itself when it is a Markdown file, else its direct `*.md` children.
fn markdown_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }

    list_dir(path)
        .into_iter()
        .filter(|entry| entry.is_file && entry.name.ends_with(".md"))
        .map(|entry| entry.path)
        .collect()
}

/// The template named after its file; description and hint from frontmatter (the body is never
/// read into the view).
fn template(path: &Path, scope: ExtensionScope) -> PromptTemplateInfo {
    let frontmatter = read_frontmatter(path).unwrap_or(Value::Null);

    PromptTemplateInfo {
        name: path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default(),
        description: text_field(&frontmatter, "description"),
        argument_hint: text_field(&frontmatter, "argument-hint"),
        scope,
        path: path.to_string_lossy().into_owned(),
    }
}

/// Built-ins, `<agent>/themes`, `.pi/themes` and listed theme paths. The `theme` setting (project
/// first) selects one theme or a `light/dark` pair; the default is `system`.
fn themes(env: &Env, paths: &HarnessPaths, layers: &[Layer]) -> Vec<ThemeInfo> {
    let setting = layers
        .iter()
        .find_map(|layer| layer.settings.get("theme").and_then(Value::as_str))
        .unwrap_or(DEFAULT_THEME)
        .to_owned();
    let is_active = |name: &str| setting.split('/').any(|selected| selected == name);

    let mut themes = builtin_themes(BUILTIN_THEMES, &is_active);
    themes.extend(themes_in_dir(
        &paths.agent_dir.join("themes"),
        ExtensionScope::User,
        &is_active,
    ));

    if let Some(dir) = &paths.project_dir {
        themes.extend(themes_in_dir(&dir.join("themes"), ExtensionScope::Project, &is_active));
    }

    for layer in layers {
        for path in included(env, layer, "themes") {
            themes.extend(themes_in_dir(&path, layer.scope, &is_active));
        }
    }

    themes
}

/// Modules in `.pi/extensions` and `<agent>/extensions` plus listed paths; `-path` entries
/// switch a module off.
fn extension_modules(env: &Env, paths: &HarnessPaths, layers: &[Layer]) -> Vec<ExtensionModuleInfo> {
    let mut found: Vec<(PathBuf, ExtensionScope)> = Vec::new();
    let mut excluded: Vec<PathBuf> = Vec::new();

    for layer in layers {
        let conventional = match layer.scope {
            ExtensionScope::Project => paths.project_dir.as_ref().map(|dir| dir.join("extensions")),
            _ => Some(paths.agent_dir.join("extensions")),
        };

        if let Some(dir) = conventional {
            found.extend(extension_module_paths(&dir).into_iter().map(|path| (path, layer.scope)));
        }

        for entry in listed(env, layer, "extensions") {
            match entry {
                Listed::Include(path) if path.is_dir() => {
                    found.extend(
                        extension_module_paths(&path)
                            .into_iter()
                            .map(|path| (path, layer.scope)),
                    );
                }
                Listed::Include(path) if path.is_file() => found.push((path, layer.scope)),
                Listed::Include(_) => {}
                Listed::Exclude(path) => excluded.push(path),
            }
        }
    }

    let mut modules: Vec<ExtensionModuleInfo> = Vec::new();
    for (path, scope) in found {
        let enabled = !excluded.contains(&path);
        let module = extension_module(&path, scope, enabled);

        if !modules.iter().any(|existing| existing.path == module.path) {
            modules.push(module);
        }
    }

    modules
}

/// User servers, then project servers: a project entry replaces the same-named user server, and
/// an entry without `command`/`url`/`type` only overrides the user server's `enabled` flag.
fn mcp_servers(paths: &HarnessPaths) -> Vec<McpServerInfo> {
    let user_path = paths.agent_dir.join("mcp.json");
    let user_source = source(&user_path, ConfigLayer::User, None, ConfigFormat::Json);
    let user_file = read_json(&user_path);
    let mut servers: Vec<McpServerInfo> = parse_servers(user_file.get("mcpServers"), &["mcpServers"], &user_source)
        .into_iter()
        .map(|parsed| parsed.info)
        .collect();

    let Some(project_path) = paths.project_dir.as_ref().map(|dir| dir.join("mcp.json")) else {
        return servers;
    };
    let project_source = source(&project_path, ConfigLayer::Project, None, ConfigFormat::Json);
    let project_file = read_json(&project_path);

    for parsed in parse_servers(project_file.get("mcpServers"), &["mcpServers"], &project_source) {
        let existing = servers.iter_mut().find(|server| server.name == parsed.info.name);

        match (existing, parsed.has_endpoint) {
            (Some(existing), false) => existing.enabled = parsed.enabled_flag.unwrap_or(existing.enabled),
            (Some(existing), true) => *existing = parsed.info,
            (None, true) => servers.push(parsed.info),
            (None, false) => {}
        }
    }

    servers
}
