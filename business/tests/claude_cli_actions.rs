//! Claude Code extension actions through `claude plugin …`: the argv of each action (user scope,
//! `--json`, never `-y`), target validation, a dry run that runs nothing, hostile values that
//! spawn nothing, and the summary taken from the JSON result line (a recording stand-in for
//! `claude`, never the real binary).

mod claude_support;

use std::path::Path;

use claude_support::{FakeClaude, args, binary, exit};
use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::{CliRun, Harness, run_cli_action};
use harness_business::harnesses::Claude;
use harness_business::model::{ExtensionAction, InstallScope};
use pretty_assertions::assert_eq;

const SKILLS: &str = "useful-skills@useful-skills-local";

fn argv(action: &ExtensionAction, project: Option<&Path>) -> Vec<String> {
    let invocation = Claude
        .cli_action(action, project)
        .unwrap_or_else(|error| panic!("{action:?}: {error}"));

    assert_eq!(invocation.dry_run_flag, None, "Claude Code has no preview flag");
    invocation.args
}

fn name() -> String {
    SKILLS.to_owned()
}

#[test]
fn plugin_actions_run_in_the_user_scope_with_a_json_result() {
    use ExtensionAction::*;

    let project = Path::new("/home/demo/Projekty/demo");
    let cases: Vec<(ExtensionAction, Option<&Path>, Vec<&str>)> = vec![
        (
            PluginInstall {
                target: name(),
                scope: InstallScope::User,
            },
            None,
            vec!["plugin", "install", SKILLS, "--scope", "user", "--json"],
        ),
        (
            PluginInstall {
                target: name(),
                scope: InstallScope::Project,
            },
            Some(project),
            vec!["plugin", "install", SKILLS, "--scope", "project", "--json"],
        ),
        (
            PluginUninstall { name: name() },
            None,
            vec!["plugin", "uninstall", SKILLS, "--scope", "user", "--json"],
        ),
        (
            PluginEnable { name: name() },
            None,
            vec!["plugin", "enable", SKILLS, "--scope", "user", "--json"],
        ),
        (
            PluginDisable { name: name() },
            None,
            vec!["plugin", "disable", SKILLS, "--scope", "user", "--json"],
        ),
        (
            PluginUpgrade { name: Some(name()) },
            None,
            vec!["plugin", "update", SKILLS, "--scope", "user", "--json"],
        ),
    ];

    for (action, project, expected) in cases {
        assert_eq!(argv(&action, project), expected, "{action:?}");
    }
}

#[test]
fn marketplace_actions_use_the_user_scope_and_json_where_help_offers_it() {
    use ExtensionAction::*;

    let cases: Vec<(ExtensionAction, Vec<&str>)> = vec![
        (
            MarketplaceAdd {
                source: "MSpiechowicz/harness-useful-skills".to_owned(),
            },
            vec![
                "plugin",
                "marketplace",
                "add",
                "MSpiechowicz/harness-useful-skills",
                "--scope",
                "user",
                "--json",
            ],
        ),
        (
            MarketplaceAdd {
                source: "https://github.com/MSpiechowicz/harness-useful-skills.git".to_owned(),
            },
            vec![
                "plugin",
                "marketplace",
                "add",
                "https://github.com/MSpiechowicz/harness-useful-skills.git",
                "--scope",
                "user",
                "--json",
            ],
        ),
        (
            MarketplaceRemove {
                name: "useful-skills-local".to_owned(),
            },
            vec![
                "plugin",
                "marketplace",
                "remove",
                "useful-skills-local",
                "--scope",
                "user",
                "--json",
            ],
        ),
        (
            MarketplaceUpdate {
                name: Some("useful-skills-local".to_owned()),
            },
            vec!["plugin", "marketplace", "update", "useful-skills-local", "--json"],
        ),
        (
            MarketplaceUpdate { name: None },
            vec!["plugin", "marketplace", "update"],
        ),
    ];

    for (action, expected) in cases {
        assert_eq!(argv(&action, None), expected, "{action:?}");
    }
}

#[cfg(unix)]
#[test]
fn an_absolute_path_is_a_marketplace_source() {
    let action = ExtensionAction::MarketplaceAdd {
        source: "/home/demo/harness-useful-skills".to_owned(),
    };

    assert_eq!(argv(&action, None)[3], "/home/demo/harness-useful-skills");
}

#[test]
fn hostile_or_unknown_values_are_refused_without_echoing_them() {
    use ExtensionAction::*;

    let plugins = [
        "",
        "-y",
        "--yes",
        "a b",
        "a\nb",
        "x;rm",
        "a`b`",
        "$(x)",
        "https://user:pass@host/x",
    ];
    let sources = [
        "",
        "-y",
        "./local",
        "relative/dir/deeper",
        "http://example.com/m.git",
        "https://",
        "https:///x",
        "git@github.com:a/b.git",
        "file:///x",
        "owner/../x",
        "owner/..",
        "https://user:pass@example.com/m.git",
        "a b/c",
    ];

    let mut actions: Vec<ExtensionAction> = Vec::new();
    for value in plugins {
        let value = value.to_owned();
        actions.push(PluginInstall {
            target: value.clone(),
            scope: InstallScope::User,
        });
        actions.push(PluginUninstall { name: value.clone() });
        actions.push(PluginUpgrade {
            name: Some(value.clone()),
        });
        actions.push(MarketplaceRemove { name: value });
    }
    for source in sources {
        actions.push(MarketplaceAdd {
            source: source.to_owned(),
        });
    }

    for action in actions {
        let error = Claude.cli_action(&action, None).expect_err(&format!("{action:?}"));
        assert!(matches!(error, BusinessError::InvalidInput(_)), "{action:?}: {error:?}");

        let shown = format!("{action:?}");
        for value in ["x;rm", "a`b`", "$(x)", "user:pass", "git@github.com:a/b.git"] {
            if shown.contains(value) {
                assert!(!error.to_string().contains(value), "{value:?} is echoed: {error}");
            }
        }
    }
}

