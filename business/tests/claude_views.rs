//! The Claude Code config and extensions views over the synthetic fixture home (`claude-home`, with
//! managed settings under `Program Files` and a second config directory `claude-work`) and the demo
//! project. Extension details are covered in `claude_extensions.rs`.

mod common;

use std::path::Path;

use common::{CANARY, claude_home, demo_project, env_at, exported, json_text, write_file};
use harness_business::env::{Env, OsKind};
use harness_business::harness::Harness;
use harness_business::harnesses::Claude;
use harness_business::model::{
    ConfigField, ConfigLayer, ConfigSection, ConfigValue, ConfigView, NoticeLevel, SecretKind,
};
use pretty_assertions::assert_eq;
use serde_json::json;

fn config(env: &Env, project: Option<&Path>) -> ConfigView {
    Claude.config_view(env, project).unwrap()
}

/// The fixture home with its managed settings in reach, on every OS (`%ProgramFiles%\ClaudeCode`).
fn managed_env(home: &Path) -> Env {
    env_at(home)
        .with_os(OsKind::Windows)
        .with_var("ProgramFiles", home.join("Program Files").to_string_lossy())
}

fn ids(view: &ConfigView) -> Vec<&str> {
    view.sections.iter().map(|section| section.id.as_str()).collect()
}

fn section<'a>(view: &'a ConfigView, id: &str) -> &'a ConfigSection {
    view.sections
        .iter()
        .find(|section| section.id == id)
        .unwrap_or_else(|| panic!("section {id} missing"))
}

fn field<'a>(fields: &'a [ConfigField], key: &str) -> &'a ConfigValue {
    &fields
        .iter()
        .find(|field| field.key == key)
        .unwrap_or_else(|| panic!("field {key} missing"))
        .value
}

fn map(value: &ConfigValue) -> &[ConfigField] {
    match value {
        ConfigValue::Map(fields) => fields,
        other => panic!("expected a map, got {other:?}"),
    }
}

fn keys(fields: &[ConfigField]) -> Vec<&str> {
    fields.iter().map(|field| field.key.as_str()).collect()
}

#[test]
fn config_view_snapshots() {
    let home = claude_home();
    let project = demo_project();
    let env = managed_env(&home);

    insta::assert_json_snapshot!("claude_config_user", exported(config(&env, None), &home, &project));
    insta::assert_json_snapshot!(
        "claude_config_project",
        exported(config(&env, Some(&project)), &home, &project)
    );
}

#[test]
fn extensions_view_snapshots() {
    let home = claude_home();
    let project = demo_project();
    let env = env_at(&home);
    let view = |project: Option<&Path>| Claude.extensions_view(&env, project).unwrap();

    insta::assert_json_snapshot!("claude_extensions_user", exported(view(None), &home, &project));
    insta::assert_json_snapshot!(
        "claude_extensions_project",
        exported(view(Some(&project)), &home, &project)
    );
}

#[test]
fn sections_follow_managed_user_and_project_order() {
    let home = claude_home();
    let project = demo_project();
    let view = config(&managed_env(&home), Some(&project));

    assert_eq!(
        ids(&view),
        [
            "managed.settings",
            "managed.dropIn:10-team.json",
            "user.settings",
            "user.globalFile",
            "user.harnessSkillsModels",
            "user.harnessSkillsWorkflow",
            "project.settings",
            "project.localSettings",
        ]
    );
    assert_eq!(
        section(&view, "project.localSettings").source.layer,
        ConfigLayer::Project
    );
    assert_eq!(section(&view, "managed.settings").source.layer, ConfigLayer::User);
}

#[test]
fn missing_files_are_left_out() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();

    let view = config(&env_at(home.path()), Some(project.path()));

    assert!(view.sections.is_empty(), "{:?}", ids(&view));
}

#[test]
fn claude_secret_settings_are_hidden() {
    let home = claude_home();
    let view = config(&managed_env(&home), None);
    let user = &section(&view, "user.settings").entries;

    for helper in [
        "apiKeyHelper",
        "awsCredentialExport",
        "awsAuthRefresh",
        "otelHeadersHelper",
    ] {
        assert_eq!(
            field(user, helper),
            &ConfigValue::Secret(SecretKind::Command),
            "{helper}"
        );
    }

    let env = map(field(user, "env"));
    assert_eq!(keys(env), ["ANTHROPIC_BASE_URL", "GATEWAY_TOKEN"]);
    assert!(
        env.iter()
            .all(|entry| entry.value == ConfigValue::Secret(SecretKind::Value))
    );

    let plugin_options = json_text(field(user, "pluginConfigs"));
    assert!(plugin_options.contains("useful-skills@useful-skills-local"));
    assert!(
        !plugin_options.contains("\"3\""),
        "plugin options are hidden: {plugin_options}"
    );

    let dropped_in = &section(&view, "managed.dropIn:10-team.json").entries;
    assert_eq!(
        field(dropped_in, "apiKeyHelper"),
        &ConfigValue::Secret(SecretKind::Command)
    );
}

