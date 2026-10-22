//! The Codex extensions view (Codex CLI 0.154): plugins and marketplaces ([`plugins`]), skills, MCP
//! servers, and notices naming the command rules and the instruction files. It reads files only,
//! never runs `codex` (an unfiltered `plugin/list` returns the whole remote catalogue).
//!
//! - Skills: `<dir>/<skill>/SKILL.md` in `<home>/skills` and `~/.agents/skills` (user), in
//!   `.agents/skills` from the project up to its repository root (project), Codex's bundled skills
//!   in `<home>/skills/.system` (built in), and each enabled plugin's `skills/` (named
//!   `<plugin>:<skill>`). The first skill of a name wins in that order.
//! - MCP servers: `[mcp_servers.<name>]` of the project's `.codex/config.toml` in a trusted
//!   folder, then of the user's `config.toml`, then each enabled plugin's `.mcp.json` (named
//!   `plugin:<plugin>:<server>`). The first definition of a name wins. Redacted like the config
//!   view: `env` and `http_headers` hidden, a `bearer_token_env_var` shown as the variable it names,
//!   secret-looking `args` words masked.
//! - Rules (`<home>/rules/*.rules`) and instructions (`AGENTS.override.md` or `AGENTS.md` in the
//!   home and in the project's folders up to its repository root) are named in notices; their text
//!   is not shown.

mod plugins;

use std::path::{Path, PathBuf};

use serde_json::Value;

use self::plugins::{EnabledPlugin, Plugins};
pub use self::plugins::{MarketplaceRecord, PluginRecord, marketplace_records, plugin_records};
use super::config::marked_env_names;
use super::paths::codex_home;
use super::settings::{is_trusted, project_config, project_config_path, user_config, user_config_path};
use crate::env::Env;
use crate::harnesses::plugin_registry::mark_updates;
use crate::model::{
    ConfigFormat, ConfigLayer, ExtensionScope, ExtensionsView, HarnessId, McpServerInfo, Notice, NoticeLevel, SkillInfo,
};
use crate::pi_family::mcp_config::command_words;
use crate::pi_family::{
    Document, SkillRoot, Truncation, dedupe_skills, list_dir, origin_label, parse_servers, project_ancestors,
    read_json, scan_plugin_skills, scan_project_skills, scan_roots, source,
};
use crate::security::canonical_within;

/// Codex reads `<dir>/<skill>/SKILL.md` one level deep.
const SKILL_DEPTH: usize = 1;

/// The servers key, also the redaction path of every entry.
const SERVERS_KEY: &[&str] = &["mcp_servers"];

/// Most rule files named in the notice.
const MAX_RULE_FILES: usize = 32;

/// The instruction files Codex reads in a folder, the override first (it replaces the other).
const INSTRUCTION_FILES: [&str; 2] = ["AGENTS.override.md", "AGENTS.md"];

/// See the module docs.
pub(super) fn extensions_view(env: &Env, project: Option<&Path>) -> ExtensionsView {
    let user = user_config(env);
    let mut plugins = plugins::installed(env, &user);
    let marketplaces = plugins::marketplaces(env, &user, &plugins.plugins);
    mark_updates(&mut plugins.plugins, &marketplaces);

    let mut truncation = Truncation::default();
    let skills = skills(env, project, &plugins.enabled, &mut truncation);
    let mcp_servers = mcp_servers(env, project, &user, &plugins.enabled);

    let Plugins {
        plugins: listed,
        mut notices,
        ..
    } = plugins;

    if let Document::Invalid(summary) = &user {
        notices.push(Notice {
            level: NoticeLevel::Warning,
            message: format!("Your config.toml could not be read: {summary}"),
        });
    }

    notices.extend(rules_notice(env));
    notices.extend(instructions_notice(env, project));
    notices.extend(truncation.notices());

    ExtensionsView {
        harness: HarnessId::Codex,
        plugins: listed,
        marketplaces,
        skills,
        prompt_templates: Vec::new(),
        agents: Vec::new(),
        mcp_servers,
        themes: Vec::new(),
        packages: Vec::new(),
        extension_modules: Vec::new(),
        notices,
    }
}

/// The project's folders, closest first (see [`project_ancestors`]).
fn project_dirs(env: &Env, project: Option<&Path>) -> Vec<PathBuf> {
    project
        .map(|project| project_ancestors(env, project))
        .unwrap_or_default()
}

