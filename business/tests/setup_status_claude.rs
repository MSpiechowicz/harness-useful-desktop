//! What the "Recommended setup" step offers for Claude Code: the Harness Skills plugin from its
//! marketplace (read from Claude's plugin registries) and the MCP states of the companion servers
//! in the user scope of its global file, which is only read. The empty machine is in
//! `setup_status.rs`.

mod claude_support;

use std::path::Path;

use claude_support::{stdio, write_json};
use harness_business::ecosystem::setup_status;
use harness_business::env::Env;
use harness_business::harness::HarnessRegistry;
use harness_business::model::{
    ExtensionAction, HarnessId, HarnessSetup, InstallScope, McpRefusal, McpState, SkillsSetup,
};
use pretty_assertions::assert_eq;
use serde_json::json;

mod setup_support;

use setup_support::CountingRunner;

async fn claude(env: &Env) -> HarnessSetup {
    let setup = setup_status(env, &CountingRunner::default(), &HarnessRegistry::with_defaults(), &[]).await;

    setup
        .harnesses
        .into_iter()
        .find(|entry| entry.harness == HarnessId::Claude)
        .expect("Claude Code")
}

fn install() -> ExtensionAction {
    ExtensionAction::PluginInstall {
        target: "useful-skills@useful-skills-local".to_owned(),
        scope: InstallScope::User,
    }
}

fn installed_plugins(config: &Path, entries: serde_json::Value) {
    write_json(
        &config.join("plugins/installed_plugins.json"),
        &json!({ "version": 2, "plugins": { "useful-skills@useful-skills-local": entries } }),
    );
}

#[tokio::test]
async fn the_skills_plugin_of_the_user_scope_counts_with_its_version() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join(".claude");
    installed_plugins(
        &config,
        json!([{ "scope": "user", "installPath": "/x", "version": "0.3.1" }]),
    );

    let setup = claude(&Env::for_tests(home.path())).await;

    assert_eq!(
        setup.skills,
        SkillsSetup::Installed {
            version: Some("0.3.1".to_owned())
        }
    );
}

#[tokio::test]
async fn a_project_install_does_not_count_and_a_registered_marketplace_is_not_added_again() {
    // Registered by the name the skills' marketplace gives itself, or from the skills' repository
    // under any name.
    let registries = [
        json!({ "useful-skills-local": {
            "source": { "source": "github", "repo": "MSpiechowicz/harness-useful-skills" },
            "installLocation": "/x"
        } }),
        json!({ "mine": { "source": { "source": "git", "url": "https://github.com/mspiechowicz/harness-useful-skills.git" } } }),
    ];

    for registry in registries {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join(".claude");
        installed_plugins(
            &config,
            json!([{ "scope": "project", "projectPath": "/w/demo", "version": "0.3.1" }]),
        );
        write_json(&config.join("plugins/known_marketplaces.json"), &registry);

        let setup = claude(&Env::for_tests(home.path())).await;

        assert_eq!(
            setup.skills,
            SkillsSetup::Install {
                actions: vec![install()]
            },
            "{registry}"
        );
    }
}

#[tokio::test]
async fn a_switched_off_plugin_is_offered_its_enable() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join(".claude");
    installed_plugins(&config, json!([{ "scope": "user", "version": "0.3.1" }]));
    write_json(
        &config.join("settings.json"),
        &json!({ "enabledPlugins": { "useful-skills@useful-skills-local": false } }),
    );

    let setup = claude(&Env::for_tests(home.path())).await;

    assert_eq!(
        setup.skills,
        SkillsSetup::Install {
            actions: vec![ExtensionAction::PluginEnable {
                name: "useful-skills@useful-skills-local".to_owned()
            }]
        }
    );
}

#[tokio::test]
async fn servers_of_the_user_scope_are_read_from_the_global_file() {
    let home = tempfile::tempdir().unwrap();
    let global = home.path().join(".claude.json");
    write_json(
        &global,
        &json!({
            "numStartups": 12,
            "mcpServers": {
                "harness-dashboard": stdio("harness-dashboard"),
                "notes": stdio(&home.path().join(".local/bin/harness-memory").to_string_lossy())
            },
            "projects": { "/w/demo": { "mcpServers": { "harness-desktop": stdio("/opt/harness-desktop") } } }
        }),
    );
    let text = std::fs::read_to_string(&global).unwrap();

    let setup = claude(&Env::for_tests(home.path())).await;

    let at = |name: &str| McpState::Configured {
        file: "~/.claude.json".to_owned(),
        name: name.to_owned(),
    };
    assert_eq!(setup.mcp, at("harness-dashboard"));
    assert_eq!(setup.memory_mcp, at("notes"));
    assert_eq!(
        setup.desktop_mcp,
        McpState::Missing {
            file: "~/.claude.json".to_owned(),
            exists: true
        },
        "a project's local servers do not count"
    );
    assert_eq!(std::fs::read_to_string(&global).unwrap(), text, "only read");
}

#[tokio::test]
async fn claude_config_dir_moves_the_registries_and_the_global_file() {
    let home = tempfile::tempdir().unwrap();
    let custom = home.path().join("claude-work");
    installed_plugins(&custom, json!([{ "scope": "user" }]));
    write_json(
        &custom.join(".claude.json"),
        &json!({ "mcpServers": { "harness-memory": stdio("/x/harness-memory") } }),
    );

    let env = Env::for_tests(home.path()).with_var("CLAUDE_CONFIG_DIR", custom.to_string_lossy());
    let setup = claude(&env).await;

    assert_eq!(setup.skills, SkillsSetup::Installed { version: None });
    assert_eq!(
        setup.memory_mcp,
        McpState::Configured {
            file: "~/claude-work/.claude.json".to_owned(),
            name: "harness-memory".to_owned()
        }
    );
}

#[tokio::test]
async fn an_unreadable_global_file_is_refused() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join(".claude.json"), "{ not json").unwrap();

    let setup = claude(&Env::for_tests(home.path())).await;

    assert_eq!(
        setup.mcp,
        McpState::Refused {
            file: "~/.claude.json".to_owned(),
            reason: McpRefusal::Unparsable
        }
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlinked_global_file_is_followed_since_it_is_only_read() {
    let home = tempfile::tempdir().unwrap();
    let real = home.path().join("dotfiles/claude.json");
    write_json(
        &real,
        &json!({ "mcpServers": { "harness-dashboard": stdio("harness-dashboard") } }),
    );
    std::os::unix::fs::symlink(&real, home.path().join(".claude.json")).unwrap();

    let setup = claude(&Env::for_tests(home.path())).await;

    assert_eq!(
        setup.mcp,
        McpState::Configured {
            file: "~/.claude.json".to_owned(),
            name: "harness-dashboard".to_owned()
        }
    );
}