#[test]
fn the_global_file_shows_only_user_mcp_servers_with_secrets_hidden() {
    let home = claude_home();
    let view = config(&env_at(&home), None);
    let global = section(&view, "user.globalFile");

    assert_eq!(keys(&global.entries), ["mcpServers"]);
    assert!(global.source.path.ends_with(".claude.json"));

    let servers = map(field(&global.entries, "mcpServers"));
    let github = map(field(servers, "github"));
    let args = match field(github, "args") {
        ConfigValue::List(args) => args,
        other => panic!("expected args, got {other:?}"),
    };
    assert_eq!(args.last(), Some(&ConfigValue::Secret(SecretKind::Value)));
    assert!(
        map(field(github, "env"))
            .iter()
            .all(|entry| matches!(entry.value, ConfigValue::Secret(_)))
    );

    let linear = map(field(servers, "linear"));
    assert_eq!(
        map(field(linear, "headers"))[0].value,
        ConfigValue::Secret(SecretKind::Value)
    );
    assert!(!json_text(&view).contains(CANARY));
}

#[test]
fn the_project_entry_of_the_global_file_keeps_only_allow_listed_keys() {
    let home = tempfile::tempdir().unwrap();
    let project = demo_project();
    let other = format!("{CANARY}-elsewhere");
    let global = json!({
        "userID": CANARY,
        "oauthAccount": { "emailAddress": CANARY },
        "projects": {
            project.to_string_lossy(): {
                "allowedTools": ["Bash(bun run test)"],
                "mcpServers": { "local-db": { "command": "db-mcp", "env": { "DB_PASSWORD": CANARY } } },
                "enabledMcpjsonServers": ["docs"],
                "disabledMcpjsonServers": [],
                "disabledMcpServers": ["linear"],
                "hasTrustDialogAccepted": true,
                "history": [{ "display": CANARY }],
                "exampleFiles": [CANARY],
                "lastSessionId": CANARY,
            },
            "/somewhere/else": { "allowedTools": [other] },
        },
    });
    write_file(&home.path().join(".claude.json"), &global.to_string());

    let view = config(&env_at(home.path()), Some(&project));
    let entry = section(&view, "project.globalFile");

    assert_eq!(entry.source.layer, ConfigLayer::Project);
    assert_eq!(
        keys(&entry.entries),
        [
            "allowedTools",
            "mcpServers",
            "enabledMcpjsonServers",
            "disabledMcpjsonServers",
            "disabledMcpServers",
            "hasTrustDialogAccepted",
        ]
    );
    assert_eq!(keys(&section(&view, "user.globalFile").entries), Vec::<&str>::new());
    assert!(!json_text(&view).contains(CANARY));
}

#[test]
fn unreadable_files_report_a_location_only_error() {
    let home = tempfile::tempdir().unwrap();
    write_file(
        &home.path().join(".claude.json"),
        &format!("{{\"userID\": \"{CANARY}\""),
    );
    write_file(&home.path().join(".claude/settings.json"), "[1, 2]");

    let view = config(&env_at(home.path()), None);

    assert!(
        section(&view, "user.globalFile")
            .error
            .as_deref()
            .unwrap()
            .starts_with("Invalid JSON at line")
    );
    assert_eq!(
        section(&view, "user.settings").error.as_deref(),
        Some("Expected a key/value object at the top level")
    );
    assert!(!json_text(&view).contains(CANARY));
}

#[test]
fn a_renamed_config_directory_moves_settings_and_the_global_file() {
    let home = claude_home();
    let env = env_at(&home).with_var("CLAUDE_CONFIG_DIR", home.join("claude-work").to_string_lossy());
    let view = config(&env, None);

    assert_eq!(ids(&view), ["user.settings", "user.globalFile"]);
    assert!(section(&view, "user.settings").source.path.contains("claude-work"));
    assert!(section(&view, "user.globalFile").source.path.contains("claude-work"));
    assert!(json_text(&view).contains("claude-haiku-4-5"));
}

#[test]
fn login_variables_are_reported_by_name_and_provider_switches_as_set() {
    let home = tempfile::tempdir().unwrap();
    let env = env_at(home.path())
        .with_var("ANTHROPIC_API_KEY", CANARY)
        .with_var("ANTHROPIC_CUSTOM_HEADERS", CANARY)
        .with_var("CLAUDE_CODE_USE_BEDROCK", "1")
        .with_var("CLAUDE_CODE_USE_VERTEX", "0");

    let view = config(&env, None);
    let messages: Vec<&str> = view.notices.iter().map(|notice| notice.message.as_str()).collect();

    assert!(
        messages
            .iter()
            .any(|message| message.starts_with("ANTHROPIC_API_KEY is set"))
    );
    assert!(
        messages
            .iter()
            .any(|message| message.starts_with("ANTHROPIC_CUSTOM_HEADERS is set"))
    );
    assert!(messages.iter().any(|message| message.contains("Amazon Bedrock")));
    assert!(!messages.iter().any(|message| message.contains("Vertex")));
    assert_eq!(view.notices[0].level, NoticeLevel::Warning);
    assert!(!json_text(&view).contains(CANARY));
}

#[test]
fn memory_files_are_listed_by_name_only() {
    let home = claude_home();
    let project = demo_project();
    let view = config(&env_at(&home), Some(&project));

    let memory = view
        .notices
        .iter()
        .find(|notice| notice.message.contains("memory files"))
        .expect("a memory files notice");

    assert_eq!(
        memory.message,
        "Claude Code also loads these memory files, whose text is not shown here: ~/.claude/CLAUDE.md (user), \
         CLAUDE.md (project), CLAUDE.local.md (project, local), .claude/CLAUDE.md (project)."
    );
}