/// See the module docs. Walks cut short are recorded in `truncation`.
fn skills(env: &Env, project: Option<&Path>, plugins: &[EnabledPlugin], truncation: &mut Truncation) -> Vec<SkillInfo> {
    let home = codex_home(env);
    let user_roots = [
        SkillRoot::labeled(env, project, home.join("skills"), ExtensionScope::User),
        SkillRoot::labeled(env, project, env.home_join(".agents/skills"), ExtensionScope::User),
    ];
    let mut skills = scan_roots(&user_roots, SKILL_DEPTH, truncation);

    skills.extend(scan_project_skills(
        env,
        project,
        Path::new(".agents/skills"),
        SKILL_DEPTH,
        truncation,
    ));

    let bundled = SkillRoot::labeled(
        env,
        project,
        home.join("skills").join(".system"),
        ExtensionScope::Builtin,
    );
    skills.extend(scan_roots(&[bundled], SKILL_DEPTH, truncation));

    for plugin in plugins {
        skills.extend(scan_plugin_skills(&plugin.name, &plugin.dir, SKILL_DEPTH, truncation));
    }

    dedupe_skills(skills)
}

/// See the module docs.
fn mcp_servers(env: &Env, project: Option<&Path>, user: &Document, plugins: &[EnabledPlugin]) -> Vec<McpServerInfo> {
    let mut ordered = Vec::new();

    if let Some(project) = project.filter(|project| is_trusted(user, project)) {
        let path = project_config_path(project);

        if path != user_config_path(env) {
            let document = project_config(project);
            ordered.extend(servers(document.get("mcp_servers"), &path, ConfigLayer::Project));
        }
    }

    ordered.extend(servers(
        user.get("mcp_servers"),
        &user_config_path(env),
        ConfigLayer::User,
    ));

    for plugin in plugins {
        ordered.extend(plugin_servers(plugin));
    }

    let mut kept: Vec<McpServerInfo> = Vec::new();
    for server in ordered {
        if !kept.iter().any(|existing| existing.name == server.name) {
            kept.push(server);
        }
    }

    kept
}

/// The servers of an enabled plugin's `.mcp.json`: its `mcpServers` object, or the file's top
/// level when it has none. A file that resolves outside the plugin is not read.
fn plugin_servers(plugin: &EnabledPlugin) -> Vec<McpServerInfo> {
    let file = plugin.dir.join(".mcp.json");

    let Ok(file) = canonical_within(&plugin.dir, &file) else {
        return Vec::new();
    };

    let Document::Parsed(value) = read_json(&file) else {
        return Vec::new();
    };

    let entries = value.get("mcpServers").unwrap_or(&value);
    let source = source(&file, ConfigLayer::User, None, ConfigFormat::Json);

    parse_servers(Some(entries), &["mcpServers"], &source)
        .into_iter()
        .map(|parsed| McpServerInfo {
            name: format!("plugin:{}:{}", plugin.name, parsed.info.name),
            args: command_words(parsed.info.args),
            ..parsed.info
        })
        .collect()
}

/// The redacted servers of one `mcp_servers` table.
fn servers(entries: Option<&Value>, path: &Path, layer: ConfigLayer) -> Vec<McpServerInfo> {
    let source = source(path, layer, None, ConfigFormat::Toml);
    let marked = entries.map(marked_env_names);

    parse_servers(marked.as_ref(), SERVERS_KEY, &source)
        .into_iter()
        .map(|parsed| McpServerInfo {
            args: command_words(parsed.info.args),
            ..parsed.info
        })
        .collect()
}

/// The rule files of `<home>/rules`, by name. Their rules are not shown.
fn rules_notice(env: &Env) -> Option<Notice> {
    let names: Vec<String> = list_dir(&codex_home(env).join("rules"))
        .into_iter()
        .filter(|entry| entry.is_file && entry.name.ends_with(".rules"))
        .take(MAX_RULE_FILES)
        .map(|entry| entry.name)
        .collect();

    if names.is_empty() {
        return None;
    }

    Some(info(format!(
        "Codex also applies the command rules in {}, which are shown read-only in Codex itself.",
        names.join(", ")
    )))
}

/// The instruction files Codex loads, by name and layer. Their text is not shown.
fn instructions_notice(env: &Env, project: Option<&Path>) -> Option<Notice> {
    let mut files: Vec<(PathBuf, &str)> = first_instructions(&codex_home(env))
        .map(|path| (path, "user"))
        .into_iter()
        .collect();

    let mut dirs = project_dirs(env, project);
    dirs.reverse();
    files.extend(
        dirs.iter()
            .filter_map(|dir| first_instructions(dir))
            .map(|path| (path, "project")),
    );

    if files.is_empty() {
        return None;
    }

    let names: Vec<String> = files
        .iter()
        .map(|(path, layer)| format!("{} ({layer})", origin_label(env, project, path)))
        .collect();

    Some(info(format!(
        "Codex also loads these instruction files, whose text is not shown here: {}.",
        names.join(", ")
    )))
}

/// The instruction file Codex reads in `dir`: the override when there is one.
fn first_instructions(dir: &Path) -> Option<PathBuf> {
    INSTRUCTION_FILES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

fn info(message: String) -> Notice {
    Notice {
        level: NoticeLevel::Info,
        message,
    }
}
