//! OpenCode config and extensions views over the synthetic fixture home and the demo project.

mod common;

use std::path::Path;

use common::{demo_project, env_at, exported, opencode_home};
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::OpenCode;
use harness_business::model::{
    AgentInfo, AgentMode, ConfigFormat, ConfigLayer, ConfigValue, ConfigView, ExtensionScope, ExtensionsView,
    McpServerInfo, McpTransport, NoticeLevel, SecretKind,
};
use pretty_assertions::assert_eq;

fn config(env: &Env, project: Option<&Path>) -> ConfigView {
    OpenCode::default().config_view(env, project).unwrap()
}

fn extensions(env: &Env, project: Option<&Path>) -> ExtensionsView {
    OpenCode::default().extensions_view(env, project).unwrap()
}

/// Section file paths relative to the fixture home or the demo project.
fn section_files(view: &ConfigView, home: &Path, project: &Path) -> Vec<String> {
    view.sections
        .iter()
        .map(|section| relative(&section.source.path, home, project))
        .collect()
}

fn relative(path: &str, home: &Path, project: &Path) -> String {
    let path = Path::new(path);

    if let Ok(rest) = path.strip_prefix(project) {
        return format!("<project>/{}", rest.to_string_lossy().replace('\\', "/"));
    }

    match path.strip_prefix(home) {
        Ok(rest) => format!("~/{}", rest.to_string_lossy().replace('\\', "/")),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

fn server<'a>(view: &'a ExtensionsView, name: &str) -> &'a McpServerInfo {
    view.mcp_servers
        .iter()
        .find(|server| server.name == name)
        .unwrap_or_else(|| panic!("server {name} missing"))
}

fn agent<'a>(view: &'a ExtensionsView, name: &str) -> Option<&'a AgentInfo> {
    view.agents.iter().find(|agent| agent.name == name)
}

fn names<T>(items: &[T], name: impl Fn(&T) -> &str) -> Vec<String> {
    items.iter().map(|item| name(item).to_owned()).collect()
}

#[test]
fn config_view_snapshots() {
    let home = opencode_home();
    let project = demo_project();
    let env = env_at(&home);

    insta::assert_json_snapshot!("opencode_config_user", exported(config(&env, None), &home, &project));
    insta::assert_json_snapshot!(
        "opencode_config_project",
        exported(config(&env, Some(&project)), &home, &project)
    );
}

#[test]
fn extensions_view_snapshots() {
    let home = opencode_home();
    let project = demo_project();
    let env = env_at(&home);

    insta::assert_json_snapshot!(
        "opencode_extensions_user",
        exported(extensions(&env, None), &home, &project)
    );
    insta::assert_json_snapshot!(
        "opencode_extensions_project",
        exported(extensions(&env, Some(&project)), &home, &project)
    );
}

#[test]
fn layers_follow_the_opencode_merge_order() {
    let home = opencode_home();
    let project = demo_project();
    let env = env_at(&home)
        .with_var("OPENCODE_CONFIG", home.join("custom-opencode.jsonc").to_string_lossy())
        .with_var("OPENCODE_CONFIG_DIR", home.join("team-opencode").to_string_lossy());
    let mut view = config(&env, Some(&project));
    // The Harness Skills sections follow these files (see `opencode_skills_config.rs`).
    view.sections.retain(|section| !section.id.contains("harnessSkills"));

    assert_eq!(
        section_files(&view, &home, &project),
        [
            "~/.config/opencode/opencode.jsonc",
            "~/.config/opencode/tui.json",
            "~/custom-opencode.jsonc",
            "<project>/opencode.json",
            "<project>/.opencode/opencode.jsonc",
            "<project>/.opencode/tui.json",
            "~/.opencode/opencode.json",
            "~/team-opencode/opencode.json",
        ]
    );

    use ConfigLayer::{Project, User};
    let layers: Vec<ConfigLayer> = view.sections.iter().map(|section| section.source.layer).collect();
    assert_eq!(layers, [User, User, User, Project, Project, Project, User, User]);
    assert_eq!(view.sections[0].source.format, ConfigFormat::Jsonc);
    assert_eq!(view.sections[3].source.format, ConfigFormat::Json);
    assert!(view.sections.iter().all(|section| section.error.is_none()));
}

