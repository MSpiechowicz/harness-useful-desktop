//! OpenCode 2 config and extensions views over a copy of the `opencode2-home` fixture, compared
//! with what OpenCode 1 reads from the same files. The project (`~/work/app`) sits below the home
//! directory, so OpenCode 2's walk reaches `~/.opencode`.
//!
//! OpenCode 2 walks up to the filesystem root, so only files inside the temporary directory are
//! compared.

mod common;

use std::path::{Path, PathBuf};

use common::{CANARY, json_text};
use harness_business::env::Env;
use harness_business::harness::{Harness, OpenCodeGeneration};
use harness_business::harnesses::OpenCode;
use harness_business::model::{
    AgentMode, ConfigLayer, ConfigValue, ConfigView, ExtensionsView, McpServerInfo, NoticeLevel, SecretKind,
};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use OpenCodeGeneration::{V1, V2};

/// A copy of the fixture home in a temporary directory; `home/work/app` is the project.
struct Machine {
    _temp: TempDir,
    home: PathBuf,
    project: PathBuf,
}

impl Machine {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let home = dunce::canonicalize(temp.path()).unwrap().join("home");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/opencode2-home");
        copy_dir(&fixture, &home);

        let project = home.join("work/app");
        Self {
            _temp: temp,
            home,
            project,
        }
    }

    fn env(&self) -> Env {
        Env::for_tests(&self.home)
    }

    /// `path` relative to the home directory (`~/...`), `/`-separated; `None` outside the copy.
    fn shown(&self, path: &str) -> Option<String> {
        let root = self.home.parent().unwrap();
        let path = Path::new(path);
        path.strip_prefix(root).ok()?;

        let rest = path.strip_prefix(&self.home).ok()?;
        Some(format!("~/{}", rest.to_string_lossy().replace('\\', "/")))
    }

    fn sections(&self, view: &ConfigView) -> Vec<(String, ConfigLayer)> {
        view.sections
            .iter()
            .filter_map(|section| Some((self.shown(&section.source.path)?, section.source.layer)))
            .collect()
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();

    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());

        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn config(generation: OpenCodeGeneration, env: &Env, project: Option<&Path>) -> ConfigView {
    OpenCode::with_generation(generation).config_view(env, project).unwrap()
}

fn extensions(generation: OpenCodeGeneration, env: &Env, project: Option<&Path>) -> ExtensionsView {
    OpenCode::with_generation(generation)
        .extensions_view(env, project)
        .unwrap()
}

fn server<'a>(view: &'a ExtensionsView, name: &str) -> &'a McpServerInfo {
    view.mcp_servers
        .iter()
        .find(|server| server.name == name)
        .unwrap_or_else(|| panic!("server {name} missing"))
}

fn plugin_names(view: &ExtensionsView) -> Vec<&str> {
    view.plugins.iter().map(|plugin| plugin.name.as_str()).collect()
}

#[test]
fn opencode_2_reads_no_config_json_and_its_terminal_settings_from_cli_json() {
    let machine = Machine::new();
    let env = machine.env();

    assert_eq!(
        machine.sections(&config(V2, &env, None)),
        [
            ("~/.config/opencode/opencode.jsonc".to_owned(), ConfigLayer::User),
            ("~/.config/opencode/cli.json".to_owned(), ConfigLayer::User),
        ]
    );

    let v1: Vec<String> = machine
        .sections(&config(V1, &env, None))
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert_eq!(
        v1,
        [
            "~/.config/opencode/config.json",
            "~/.config/opencode/opencode.jsonc",
            "~/.config/opencode/tui.json",
            "~/.opencode/opencode.json",
        ]
    );
}

#[test]
fn the_project_walk_passes_the_repository_root_and_reads_home_settings_below_home() {
    let machine = Machine::new();
    std::fs::create_dir_all(machine.home.join("work/.git")).unwrap();
    let view = config(V2, &machine.env(), Some(&machine.project));

    assert_eq!(
        machine.sections(&view),
        [
            ("~/.config/opencode/opencode.jsonc".to_owned(), ConfigLayer::User),
            ("~/.config/opencode/cli.json".to_owned(), ConfigLayer::User),
            ("~/work/app/opencode.json".to_owned(), ConfigLayer::Project),
            ("~/.opencode/opencode.json".to_owned(), ConfigLayer::User),
            ("~/work/app/.opencode/opencode.jsonc".to_owned(), ConfigLayer::Project),
        ]
    );
}

#[test]
fn opencode_config_dir_replaces_the_global_directory_on_opencode_2_only() {
    let machine = Machine::new();
    let team = machine.home.join("team-opencode");
    let env = machine.env().with_var("OPENCODE_CONFIG_DIR", team.to_string_lossy());

    assert_eq!(
        machine.sections(&config(V2, &env, None)),
        [("~/team-opencode/opencode.json".to_owned(), ConfigLayer::User)]
    );
    assert_eq!(plugin_names(&extensions(V2, &env, None)), ["team-plugin"]);

    let v1 = machine.sections(&config(V1, &env, None));
    assert_eq!(v1[1].0, "~/.config/opencode/opencode.jsonc");
    assert_eq!(v1.last().unwrap().0, "~/team-opencode/opencode.json");
}

