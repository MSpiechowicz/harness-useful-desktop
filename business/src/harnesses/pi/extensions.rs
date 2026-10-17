//! PI extensions view: skills, prompt templates, themes, packages, extension modules and MCP.
//!
//! Locations follow the PI docs (configuration, skills, prompt templates, themes, extensions,
//! MCP). Project resources are listed even though PI loads them only for trusted projects; the
//! view carries the trust notice.
//!
//! Scans are bounded. The conventional extension directories each get their own budget, and
//! everything a settings layer lists (with its packages) is charged to that layer's budget, so a
//! project's settings cannot hide the user's modules. A view cut short by any cap says so in a
//! notice.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::config::{TRUST_NOTICE, info};
use super::layers::{Layer, Listed, included, layers, listed};
use super::packages::{InstalledPackage, Packages, resolve_packages};
use super::pi_paths;
use crate::env::Env;
use crate::harness::HarnessPaths;
use crate::model::{
    ConfigFormat, ConfigLayer, ExtensionModuleInfo, ExtensionScope, ExtensionsView, HarnessId, McpServerInfo,
    PromptTemplateInfo, SkillInfo, ThemeInfo,
};
use crate::pi_family::{
    MAX_DECLARED_ENTRIES, MAX_WALK_ENTRIES, PerLayer, SKILL_FILE, ScanBudget, SkillRoot, Truncation, WalkBudget,
    builtin_themes, canonical_inside, dedupe_skills, distinct_targets, distinct_within, extension_module,
    extension_module_paths, load_skill, origin_label, parse_servers, project_ancestors, read_json, scan_roots,
    scan_skills_budgeted, source, text_field, themes_in_dir_budgeted,
};

/// PI discovers skill directories recursively; this caps the walk.
const SKILL_DEPTH: usize = 4;

/// Directory entries the walks of one layer's settings-listed and package skill paths read
/// together, so that many listed paths cannot stall the view.
const LISTED_SKILL_ENTRIES: usize = 4 * MAX_WALK_ENTRIES;

/// Themes shipped with PI.
const BUILTIN_THEMES: &[&str] = &["system", "dark", "light"];

const DEFAULT_THEME: &str = "system";

/// See the module docs.
pub fn pi_extensions_view(env: &Env, project: Option<&Path>) -> ExtensionsView {
    let paths = pi_paths(env, project);
    let layers = layers(&paths);
    let packages = resolve_packages(env, &layers);
    let mut truncation = packages.truncation;

    // Modules first, as they run code. Prompt, theme and module scans of one layer share that
    // layer's budget (skills have their own walks).
    let mut budgets = PerLayer::new(ScanBudget::per_view());
    let extension_modules = extension_modules(env, &paths, &layers, &packages, &mut budgets, &mut truncation);
    let prompt_templates = prompt_templates(env, &paths, &layers, &packages, &mut budgets);
    let themes = themes(env, &paths, &layers, &packages, &mut budgets);
    let skills = skills(env, &paths, project, &layers, &packages, &mut truncation);
    truncation.note_scans(&budgets);

    let mut notices = match project {
        Some(_) => vec![info(TRUST_NOTICE)],
        None => Vec::new(),
    };
    notices.extend(truncation.notices());

    ExtensionsView {
        harness: HarnessId::Pi,
        plugins: Vec::new(),
        marketplaces: Vec::new(),
        skills,
        prompt_templates,
        agents: Vec::new(),
        mcp_servers: mcp_servers(&paths),
        themes,
        extension_modules,
        packages: packages.listed,
        notices,
    }
}

/// Precedence (first name wins): project `.pi/skills`, project `.agents/skills` up to the
/// repository root, user skills, `~/.agents/skills`, paths listed in settings, then installed
/// packages (project packages first), as PI ranks package resources last. Walks cut short are
/// recorded in `truncation`.
fn skills(
    env: &Env,
    paths: &HarnessPaths,
    project: Option<&Path>,
    layers: &[Layer],
    packages: &Packages,
    truncation: &mut Truncation,
) -> Vec<SkillInfo> {
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

    let mut found = scan_roots(&roots, SKILL_DEPTH, truncation);

    let mut budgets = PerLayer::new(WalkBudget::new(LISTED_SKILL_ENTRIES));

    for layer in layers {
        let budget = budgets.get(layer.scope);
        for path in included_paths(env, layer, "skills", budget) {
            found.extend(listed_skills(env, project, layer.scope, &path, budget));
        }
    }

    for (package, path) in packages.resource_paths("skills") {
        found.extend(package_skills(package, path, budgets.get(package.scope)));
    }

    truncation.note_walks(&budgets);
    dedupe_skills(found)
}

