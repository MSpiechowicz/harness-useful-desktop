//! The Codex config and extensions views over the fixture home (`tests/fixtures/codex-home`) and
//! small homes of their own: `config.toml` in source order with its secrets hidden, the project
//! layer, and the plugins, marketplaces, skills, MCP servers, rules and instruction files Codex
//! uses. The configured mode and the trust lookup are in `codex_trust_views`.

use crate::codex_trust_views::{real, trust_entry};
use crate::common;

use std::path::Path;

use common::{CANARY, codex_home, demo_project, env_at, json_text};
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::Codex;
use harness_business::model::{ConfigFormat, ConfigLayer, ConfigValue, ExtensionScope, McpTransport, SecretKind};
use pretty_assertions::assert_eq;

pub(crate) fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A marketplace's name, source type, source and catalog entries (name, installed).
type Market<'a> = (&'a str, &'a str, &'a str, Vec<(&'a str, bool)>);

fn messages(notices: &[harness_business::model::Notice]) -> Vec<String> {
    notices.iter().map(|notice| notice.message.clone()).collect()
}

#[test]
fn the_config_view_shows_both_layers_in_source_order_with_secrets_hidden() {
    let home = codex_home();
    let project = demo_project();
    let view = Codex.config_view(&env_at(&home), Some(&project)).unwrap();

    let ids: Vec<&str> = view.sections.iter().map(|section| section.id.as_str()).collect();
    assert_eq!(ids, ["user.config", "project.config"]);

    let user = &view.sections[0];
    assert_eq!(user.source.format, ConfigFormat::Toml);
    assert_eq!(user.source.layer, ConfigLayer::User);
    assert_eq!(
        user.source.path,
        home.join(".codex").join("config.toml").to_string_lossy()
    );
    let keys: Vec<&str> = user.entries.iter().map(|entry| entry.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "model",
            "model_reasoning_effort",
            "approval_policy",
            "sandbox_mode",
            "notify",
            "shell_environment_policy",
            "model_providers",
            "mcp_servers",
            "marketplaces",
            "plugins",
            "features",
            "tui"
        ]
    );
    assert_eq!(view.sections[1].source.layer, ConfigLayer::Project);

    let text = json_text(&view);
    assert!(!text.contains(CANARY), "a secret leaked: {text}");
    assert!(text.contains("gpt-5-codex"));
    assert!(text.contains("https://docs.example.com/mcp"));
    assert!(text.contains("DOCS_TOKEN"), "a variable name is shown");
    assert!(text.contains("acme/demo-market"));

    let notices = messages(&view.notices);
    assert!(notices.contains(
        &"Codex is configured with model gpt-5-codex, reasoning effort high, approval policy on-request, sandbox workspace-write."
            .to_owned()
    ));
    assert!(
        notices
            .iter()
            .any(|notice| notice.contains("does not trust this folder"))
    );
    assert!(notices.iter().any(|notice| notice.contains("never read")));
}

#[test]
fn a_renamed_home_is_read_and_api_key_variables_are_named_only() {
    let home = codex_home();
    let env = env_at(&home)
        .with_var("CODEX_HOME", home.join("codex-work").to_string_lossy())
        .with_var("OPENAI_API_KEY", CANARY);
    let view = Codex.config_view(&env, None).unwrap();

    assert_eq!(view.sections.len(), 1);
    assert_eq!(
        view.sections[0].source.path,
        home.join("codex-work").join("config.toml").to_string_lossy()
    );

    let text = json_text(&view);
    assert!(!text.contains(CANARY));
    assert!(text.contains("OPENAI_API_KEY is set in the environment"));
    assert!(text.contains("gpt-5-mini"));
}

#[test]
fn a_broken_config_reports_a_location_only() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".codex/config.toml"),
        &format!("model = \"gpt-5\"\nsecret = {CANARY}\n"),
    );

    let view = Codex.config_view(&Env::for_tests(home.path()), None).unwrap();

    let error = view.sections[0].error.clone().unwrap();
    assert!(error.starts_with("Invalid TOML at line 2"), "{error}");
    assert!(!json_text(&view).contains(CANARY));
}

#[test]
fn a_full_access_sandbox_alone_runs_unattended() {
    let home = tempfile::tempdir().unwrap();
    write(
        &home.path().join(".codex/config.toml"),
        "sandbox_mode = \"danger-full-access\"\n",
    );

    let native = Codex.native_approval_mode(&Env::for_tests(home.path()), None).unwrap();

    assert_eq!(native.name, "on-request / danger-full-access");
    assert!(native.unattended);
    assert_eq!(Codex.configured_approval_mode(&Env::for_tests(home.path()), None), None);
}