#[test]
fn an_empty_home_lists_where_the_global_settings_would_go() {
    let temp = tempfile::TempDir::new().unwrap();
    let view = config(&env_at(temp.path()), None);

    assert_eq!(view.sections.len(), 1);
    assert_eq!(
        Path::new(&view.sections[0].source.path), // as paths: Windows mixes `/` and `\`
        temp.path().join(".config/opencode/opencode.json")
    );
    assert!(!view.sections[0].source.exists);
}

#[test]
fn env_references_show_the_variable_and_file_references_stay_literal() {
    let home = opencode_home();
    let view = config(&env_at(&home), None);
    let text = serde_json::to_string(&view.sections[0].entries).unwrap();

    let env_ref = serde_json::to_string(&ConfigValue::Secret(SecretKind::EnvRef {
        name: "ANTHROPIC_API_KEY".to_owned(),
    }))
    .unwrap();
    assert!(text.contains(&env_ref), "{{env:...}} is shown by name");
    assert!(
        text.contains("{file:./house-rules.md}"),
        "{{file:...}} is shown as written"
    );
}

#[test]
fn environment_overrides_are_named_without_their_values() {
    let home = opencode_home();
    let env = env_at(&home)
        .with_var("OPENCODE_PERMISSION", r#"{"bash":"allow"}"#)
        .with_var("OPENCODE_CONFIG_CONTENT", r#"{"model":"x/y"}"#);
    let view = config(&env, None);

    let warnings: Vec<&str> = view
        .notices
        .iter()
        .filter(|notice| notice.level == NoticeLevel::Warning)
        .map(|notice| notice.message.as_str())
        .collect();

    assert_eq!(
        warnings,
        [
            "OPENCODE_CONFIG_CONTENT is set in the environment and overrides these files. Its value is not shown.",
            "OPENCODE_PERMISSION is set in the environment and overrides the permissions in these files. Its value is not shown.",
        ]
    );
    assert!(
        warnings
            .iter()
            .all(|message| !message.contains("allow") && !message.contains("x/y"))
    );
}

#[test]
fn disabling_project_config_drops_project_files_and_resources() {
    let home = opencode_home();
    let project = demo_project();
    let env = env_at(&home).with_var("OPENCODE_DISABLE_PROJECT_CONFIG", "1");

    let view = config(&env, Some(&project));
    assert!(
        view.sections
            .iter()
            .all(|section| section.source.layer == ConfigLayer::User)
    );
    assert!(
        view.notices
            .iter()
            .any(|notice| notice.message.starts_with("OPENCODE_DISABLE_PROJECT_CONFIG"))
    );

    let extensions = extensions(&env, Some(&project));
    assert!(agent(&extensions, "migrator").is_none());
    assert_eq!(server(&extensions, "linear").enabled, true);
    assert!(
        extensions
            .prompt_templates
            .iter()
            .all(|template| template.name != "release")
    );
}

#[test]
fn mcp_servers_merge_by_name_and_hide_secrets() {
    let home = opencode_home();
    let project = demo_project();
    let view = extensions(&env_at(&home), Some(&project));

    let github = server(&view, "github");
    assert_eq!(github.transport, McpTransport::Stdio);
    assert_eq!(github.command.as_deref(), Some("npx"));
    assert_eq!(
        github.args,
        [
            ConfigValue::Text("-y".to_owned()),
            ConfigValue::Text("@modelcontextprotocol/server-github".to_owned()),
            ConfigValue::Text("--token=••••".to_owned()),
            ConfigValue::Text("--api-key".to_owned()),
            ConfigValue::Secret(SecretKind::Value),
        ]
    );

    let linear = server(&view, "linear");
    assert_eq!(linear.transport, McpTransport::Http);
    assert!(!linear.enabled, "the project switches it off");
    assert!(linear.source.path.ends_with("opencode.json"));
    assert_eq!(linear.source.layer, ConfigLayer::Project);
    assert_eq!(
        linear.url,
        Some(ConfigValue::Text("https://mcp.linear.app/mcp".to_owned()))
    );

    let internal = server(&view, "internal");
    assert_eq!(
        internal.url,
        Some(ConfigValue::Text(
            "https://••••@mcp.example.com/sse?key=••••".to_owned()
        ))
    );
    assert!(!internal.enabled);
}

#[test]
fn agents_merge_builtins_config_and_markdown() {
    let home = opencode_home();
    let project = demo_project();
    let env = env_at(&home).with_var("OPENCODE_CONFIG_DIR", home.join("team-opencode").to_string_lossy());
    let view = extensions(&env, Some(&project));

    assert_eq!(
        names(&view.agents, |agent| &agent.name),
        [
            "build",
            "plan",
            "general",
            "compaction",
            "title",
            "summary",
            "docs",
            "focus",
            "migrator",
            "reviewer",
            "triage",
        ]
    );

    let build = agent(&view, "build").unwrap();
    assert!(build.built_in);
    assert_eq!(build.scope, ExtensionScope::Builtin);
    assert_eq!(build.description.as_deref(), Some("Build agent with the house model."));

    let docs = agent(&view, "docs").unwrap();
    assert_eq!(docs.mode, AgentMode::Subagent);
    assert_eq!(docs.model.as_deref(), Some("anthropic/claude-haiku-4-5"));
    assert_eq!(docs.scope, ExtensionScope::Project);

    assert_eq!(agent(&view, "focus").unwrap().mode, AgentMode::Primary);
    assert_eq!(agent(&view, "triage").unwrap().mode, AgentMode::All);
    assert!(agent(&view, "title").unwrap().hidden);
    assert!(
        agent(&view, "explore").is_none(),
        "disable: true removes a built-in agent"
    );
}

#[test]
fn plugins_are_deduplicated_by_package_and_file() {
    let home = opencode_home();
    let project = demo_project();
    let view = extensions(&env_at(&home), Some(&project));

    let listed: Vec<(String, Option<String>, ExtensionScope)> = view
        .plugins
        .iter()
        .map(|plugin| (plugin.name.clone(), plugin.version.clone(), plugin.scope))
        .collect();

    assert_eq!(
        listed,
        [
            (
                "opencode-wakatime".to_owned(),
                Some("1.3.0".to_owned()),
                ExtensionScope::Project
            ),
            ("@acme/opencode-plugin".to_owned(), None, ExtensionScope::User),
            ("hello".to_owned(), None, ExtensionScope::User),
            ("guard".to_owned(), None, ExtensionScope::Project),
        ]
    );
}

#[test]
fn the_nearest_terminal_ui_theme_is_active() {
    let home = opencode_home();
    let project = demo_project();
    let env = env_at(&home);

    let active = |view: &ExtensionsView| -> Vec<String> {
        view.themes
            .iter()
            .filter(|theme| theme.active)
            .map(|theme| theme.name.clone())
            .collect()
    };

    assert_eq!(active(&extensions(&env, None)), ["dusk"]);
    assert_eq!(active(&extensions(&env, Some(&project))), ["demo-light"]);
}

#[test]
fn skills_come_from_config_dirs_and_claude_and_agents_dirs() {
    let home = opencode_home();
    let project = demo_project();
    let view = extensions(&env_at(&home), Some(&project));

    let mut found: Vec<(String, ExtensionScope)> = view
        .skills
        .iter()
        .map(|skill| (skill.name.clone(), skill.scope))
        .collect();
    found.sort_by(|left, right| left.0.cmp(&right.0));

    assert_eq!(
        found,
        [
            ("claude-notes".to_owned(), ExtensionScope::User),
            ("demo".to_owned(), ExtensionScope::User),
            ("plain-writing".to_owned(), ExtensionScope::User),
            ("release-notes".to_owned(), ExtensionScope::Project),
            ("shared-skill".to_owned(), ExtensionScope::Project),
        ]
    );
}

/// `(name, scope)` of every command, sorted.
fn commands(view: &ExtensionsView) -> Vec<(String, ExtensionScope)> {
    let mut found: Vec<(String, ExtensionScope)> = view
        .prompt_templates
        .iter()
        .map(|template| (template.name.clone(), template.scope))
        .collect();
    found.sort_by(|left, right| left.0.cmp(&right.0));

    found
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The layer of the section showing `file`.
fn layer_of(view: &ConfigView, file: &Path) -> ConfigLayer {
    let section = view
        .sections
        .iter()
        .find(|section| Path::new(&section.source.path) == file);

    section
        .unwrap_or_else(|| panic!("no section for {file:?}"))
        .source
        .layer
}

#[test]
fn a_repository_at_home_reads_the_home_settings_as_user_settings() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = home.join("app");
    std::fs::create_dir_all(home.join(".git")).unwrap();
    write(
        &home.join("opencode.json"),
        r#"{ "command": { "ship": { "template": "Ship it", "description": "Ship" } } }"#,
    );
    write(
        &home.join(".opencode/command/tidy.md"),
        "---\ndescription: Tidy\n---\nTidy up\n",
    );
    write(
        &temp.path().join(".opencode/command/outside.md"),
        "Beyond the repository\n",
    );
    let env = env_at(&home);

    let view = config(&env, Some(&project));
    assert_eq!(layer_of(&view, &home.join("opencode.json")), ConfigLayer::User);
    assert_eq!(
        commands(&extensions(&env, Some(&project))),
        [
            ("ship".to_owned(), ExtensionScope::User),
            ("tidy".to_owned(), ExtensionScope::User),
        ],
        "the walk stops at the repository root, and the home directory stays user configuration"
    );
}

#[test]
fn outside_a_repository_the_home_settings_are_user_settings() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let project = home.join("notes");
    std::fs::create_dir_all(&project).unwrap();
    write(&home.join("opencode.json"), r#"{ "theme": "dusk" }"#);
    write(&project.join("opencode.json"), r#"{ "theme": "dawn" }"#);
    let view = config(&env_at(&home), Some(&project));

    assert_eq!(layer_of(&view, &home.join("opencode.json")), ConfigLayer::User);
    assert_eq!(layer_of(&view, &project.join("opencode.json")), ConfigLayer::Project);
}

#[test]
fn without_a_repository_every_ancestor_opencode_dir_is_read_past_home() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let outer = home.join("work");
    let project = outer.join("notes").join("draft");
    std::fs::create_dir_all(&project).unwrap();
    write(&outer.join(".opencode/command/outer.md"), "From the outer folder\n");
    write(&temp.path().join(".opencode/command/top.md"), "From the top folder\n");

    let found = commands(&extensions(&env_at(&home), Some(&project)));

    // The walk goes on to the filesystem root, so only what the test wrote is checked.
    for name in ["outer", "top"] {
        assert!(
            found.contains(&(name.to_owned(), ExtensionScope::Project)),
            "{name} missing from {found:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn symlink_loops_in_agent_and_skill_dirs_are_not_followed() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::TempDir::new().unwrap();
    let project = temp.path().join("project");
    let agents = project.join(".opencode/agents");
    let skills = project.join(".opencode/skills");
    write(&agents.join("reviewer.md"), "---\ndescription: Reviews\n---\nReview\n");
    write(&agents.join("team/lead.md"), "---\ndescription: Leads\n---\nLead\n");
    write(&skills.join("notes/SKILL.md"), "---\ndescription: Notes\n---\n");
    for index in 0..50 {
        symlink(".", agents.join(format!("loop-{index}"))).unwrap();
        symlink(".", skills.join(format!("loop-{index}"))).unwrap();
    }

    let started = std::time::Instant::now();
    let view = extensions(&env_at(&temp.path().join("home")), Some(&project));
    let elapsed = started.elapsed();
    assert!(elapsed.as_secs_f32() < 2.0, "{elapsed:?}");

    let project_agents: Vec<&str> = view
        .agents
        .iter()
        .filter(|agent| agent.scope == ExtensionScope::Project)
        .map(|agent| agent.name.as_str())
        .collect();
    assert_eq!(project_agents, ["reviewer", "team/lead"]);
    assert_eq!(names(&view.skills, |skill| &skill.name), ["notes"]);
}