/// The distinct paths `layer` includes under `key`, at most [`MAX_DECLARED_ENTRIES`] of them (more
/// mark `budget` truncated).
fn included_paths(env: &Env, layer: &Layer, key: &str, budget: &mut WalkBudget) -> Vec<PathBuf> {
    let (paths, capped) = distinct_within(included(env, layer, key), MAX_DECLARED_ENTRIES);
    if capped {
        budget.mark_truncated();
    }

    paths
}

/// [`included_paths`] with aliases of one target (`../hub`, `../hub/../hub`, links) counted once,
/// resolved within `budget`.
fn included_targets(env: &Env, layer: &Layer, key: &str, budget: &mut ScanBudget) -> Vec<PathBuf> {
    distinct_targets(included(env, layer, key), MAX_DECLARED_ENTRIES, budget)
}

/// A settings-listed skill directory, skill directory or `SKILL.md` file.
fn listed_skills(
    env: &Env,
    project: Option<&Path>,
    scope: ExtensionScope,
    path: &Path,
    budget: &mut WalkBudget,
) -> Vec<SkillInfo> {
    let skill_file = skill_file(path);
    let skill_dir = skill_file.parent().unwrap_or(path);

    let root = SkillRoot {
        dir: path.to_path_buf(),
        scope,
        origin: origin_label(env, project, skill_dir),
    };

    skills_at(&root, path, None, budget)
}

/// The skills a package provides at `path`, labeled `package <name>`; nothing resolving outside
/// the package directory is read.
fn package_skills(package: &InstalledPackage, path: &Path, budget: &mut WalkBudget) -> Vec<SkillInfo> {
    let root = SkillRoot {
        dir: path.to_path_buf(),
        scope: package.scope,
        origin: format!("package {}", package.name),
    };

    skills_at(&root, path, Some(&package.dir), budget)
        .into_iter()
        .map(|skill| SkillInfo {
            package: Some(package.name.clone()),
            ..skill
        })
        .collect()
}

/// The skill at `path` (a skill directory or its `SKILL.md`), else the skills below it. With a
/// `boundary`, files and directories resolving outside it are skipped. Walks are charged to
/// `budget`.
fn skills_at(root: &SkillRoot, path: &Path, boundary: Option<&Path>, budget: &mut WalkBudget) -> Vec<SkillInfo> {
    let skill_file = skill_file(path);

    if skill_file.is_file() {
        let escapes = boundary.is_some_and(|boundary| !is_inside(boundary, &skill_file));
        if escapes {
            return Vec::new();
        }

        let skill_dir = skill_file.parent().unwrap_or(path);
        let dir_name = skill_dir.file_name().map(|name| name.to_string_lossy().into_owned());

        return load_skill(root, &dir_name.unwrap_or_default(), &skill_file)
            .into_iter()
            .collect();
    }

    scan_skills_budgeted(root, SKILL_DEPTH, boundary, budget)
}

/// `path` itself when it names a `SKILL.md` file, else `<path>/SKILL.md`.
fn skill_file(path: &Path) -> PathBuf {
    let is_skill_file = path.file_name().is_some_and(|name| name == SKILL_FILE);

    if is_skill_file {
        path.to_path_buf()
    } else {
        path.join(SKILL_FILE)
    }
}

/// True when `path` resolves (through `..` and symlinks) inside the canonical directory `dir`.
fn is_inside(dir: &Path, path: &Path) -> bool {
    canonical_inside(dir, path).is_some()
}

/// Direct `*.md` children of `.pi/prompts` and `<agent>/prompts`, plus listed files/directories,
/// then those of installed packages, each scanned within its layer's budget.
fn prompt_templates(
    env: &Env,
    paths: &HarnessPaths,
    layers: &[Layer],
    packages: &Packages,
    budgets: &mut PerLayer<ScanBudget>,
) -> Vec<PromptTemplateInfo> {
    let mut templates = Vec::new();

    for layer in layers {
        let budget = budgets.get(layer.scope);
        let conventional = match layer.scope {
            ExtensionScope::Project => paths.project_dir.as_ref().map(|dir| dir.join("prompts")),
            _ => Some(paths.agent_dir.join("prompts")),
        };

        let listed = included_targets(env, layer, "prompts", budget);
        for path in conventional.into_iter().chain(listed) {
            for file in markdown_files(&path, budget) {
                templates.push(template(&file, layer.scope, budget));
            }
        }
    }

    for (package, path) in packages.resource_paths("prompts") {
        let budget = budgets.get(package.scope);
        let files = markdown_files(path, budget);
        let inside: Vec<PathBuf> = files
            .into_iter()
            .filter(|file| budget.is_inside(&package.dir, file))
            .collect();

        for file in inside {
            templates.push(template(&file, package.scope, budget));
        }
    }

    templates
}