#[test]
fn packages_and_updating_every_plugin_are_unsupported() {
    use ExtensionAction::*;

    for action in [
        PackageInstall {
            source: "npm:demo".to_owned(),
        },
        PackageRemove {
            source: "npm:demo".to_owned(),
        },
        PackageUpdate { source: None },
        PluginUpgrade { name: None },
    ] {
        let error = Claude.cli_action(&action, None).expect_err(&format!("{action:?}"));
        assert!(matches!(error, BusinessError::Unsupported(_)), "{action:?}: {error:?}");
    }
}

#[test]
fn a_project_install_needs_a_project() {
    let action = ExtensionAction::PluginInstall {
        target: name(),
        scope: InstallScope::Project,
    };

    let error = Claude.cli_action(&action, None).expect_err("no project");
    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
}

#[test]
fn claude_declares_cli_actions() {
    assert!(Claude.capabilities().cli_actions);
}

fn install() -> ExtensionAction {
    ExtensionAction::PluginInstall {
        target: name(),
        scope: InstallScope::User,
    }
}

async fn run(
    fake: &FakeClaude,
    home: &Path,
    action: &ExtensionAction,
    dry_run: bool,
) -> harness_business::Result<harness_business::model::CliResult> {
    let env = Env::for_tests(home);
    let claude = binary(home);

    run_cli_action(CliRun {
        harness: &Claude,
        env: &env,
        runner: fake,
        binary: &claude,
        action,
        dry_run,
        project: None,
    })
    .await
}

#[tokio::test]
async fn a_dry_run_reports_the_command_and_runs_nothing() {
    let home = tempfile::tempdir().unwrap();
    let fake = FakeClaude::new(&home.path().join(".claude.json"));

    let result = run(&fake, home.path(), &install(), true).await.unwrap();

    assert!(fake.argvs().is_empty(), "nothing runs");
    assert!(result.ok);
    assert_eq!(result.exit_code, None);
    assert_eq!(
        result.command,
        ["claude", "plugin", "install", SKILLS, "--scope", "user", "--json"]
    );
    assert_eq!(result.argv, result.command);
}

#[tokio::test]
async fn hostile_values_spawn_nothing() {
    let home = tempfile::tempdir().unwrap();
    let fake = FakeClaude::new(&home.path().join(".claude.json"));

    for dry_run in [false, true] {
        let hostile = ExtensionAction::MarketplaceAdd {
            source: "--help".to_owned(),
        };
        assert!(run(&fake, home.path(), &hostile, dry_run).await.is_err());
    }

    assert!(fake.argvs().is_empty(), "nothing runs");
}

#[tokio::test]
async fn a_run_reports_the_json_message_and_the_exit_code_decides() {
    let home = tempfile::tempdir().unwrap();
    let fake = FakeClaude::new(&home.path().join(".claude.json"));

    let line = r#"{"command":"install","outcome":"ok","plugin":"useful-skills@useful-skills-local","scope":"user","message":"✔ Successfully installed plugin: useful-skills@useful-skills-local (scope: user)\nRestart to apply."}"#;
    fake.answer(exit(0, &format!("{line}\n"), ""));
    let done = run(&fake, home.path(), &install(), false).await.unwrap();

    assert!(done.ok);
    assert_eq!(
        done.summary.as_deref(),
        Some("✔ Successfully installed plugin: useful-skills@useful-skills-local (scope: user)")
    );

    let failed_line = r#"{"command":"install","outcome":"failed","message":"Plugin not found"}"#;
    fake.answer(exit(
        1,
        failed_line,
        "✘ Failed to install plugin \"x\": Plugin not found\n",
    ));
    let failed = run(&fake, home.path(), &install(), false).await.unwrap();

    assert!(!failed.ok);
    assert_eq!(failed.exit_code, Some(1));
    assert_eq!(
        failed.summary.as_deref(),
        Some("✘ Failed to install plugin \"x\": Plugin not found")
    );

    let specs = fake.specs();
    assert_eq!(specs.len(), 2);
    assert_eq!(
        args(&specs[0]),
        ["plugin", "install", SKILLS, "--scope", "user", "--json"]
    );
    assert!(
        specs[0]
            .env
            .iter()
            .any(|(name, value)| name == "DISABLE_UPDATES" && value == "1")
    );
    assert!(!args(&specs[0]).iter().any(|arg| arg == "-y" || arg == "--yes"));
}
