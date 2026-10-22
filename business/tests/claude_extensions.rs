//! The Claude Code extensions view in detail: which plugin installs apply, what plugins may
//! contribute, the settings layers that switch plugins and `.mcp.json` servers, and precedence
//! between the user's, the project's and the plugins' skills, agents, commands and MCP servers.
//! The fixture home's full view is snapshotted in `claude_views.rs`.

mod common;

use std::path::{Path, PathBuf};

use common::{CANARY, claude_home, demo_project, env_at, json_text, write_file};
use harness_business::env::{Env, OsKind};
use harness_business::harness::Harness;
use harness_business::harnesses::Claude;
use harness_business::model::{ExtensionScope, ExtensionsView, McpServerInfo, NoticeLevel, PluginInfo};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn extensions(env: &Env, project: Option<&Path>) -> ExtensionsView {
    Claude.extensions_view(env, project).unwrap()
}

fn plugin<'a>(view: &'a ExtensionsView, id: &str) -> &'a PluginInfo {
    view.plugins
        .iter()
        .find(|plugin| plugin.id == id)
        .unwrap_or_else(|| panic!("plugin {id} missing"))
}

fn server<'a>(view: &'a ExtensionsView, name: &str) -> &'a McpServerInfo {
    view.mcp_servers
        .iter()
        .find(|server| server.name == name)
        .unwrap_or_else(|| panic!("server {name} missing"))
}

fn names<T>(items: &[T], name: impl Fn(&T) -> &str) -> Vec<&str> {
    items.iter().map(name).collect()
}

/// A canonical temporary home and project (so paths compare equal on every OS).
struct Scratch {
    _home: tempfile::TempDir,
    _project: tempfile::TempDir,
    home: PathBuf,
    project: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let home_dir = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();
        let home = dunce::canonicalize(home_dir.path()).unwrap();
        let project = dunce::canonicalize(project_dir.path()).unwrap();
        std::fs::create_dir_all(project.join(".git")).unwrap();

        Self {
            _home: home_dir,
            _project: project_dir,
            home,
            project,
        }
    }

    fn env(&self) -> Env {
        env_at(&self.home)
    }

    fn write(&self, relative: &str, value: &Value) {
        write_file(&self.home.join(relative), &value.to_string());
    }

    fn write_project(&self, relative: &str, value: &Value) {
        write_file(&self.project.join(relative), &value.to_string());
    }

    /// A plugin in the cache with a skill named after it, returning its directory.
    fn cached_plugin(&self, name: &str) -> PathBuf {
        let dir = self.home.join(".claude/plugins/cache/market").join(name).join("1.0.0");
        write_file(
            &dir.join("skills/helper/SKILL.md"),
            "---\nname: helper\ndescription: A plugin skill.\n---\n",
        );

        dir
    }

    fn install(&self, plugins: Value) {
        self.write(
            ".claude/plugins/installed_plugins.json",
            &json!({ "version": 2, "plugins": plugins }),
        );
    }
}

#[test]
fn installs_for_the_open_project_apply_and_the_most_specific_one_wins() {
    let scratch = Scratch::new();
    let shared = scratch.cached_plugin("shared");
    let mine = scratch.cached_plugin("mine");
    let theirs = scratch.cached_plugin("theirs");
    let project = scratch.project.to_string_lossy();

    scratch.install(json!({
        "shared@market": [
            { "scope": "user", "installPath": shared, "version": "1.0.0" },
            { "scope": "project", "projectPath": project, "installPath": shared, "version": "1.1.0" },
        ],
        "mine@market": [
            { "scope": "project", "projectPath": project, "installPath": mine, "version": "1.0.0" },
            { "scope": "local", "projectPath": project, "installPath": mine, "version": "1.2.0" },
        ],
        "theirs@market": [{ "scope": "project", "projectPath": "/somewhere/else", "installPath": theirs }],
    }));
    scratch.write_project(
        ".claude/settings.json",
        &json!({ "enabledPlugins": { "shared@market": true, "mine@market": true, "theirs@market": true } }),
    );

    let view = extensions(&scratch.env(), Some(&scratch.project));

    assert_eq!(
        names(&view.plugins, |plugin| &plugin.id),
        ["shared@market", "mine@market"]
    );
    assert_eq!(plugin(&view, "shared@market").version.as_deref(), Some("1.1.0"));
    assert_eq!(plugin(&view, "mine@market").version.as_deref(), Some("1.2.0"));
    assert!(
        view.plugins
            .iter()
            .all(|plugin| plugin.scope == ExtensionScope::Project && plugin.enabled)
    );
    assert_eq!(
        names(&view.skills, |skill| &skill.name),
        ["shared:helper", "mine:helper"]
    );

    let without_project = extensions(&scratch.env(), None);
    assert_eq!(names(&without_project.plugins, |plugin| &plugin.id), ["shared@market"]);
    assert_eq!(plugin(&without_project, "shared@market").scope, ExtensionScope::User);
    assert!(!plugin(&without_project, "shared@market").enabled);
}