#[test]
fn the_extensions_view_lists_plugins_marketplaces_skills_and_servers() {
    let home = codex_home();
    let project = demo_project();
    let view = Codex.extensions_view(&env_at(&home), Some(&project)).unwrap();

    let plugins: Vec<(&str, Option<&str>, bool)> = view
        .plugins
        .iter()
        .map(|plugin| (plugin.id.as_str(), plugin.version.as_deref(), plugin.enabled))
        .collect();
    assert_eq!(
        plugins,
        [
            ("lint-tools@demo-market", Some("1.0.0"), true),
            ("pdf@openai-bundled", Some("2.0.0"), false),
            ("ghost@demo-market", None, true),
        ]
    );
    assert_eq!(
        view.plugins[0].description.as_deref(),
        Some("Linters and formatters for every project.")
    );
    assert!(view.plugins.iter().all(|plugin| plugin.scope == ExtensionScope::User));

    let marketplaces: Vec<Market<'_>> = view
        .marketplaces
        .iter()
        .map(|market| {
            let entries = market
                .entries
                .iter()
                .map(|entry| (entry.name.as_str(), entry.installed))
                .collect();
            (
                market.name.as_str(),
                market.source_type.as_str(),
                market.source.as_str(),
                entries,
            )
        })
        .collect();
    assert_eq!(
        marketplaces,
        [
            (
                "demo-market",
                "git",
                "acme/demo-market",
                vec![("lint-tools", true), ("format-tools", false)]
            ),
            (
                "openai-bundled",
                "local",
                "~/.codex/.tmp/bundled-marketplaces/openai-bundled",
                vec![("pdf", true)]
            ),
        ]
    );

    let refreshable: Vec<(&str, bool)> = view
        .marketplaces
        .iter()
        .map(|market| (market.name.as_str(), market.refreshable))
        .collect();
    assert_eq!(
        refreshable,
        [("demo-market", true), ("openai-bundled", false)],
        "only a git marketplace can be upgraded, a local one (the built-in openai-bundled) is refused by Codex"
    );

    let skills: Vec<(&str, ExtensionScope)> = view
        .skills
        .iter()
        .map(|skill| (skill.name.as_str(), skill.scope))
        .collect();
    assert_eq!(
        skills,
        [
            ("explain", ExtensionScope::User),
            ("shared-skill", ExtensionScope::Project),
            ("imagegen", ExtensionScope::Builtin),
            ("lint-tools:lint", ExtensionScope::Plugin),
        ],
        "a switched-off plugin contributes nothing"
    );

    let servers: Vec<(&str, McpTransport, bool)> = view
        .mcp_servers
        .iter()
        .map(|server| (server.name.as_str(), server.transport, server.enabled))
        .collect();
    assert_eq!(
        servers,
        [
            ("github", McpTransport::Stdio, true),
            ("docs", McpTransport::Http, true),
            ("paused", McpTransport::Stdio, false),
            ("plugin:lint-tools:lint", McpTransport::Stdio, true),
        ],
        "an untrusted project's servers are not Codex's"
    );
    let docs = &view.mcp_servers[1];
    let token = docs
        .details
        .iter()
        .find(|field| field.key == "bearer_token_env_var")
        .unwrap();
    assert_eq!(
        token.value,
        ConfigValue::Secret(SecretKind::EnvRef {
            name: "DOCS_TOKEN".to_owned()
        })
    );

    let notices = messages(&view.notices);
    assert!(notices.iter().any(|notice| notice.contains("ghost@demo-market")));
    assert!(notices.iter().any(|notice| notice.contains("default.rules")));
    assert!(
        notices
            .iter()
            .any(|notice| notice.contains("~/.codex/AGENTS.md (user)") && notice.contains("AGENTS.md (project)"))
    );

    assert!(!json_text(&view).contains(CANARY));
}

#[test]
fn a_trusted_projects_servers_come_first_and_win() {
    let home = tempfile::tempdir().unwrap();
    let project = real(home.path()).join("work/app");
    std::fs::create_dir_all(project.join(".git")).unwrap();
    write(
        &home.path().join(".codex/config.toml"),
        &format!(
            "[mcp_servers.tools]\ncommand = \"user-tools\"\n\n{}",
            trust_entry(&project)
        ),
    );
    write(
        &project.join(".codex/config.toml"),
        "[mcp_servers.tools]\ncommand = \"project-tools\"\n\n[mcp_servers.extra]\nurl = \"https://extra.example.com/mcp\"\n",
    );

    let view = Codex
        .extensions_view(&Env::for_tests(home.path()), Some(&project))
        .unwrap();
    let servers: Vec<(&str, Option<&str>, ConfigLayer)> = view
        .mcp_servers
        .iter()
        .map(|server| (server.name.as_str(), server.command.as_deref(), server.source.layer))
        .collect();

    assert_eq!(
        servers,
        [
            ("tools", Some("project-tools"), ConfigLayer::Project),
            ("extra", None, ConfigLayer::Project),
        ]
    );
}

#[test]
fn a_plugin_folder_outside_the_cache_is_not_read() {
    let home = tempfile::tempdir().unwrap();
    let outside = home.path().join("elsewhere/demo");
    write(
        &outside.join("1.0.0/.codex-plugin/plugin.json"),
        &format!("{{ \"version\": \"1.0.0\", \"description\": \"{CANARY}\" }}"),
    );
    write(
        &outside.join("1.0.0/skills/leak/SKILL.md"),
        &format!("---\nname: leak\ndescription: {CANARY}\n---\n"),
    );
    let market = home.path().join(".codex/plugins/cache/market");
    std::fs::create_dir_all(&market).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, market.join("demo")).unwrap();
    write(
        &home.path().join(".codex/config.toml"),
        "[plugins.\"demo@market\"]\nenabled = true\n",
    );

    let view = Codex.extensions_view(&Env::for_tests(home.path()), None).unwrap();

    assert_eq!(view.plugins.len(), 1);
    #[cfg(unix)]
    assert_eq!(view.plugins[0].path, None, "the linked folder is outside the cache");
    assert!(view.skills.is_empty());
    assert!(!json_text(&view).contains(CANARY));
}

#[test]
fn codex_declares_its_views_actions_and_sharing() {
    let capabilities = Codex.capabilities();

    assert!(capabilities.plugins && capabilities.marketplaces && capabilities.mcp && capabilities.skills);
    assert!(capabilities.cli_actions);
    assert!(capabilities.chat && capabilities.chat_preview, "chat is a preview");
    assert!(capabilities.remove_queued && !capabilities.compact_instructions);

    let share = Codex.share_support();
    assert!(share.marketplaces && share.plugins && share.mcp_servers);
    assert!(!share.packages);
}
