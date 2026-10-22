//! The Claude Code extensions view (Claude Code 2.1.304): plugins ([`plugins`]), marketplaces
//! ([`marketplaces`]), skills, agents and commands ([`markdown`]) and MCP servers ([`mcp`]). It
//! reads files only, never runs `claude`.
//!
//! Skills are `<dir>/<skill>/SKILL.md` in the config directory's `skills/`, in `.claude/skills` from
//! the project up to its repository root, and in each enabled plugin's `skills/` (named
//! `<plugin>:<skill>`). A user skill outranks a project skill of the same name. Plugins
//! contribute only while enabled, and only from a directory inside the plugin cache.

mod markdown;
mod marketplaces;
mod mcp;
mod plugins;
pub(super) mod settings;

use std::path::{Path, PathBuf};

use self::markdown::{Folder, agents, commands};
use self::plugins::{EnabledPlugin, installed};
use self::settings::Settings;
use super::config::global_sections;
use super::paths::{PROJECT_DIR_NAME, config_dir};
use crate::env::Env;
use crate::harnesses::plugin_registry::mark_updates;
use crate::model::{ExtensionScope, ExtensionsView, HarnessId, SkillInfo};
use crate::pi_family::{
    MAX_WALK_ENTRIES, PerLayer, ScanBudget, SkillRoot, Truncation, WalkBudget, dedupe_skills, project_ancestors,
    scan_roots, scan_skills_budgeted,
};

/// Claude reads `<dir>/<skill>/SKILL.md` one level deep.
const SKILL_DEPTH: usize = 1;

/// See the module docs.
pub(super) fn extensions_view(env: &Env, project: Option<&Path>) -> ExtensionsView {
    let settings = Settings::load(env, project);
    let mut plugins = installed(env, project, &settings);
    let (marketplaces, marketplace_notices) = marketplaces::marketplaces(env, &settings, &plugins.plugins);
    mark_updates(&mut plugins.plugins, &marketplaces);

    let global = global_sections(env, project);
    let mcp_servers = mcp::mcp_servers(project, &settings, &global, &plugins.enabled);

    let mut truncation = Truncation::default();
    let skills = skills(env, project, &plugins.enabled, &mut truncation);

    let mut budgets = PerLayer::new(ScanBudget::per_view());
    let agents = agents(&folders(env, project, &plugins.enabled, "agents", true), &mut budgets);
    let prompt_templates = commands(
        &folders(env, project, &plugins.enabled, "commands", false),
        &mut budgets,
    );
    truncation.note_scans(&budgets);

    let mut notices = plugins.notices;
    notices.extend(marketplace_notices);
    notices.extend(truncation.notices());

    ExtensionsView {
        harness: HarnessId::Claude,
        plugins: plugins.plugins,
        marketplaces,
        skills,
        prompt_templates,
        agents,
        mcp_servers,
        themes: Vec::new(),
        packages: Vec::new(),
        extension_modules: Vec::new(),
        notices,
    }
}

/// The project's `.claude` folders, closest first (see [`project_ancestors`]).
fn project_dirs(env: &Env, project: Option<&Path>) -> Vec<PathBuf> {
    project
        .map(|project| project_ancestors(env, project))
        .unwrap_or_default()
        .into_iter()
        .map(|dir| dir.join(PROJECT_DIR_NAME))
        .collect()
}

/// The `sub` folders (`agents`, `commands`) in precedence order: the project's before the user's
/// when `project_first`, then the enabled plugins'.
fn folders(
    env: &Env,
    project: Option<&Path>,
    plugins: &[EnabledPlugin],
    sub: &str,
    project_first: bool,
) -> Vec<Folder> {
    let user = Folder::plain(config_dir(env).join(sub), ExtensionScope::User);
    let projects = project_dirs(env, project)
        .into_iter()
        .map(|dir| Folder::plain(dir.join(sub), ExtensionScope::Project));

    let mut folders: Vec<Folder> = if project_first {
        projects.chain(std::iter::once(user)).collect()
    } else {
        std::iter::once(user).chain(projects).collect()
    };

    folders.extend(plugins.iter().map(|plugin| Folder::of_plugin(plugin, sub)));
    folders
}

/// See the module docs. Walks cut short are recorded in `truncation`.
fn skills(env: &Env, project: Option<&Path>, plugins: &[EnabledPlugin], truncation: &mut Truncation) -> Vec<SkillInfo> {
    let mut roots = vec![SkillRoot::labeled(
        env,
        project,
        config_dir(env).join("skills"),
        ExtensionScope::User,
    )];
    roots.extend(
        project_dirs(env, project)
            .into_iter()
            .map(|dir| SkillRoot::labeled(env, project, dir.join("skills"), ExtensionScope::Project)),
    );

    let mut skills = scan_roots(&roots, SKILL_DEPTH, truncation);

    for plugin in plugins {
        let root = SkillRoot {
            dir: plugin.dir.join("skills"),
            scope: ExtensionScope::Plugin,
            origin: format!("plugin {}", plugin.name),
        };
        let mut walk = WalkBudget::new(MAX_WALK_ENTRIES);
        let found = scan_skills_budgeted(&root, SKILL_DEPTH, Some(&plugin.dir), &mut walk);
        truncation.note(ExtensionScope::Plugin, walk.truncated());

        skills.extend(found.into_iter().map(|skill| SkillInfo {
            name: format!("{}:{}", plugin.name, skill.name),
            ..skill
        }));
    }

    dedupe_skills(skills)
}
