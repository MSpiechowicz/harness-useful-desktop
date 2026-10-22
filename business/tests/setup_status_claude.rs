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

/// The user settings switching the skills plugin on, as `claude plugin install` leaves them.
fn switched_on(config: &Path) {
    write_json(
        &config.join("settings.json"),
        &json!({ "enabledPlugins": { "useful-skills@useful-skills-local": true } }),
    );
}

fn enable() -> SkillsSetup {
    SkillsSetup::Install {
        actions: vec![ExtensionAction::PluginEnable {
            name: "useful-skills@useful-skills-local".to_owned(),
        }],
    }
}

#[tokio::test]
async fn the_skills_plugin_of_the_user_scope_counts_with_its_version() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join(".claude");
    installed_plugins(
        &config,
        json!([{ "scope": "user", "installPath": "/x", "version": "0.3.1" }]),
    );
    switched_on(&config);

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
    // Registered from the skills' repository, under any name.
    let registries = [
        json!({ "useful-skills-local": {
            "source": { "source": "github", "repo": "MSpiechowicz/harness-useful-skills" },
            "installLocation": "/x"
        } }),
        json!({ "mine": { "source": { "source": "git", "url": "https://github.com/mspiechowicz/harness-useful-skills.git" } } }),
        json!({ "mine": { "source": { "source": "url", "url": "https://github.com/MSpiechowicz/harness-useful-skills/" } } }),
        json!({ "mine": { "source": { "source": "git", "url": "git@github.com:MSpiechowicz/harness-useful-skills.git" } } }),
        json!({ "mine": { "source": { "source": "git", "url": "ssh://git@github.com/MSpiechowicz/harness-useful-skills" } } }),
        json!({ "mine": { "source": { "source": "url", "url": "http://www.github.com/MSpiechowicz/harness-useful-skills" } } }),
        json!({ "mine": { "source": { "source": "git", "url": "github.com/MSpiechowicz/harness-useful-skills.git" } } }),
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

/// A marketplace that only takes the skills' marketplace name, or whose source merely contains the
/// repository's name, is someone else's: the skills' own is still added.
#[tokio::test]
async fn a_marketplace_of_the_same_name_from_elsewhere_does_not_count() {
    let registries = [
        json!({ "useful-skills-local": { "source": { "source": "github", "repo": "someone/useful-skills" } } }),
        json!({ "useful-skills-local": { "source": { "source": "directory", "path": "/tmp/fake" } } }),
        json!({ "fork": { "source": { "source": "git", "url": "https://evil.example/MSpiechowicz/harness-useful-skills.git" } } }),
        json!({ "fork": { "source": { "source": "github", "repo": "MSpiechowicz/harness-useful-skills-fork" } } }),
        json!({ "fork": { "source": { "source": "git", "url": "git@gitlab.com:MSpiechowicz/harness-useful-skills.git" } } }),
        json!({ "fork": { "source": { "source": "url", "url": "https://MSpiechowicz/harness-useful-skills" } } }),
    ];

    for registry in registries {
        let home = tempfile::tempdir().unwrap();
        write_json(&home.path().join(".claude/plugins/known_marketplaces.json"), &registry);

        let setup = claude(&Env::for_tests(home.path())).await;

        assert_eq!(
            setup.skills,
            SkillsSetup::Install {
                actions: vec![
                    ExtensionAction::MarketplaceAdd {
                        source: "MSpiechowicz/harness-useful-skills".to_owned()
                    },
                    install()
                ]
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

    assert_eq!(setup.skills, enable());
}

/// The extensions view's rule: only `enabledPlugins` set to `true` in the merged settings switches
/// a plugin on, so a plugin no layer names, or one managed settings switch off, is offered its
/// enable.
#[tokio::test]
async fn a_plugin_the_settings_do_not_switch_on_is_offered_its_enable() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join(".claude");
    installed_plugins(&config, json!([{ "scope": "user", "version": "0.3.1" }]));

    let env = Env::for_tests(home.path());
    assert_eq!(claude(&env).await.skills, enable(), "named nowhere");

    switched_on(&config);
    let managed = home.path().join("Program Files/ClaudeCode");
    write_json(
        &managed.join("managed-settings.json"),
        &json!({ "enabledPlugins": { "useful-skills@useful-skills-local": false } }),
    );
    let env = env
        .with_os(harness_business::env::OsKind::Windows)
        .with_var("ProgramFiles", home.path().join("Program Files").to_string_lossy());
    assert_eq!(claude(&env).await.skills, enable(), "switched off by managed settings");
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
    switched_on(&custom);
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