#[test]
fn managed_settings_switch_plugins_over_every_other_layer() {
    let scratch = Scratch::new();
    let dir = scratch.cached_plugin("tool");
    scratch.install(json!({ "tool@market": [{ "scope": "user", "installPath": dir }] }));
    scratch.write(
        ".claude/settings.json",
        &json!({ "enabledPlugins": { "tool@market": true } }),
    );
    scratch.write(
        "Program Files/ClaudeCode/managed-settings.json",
        &json!({ "enabledPlugins": { "tool@market": false } }),
    );

    let unmanaged = extensions(&scratch.env(), None);
    assert!(plugin(&unmanaged, "tool@market").enabled);

    let managed = scratch
        .env()
        .with_os(OsKind::Windows)
        .with_var("ProgramFiles", scratch.home.join("Program Files").to_string_lossy());
    let view = extensions(&managed, None);

    assert!(!plugin(&view, "tool@market").enabled);
    assert!(view.skills.is_empty(), "a disabled plugin contributes nothing");
}

#[test]
fn a_plugin_outside_the_cache_is_listed_but_never_read() {
    let home = claude_home();
    let view = extensions(&env_at(&home), Some(&demo_project()));

    let escaping = plugin(&view, "escaping@demo-market");
    assert!(escaping.enabled);
    assert_eq!(escaping.description, None);

    let contributed = |prefix: &str| {
        let skills = view.skills.iter().map(|skill| skill.name.as_str());
        let agents = view.agents.iter().map(|agent| agent.name.as_str());
        let commands = view.prompt_templates.iter().map(|command| command.name.as_str());
        let servers = view.mcp_servers.iter().map(|server| server.name.as_str());

        skills
            .chain(agents)
            .chain(commands)
            .chain(servers)
            .filter(|name| name.contains(prefix))
            .count()
    };
    assert_eq!(contributed("escaping:"), 0);
    assert_eq!(contributed("lint-tools:"), 0, "lint-tools is installed but off");
    assert!(contributed("useful-skills:") >= 4);
}

#[test]
fn user_skills_and_commands_outrank_the_projects_and_project_agents_outrank_the_users() {
    let scratch = Scratch::new();
    let skill = |description: &str| format!("---\nname: same\ndescription: {description}\n---\n");
    let agent = |description: &str| format!("---\nname: same\ndescription: {description}\n---\n");
    let command = |description: &str| format!("---\ndescription: {description}\n---\n");

    for (root, layer) in [
        (scratch.home.join(".claude"), "user"),
        (scratch.project.join(".claude"), "project"),
    ] {
        write_file(&root.join("skills/same/SKILL.md"), &skill(layer));
        write_file(&root.join("agents/same.md"), &agent(layer));
        write_file(&root.join("commands/same.md"), &command(layer));
    }
    write_file(
        &scratch.project.join(".claude/agents/unnamed.md"),
        "---\ndescription: no name\n---\n",
    );

    let view = extensions(&scratch.env(), Some(&scratch.project));

    assert_eq!(view.skills.len(), 1);
    assert_eq!(view.skills[0].description, "user");
    assert_eq!(view.prompt_templates.len(), 1);
    assert_eq!(view.prompt_templates[0].description.as_deref(), Some("user"));
    assert_eq!(names(&view.agents, |agent| &agent.name), ["same"]);
    assert_eq!(view.agents[0].description.as_deref(), Some("project"));
    assert_eq!(view.agents[0].scope, ExtensionScope::Project);
}