#[test]
fn plugins_come_from_both_keys_with_directories_first_and_removals_applied() {
    let machine = Machine::new();
    let env = machine.env();
    let view = extensions(V2, &env, Some(&machine.project));

    assert_eq!(
        plugin_names(&view),
        [
            "dir-plugin",
            "hello",
            "project",
            "legacy-tuple",
            "native-plugin",
            "@acme/with-options",
            "plain-object",
            "local-dir-plugin",
            "home-dot-plugin",
        ]
    );

    let native = view
        .plugins
        .iter()
        .find(|plugin| plugin.name == "native-plugin")
        .unwrap();
    assert_eq!(native.version.as_deref(), Some("2.0.0"));

    let v1 = extensions(V1, &env, None);
    assert!(plugin_names(&v1).contains(&"legacy-plugin"));
    assert!(plugin_names(&v1).contains(&"config-json-plugin"));
    assert!(!plugin_names(&v1).contains(&"native-plugin"));
}

#[test]
fn mcp_servers_merge_legacy_and_nested_entries_and_read_disabled() {
    let machine = Machine::new();
    let view = extensions(V2, &machine.env(), Some(&machine.project));

    let names: Vec<&str> = view.mcp_servers.iter().map(|server| server.name.as_str()).collect();
    assert_eq!(
        names,
        ["legacy-local", "native-remote", "native-local", "project-server"]
    );

    let legacy = server(&view, "legacy-local");
    assert_eq!(legacy.command.as_deref(), Some("overridden-server"));
    assert!(legacy.enabled, "the nested entry replaced the switched-off legacy one");

    let local = server(&view, "native-local");
    assert!(!local.enabled);
    assert_eq!(local.args[1], ConfigValue::Secret(SecretKind::Value));
    assert!(local.details.iter().all(|field| field.key != "disabled"));

    let remote = server(&view, "native-remote");
    assert_eq!(
        remote.url,
        Some(ConfigValue::Text("https://mcp.example.com/mcp".to_owned()))
    );

    assert_eq!(server(&view, "project-server").source.layer, ConfigLayer::Project);
}

#[test]
fn agents_and_commands_come_from_both_keys() {
    let machine = Machine::new();
    let view = extensions(V2, &machine.env(), Some(&machine.project));
    let agent = |name: &str| view.agents.iter().find(|agent| agent.name == name);

    assert!(agent("explore").is_none(), "legacy disable removes a built-in agent");
    assert_eq!(
        agent("plan").unwrap().description.as_deref(),
        Some("Read-only agent for exploring the codebase and planning work before implementation.")
    );

    let legacy = agent("legacy-agent").unwrap();
    assert_eq!(legacy.mode, AgentMode::Subagent);
    assert_eq!(legacy.model.as_deref(), Some("anthropic/claude-sonnet-4-5"));

    let native = agent("native-agent").unwrap();
    assert_eq!(native.mode, AgentMode::Primary);
    assert_eq!(native.model.as_deref(), Some("openai/gpt-5"));
    assert_eq!(native.description.as_deref(), Some("Project native agent"));
    assert!(agent("reviewer").is_some());

    let commands: Vec<&str> = view
        .prompt_templates
        .iter()
        .map(|template| template.name.as_str())
        .collect();
    assert_eq!(commands, ["legacy-cmd", "native-cmd", "test"]);
}

#[test]
fn the_active_theme_comes_from_cli_json() {
    let machine = Machine::new();
    let env = machine.env();
    let active = |view: &ExtensionsView| {
        let theme = view.themes.iter().find(|theme| theme.active).unwrap();
        theme.name.clone()
    };

    assert_eq!(active(&extensions(V2, &env, None)), "dusk");
    assert_eq!(active(&extensions(V1, &env, None)), "nord");
}

#[test]
fn opencode_2_has_no_permission_override_notice() {
    let machine = Machine::new();
    let env = machine.env().with_var("OPENCODE_PERMISSION", "{}");
    let warnings = |view: ConfigView| {
        view.notices
            .into_iter()
            .filter(|notice| notice.level == NoticeLevel::Warning)
            .count()
    };

    assert_eq!(warnings(config(V2, &env, None)), 0);
    assert_eq!(warnings(config(V1, &env, None)), 1);
}

#[test]
fn skills_include_claude_skills_and_no_secret_reaches_a_view() {
    let machine = Machine::new();
    let env = machine.env();
    let project = Some(machine.project.as_path());
    let view = extensions(V2, &env, project);

    assert!(view.skills.iter().any(|skill| skill.name == "claude-notes"));

    for text in [json_text(&view), json_text(config(V2, &env, project))] {
        assert!(!text.contains(CANARY), "a view leaks the canary");
    }
}