/// `path` itself when it is a Markdown file, else its direct `*.md` children.
fn markdown_files(path: &Path, budget: &mut ScanBudget) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }

    budget
        .list_dir(path)
        .into_iter()
        .filter(|entry| entry.is_file && entry.name.ends_with(".md"))
        .map(|entry| entry.path)
        .collect()
}

/// The template named after its file; description and hint from frontmatter (the body is never
/// read into the view).
fn template(path: &Path, scope: ExtensionScope, budget: &mut ScanBudget) -> PromptTemplateInfo {
    let frontmatter = budget.read_frontmatter(path).unwrap_or(Value::Null);

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

/// Built-ins, `<agent>/themes`, `.pi/themes`, listed theme paths and installed packages' themes,
/// each scanned within its layer's budget. The `theme` setting (project first) selects one theme or a
/// `light/dark` pair; the default is `system`.
fn themes(
    env: &Env,
    paths: &HarnessPaths,
    layers: &[Layer],
    packages: &Packages,
    budgets: &mut PerLayer<ScanBudget>,
) -> Vec<ThemeInfo> {
    let setting = layers
        .iter()
        .find_map(|layer| layer.settings.get("theme").and_then(Value::as_str))
        .unwrap_or(DEFAULT_THEME)
        .to_owned();
    let is_active = |name: &str| setting.split('/').any(|selected| selected == name);

    let mut themes = builtin_themes(BUILTIN_THEMES, &is_active);
    themes.extend(themes_in_dir_budgeted(
        &paths.agent_dir.join("themes"),
        ExtensionScope::User,
        &is_active,
        &mut budgets.user,
    ));

    if let Some(dir) = &paths.project_dir {
        themes.extend(themes_in_dir_budgeted(
            &dir.join("themes"),
            ExtensionScope::Project,
            &is_active,
            &mut budgets.project,
        ));
    }

    for layer in layers {
        let budget = budgets.get(layer.scope);
        for path in included_targets(env, layer, "themes", budget) {
            themes.extend(themes_in_dir_budgeted(&path, layer.scope, &is_active, budget));
        }
    }

    for (package, path) in packages.resource_paths("themes") {
        let budget = budgets.get(package.scope);
        for theme in themes_in_dir_budgeted(path, package.scope, &is_active, budget) {
            let inside = theme
                .path
                .as_deref()
                .is_some_and(|file| budget.is_inside(&package.dir, Path::new(file)));

            if inside {
                themes.push(theme);
            }
        }
    }

    themes
}

/// Modules in `.pi/extensions` and `<agent>/extensions` (each within a budget of its own, so
/// listed paths cannot starve them), listed paths, then installed packages' modules, within their
/// layer's budget; `-path` entries switch a module off.
fn extension_modules(
    env: &Env,
    paths: &HarnessPaths,
    layers: &[Layer],
    packages: &Packages,
    budgets: &mut PerLayer<ScanBudget>,
    truncation: &mut Truncation,
) -> Vec<ExtensionModuleInfo> {
    let mut found: Vec<(PathBuf, ExtensionScope)> = Vec::new();
    let mut excluded: HashSet<PathBuf> = HashSet::new();

    for layer in layers {
        let conventional = match layer.scope {
            ExtensionScope::Project => paths.project_dir.as_ref().map(|dir| dir.join("extensions")),
            _ => Some(paths.agent_dir.join("extensions")),
        };

        if let Some(dir) = conventional {
            let mut own = ScanBudget::per_view();
            let modules = extension_module_paths(&dir, &mut own);
            truncation.note(layer.scope, own.truncated());

            found.extend(modules.into_iter().map(|path| (path, layer.scope)));
        }

        let budget = budgets.get(layer.scope);
        let mut includes = Vec::new();
        for entry in listed(env, layer, "extensions") {
            match entry {
                Listed::Include(path) => includes.push(path),
                Listed::Exclude(path) => {
                    excluded.insert(path);
                }
            }
        }

        for path in distinct_targets(includes, MAX_DECLARED_ENTRIES, budget) {
            if path.is_dir() {
                found.extend(
                    extension_module_paths(&path, budget)
                        .into_iter()
                        .map(|path| (path, layer.scope)),
                );
            } else if path.is_file() {
                found.push((path, layer.scope));
            }
        }
    }

    for (package, path) in packages.resource_paths("extensions") {
        let budget = budgets.get(package.scope);
        let modules = if path.is_dir() {
            extension_module_paths(path, budget)
        } else {
            vec![path.to_path_buf()]
        };
        let inside = modules
            .into_iter()
            .filter(|module| budget.is_inside(&package.dir, module));

        found.extend(inside.map(|module| (module, package.scope)));
    }

    let mut seen = HashSet::new();
    found
        .into_iter()
        .filter(|(path, _)| seen.insert(path.clone()))
        .map(|(path, scope)| extension_module(&path, scope, !excluded.contains(&path)))
        .collect()
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