#[test]
fn mcp_servers_follow_claudes_precedence_and_mcp_json_answers() {
    let scratch = Scratch::new();
    let project = scratch.project.to_string_lossy();
    scratch.write(
        ".claude.json",
        &json!({
            "mcpServers": {
                "shared": { "command": "user-shared" },
                "listed": { "command": "user-listed" },
                "muted": { "command": "user-muted", "env": { "TOKEN": CANARY } },
            },
            "projects": {
                project: {
                    "mcpServers": { "shared": { "command": "local-shared", "args": ["--api-key", CANARY] } },
                    "disabledMcpServers": ["muted"],
                    "disabledMcpjsonServers": ["refused"],
                },
            },
        }),
    );
    scratch.write_project(
        ".mcp.json",
        &json!({
            "mcpServers": {
                "shared": { "command": "project-shared" },
                "listed": { "command": "project-listed" },
                "approved": { "type": "sse", "url": "https://mcp.example.com/sse" },
                "refused": { "command": "project-refused" },
                "waiting": { "command": "project-waiting", "env": { "KEY": "${MY_KEY}" } },
            },
        }),
    );
    scratch.write_project(
        ".claude/settings.local.json",
        &json!({ "enabledMcpjsonServers": ["approved", "listed", "refused"] }),
    );

    let env = scratch.env();
    let view = extensions(&env, Some(&scratch.project));
    let command = |name: &str| server(&view, name).command.as_deref();

    assert_eq!(
        names(&view.mcp_servers, |server| &server.name),
        ["shared", "listed", "approved", "muted", "refused", "waiting"]
    );
    assert_eq!(command("shared"), Some("local-shared"));
    assert_eq!(command("listed"), Some("project-listed"));
    assert!(server(&view, "approved").enabled);
    assert!(!server(&view, "muted").enabled, "disabledMcpServers switches it off");
    assert!(!server(&view, "refused").enabled, "turned down outranks approved");
    assert!(!server(&view, "waiting").enabled, "no answer yet");
    assert!(!json_text(&view).contains(CANARY));

    scratch.write_project(".claude/settings.json", &json!({ "enableAllProjectMcpServers": true }));
    let all = extensions(&env, Some(&scratch.project));
    assert!(server(&all, "waiting").enabled);
    assert!(!server(&all, "refused").enabled);
}

#[test]
fn unreadable_plugin_files_report_a_notice_without_content() {
    let scratch = Scratch::new();
    write_file(
        &scratch.home.join(".claude/plugins/installed_plugins.json"),
        &format!("{{\"plugins\": \"{CANARY}\""),
    );
    write_file(
        &scratch.home.join(".claude/plugins/known_marketplaces.json"),
        &format!("[\"{CANARY}\""),
    );

    let view = extensions(&scratch.env(), None);
    let messages = names(&view.notices, |notice| &notice.message);

    assert!(messages[0].starts_with("Installed plugins could not be read: Invalid JSON at line"));
    assert!(messages[1].starts_with("The marketplace list could not be read: Invalid JSON at line"));
    assert!(view.notices.iter().all(|notice| notice.level == NoticeLevel::Warning));
    assert!(!json_text(&view).contains(CANARY));
}

#[test]
fn a_directory_marketplace_reads_only_its_catalog() {
    let scratch = Scratch::new();
    let folder = scratch.home.join("my-market");
    write_file(
        &folder.join(".claude-plugin/marketplace.json"),
        &json!({ "plugins": [{ "name": "mine", "version": "2.0.0" }] }).to_string(),
    );
    scratch.write(
        ".claude/plugins/known_marketplaces.json",
        &json!({
            "mine": { "source": { "source": "directory", "path": folder }, "installLocation": folder },
            "relative": { "source": { "source": "directory", "path": "my-market" } },
        }),
    );

    let view = extensions(&scratch.env(), None);

    assert_eq!(view.marketplaces[0].entries[0].name, "mine");
    assert_eq!(view.marketplaces[0].error, None);
    assert!(
        view.marketplaces[1].entries.is_empty(),
        "relative folders are not resolved"
    );
    assert!(view.marketplaces[1].error.is_some());
}
