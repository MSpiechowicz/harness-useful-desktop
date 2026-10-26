//! The Claude Code extensions view (Claude Code 2.1.304): plugins ([`plugins`]), marketplaces
//! ([`marketplaces`]), skills, agents and commands ([`markdown`]) and MCP servers ([`mcp`]). It
//! reads files only, never runs `claude`.
//!
//! Skills are `<dir>/<skill>/SKILL.md` in the config directory's `skills/`, in `.claude/skills` from
//! the project up to its repository root, and in each enabled plugin's `skills/` (named
//! `<plugin>:<skill>`). A user skill outranks a project skill of the same name. Plugins
//! contribute only while enabled, and only from a directory inside the plugin cache. What a
//! project layer holds is read only when it resolves inside the folder of that layer.

mod markdown;
mod marketplaces;
mod mcp;
mod plugins;
pub(super) mod settings;

use std::path::{Path, PathBuf};

use self::markdown::{Folder, agents, commands};
pub(super) use self::marketplaces::{known_entries, source_location};
use self::plugins::{EnabledPlugin, installed};
pub(super) use self::plugins::{global_install, is_enabled, registered};
pub(super) use self::settings::Settings;
use super::config::global_sections;
use super::paths::{PROJECT_DIR_NAME, config_dir};
use crate::env::Env;
use crate::harnesses::plugin_registry::mark_updates;
use crate::model::{ExtensionScope, ExtensionsView, HarnessId, Notice, NoticeLevel, SkillInfo};
use crate::pi_family::{
    Document, PerLayer, ScanBudget, SkillRoot, Truncation, dedupe_skills, project_ancestors, scan_plugin_skills,
    scan_project_skills, scan_roots,
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
    let global_notice = global.user.as_ref().and_then(|(_, document)| match document {
        Document::Invalid(summary) => Some(Notice {
            level: NoticeLevel::Warning,
            message: format!("Your MCP servers in .claude.json could not be read: {summary}"),
        }),
        _ => None,
    });

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
    notices.extend(global_notice);
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

/// The project's folders holding a `.claude` layer, closest first (see [`project_ancestors`]).
/// What a layer holds must resolve inside its folder.
fn project_dirs(env: &Env, project: Option<&Path>) -> Vec<PathBuf> {
    project
        .map(|project| project_ancestors(env, project))
        .unwrap_or_default()
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
        .map(|dir| Folder::within(dir.join(PROJECT_DIR_NAME).join(sub), ExtensionScope::Project, dir));

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
    let user = SkillRoot::labeled(env, project, config_dir(env).join("skills"), ExtensionScope::User);
    let mut skills = scan_roots(&[user], SKILL_DEPTH, truncation);

    let relative = Path::new(PROJECT_DIR_NAME).join("skills");
    skills.extend(scan_project_skills(env, project, &relative, SKILL_DEPTH, truncation));

    for plugin in plugins {
        skills.extend(scan_plugin_skills(&plugin.name, &plugin.dir, SKILL_DEPTH, truncation));
    }

    dedupe_skills(skills)
}
