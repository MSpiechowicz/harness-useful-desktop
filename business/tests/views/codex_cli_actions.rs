//! Codex extension actions through `codex plugin …`: the argv of each action (user scope only, run
//! in the home directory), target validation, the actions Codex has no command for, a dry run that
//! runs nothing, and a run whose exit code decides (a recording stand-in for `codex`, never the
//! real binary).

use crate::codex_support;

use std::path::Path;

use codex_support::{FakeCodex, binary, exit};
use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::{CliRun, Harness, run_cli_action};
use harness_business::harnesses::Codex;
use harness_business::model::{ExtensionAction, InstallScope};
use pretty_assertions::assert_eq;

const PLUGIN: &str = "lint-tools@demo-market";

fn argv(action: &ExtensionAction) -> Vec<String> {
    let invocation = Codex
        .cli_action(action, Some(Path::new("/home/demo/Projects/demo")))
        .unwrap_or_else(|error| panic!("{action:?}: {error}"));

    assert_eq!(invocation.dry_run_flag, None, "Codex has no preview flag");
    assert!(invocation.in_home_dir, "{action:?} acts on the user's settings");
    invocation.args
}

#[test]
fn plugin_and_marketplace_actions_map_to_codex_plugin() {
    use ExtensionAction::*;

    let cases: Vec<(ExtensionAction, Vec<&str>)> = vec![
        (
            PluginInstall {
                target: PLUGIN.to_owned(),
                scope: InstallScope::User,
            },
            vec!["plugin", "add", PLUGIN],
        ),
        (
            PluginUninstall {
                name: PLUGIN.to_owned(),
            },
            vec!["plugin", "remove", PLUGIN],
        ),
        (
            MarketplaceAdd {
                source: "acme/demo-market".to_owned(),
            },
            vec!["plugin", "marketplace", "add", "acme/demo-market"],
        ),
        (
            MarketplaceAdd {
                source: "https://github.com/acme/demo-market.git".to_owned(),
            },
            vec![
                "plugin",
                "marketplace",
                "add",
                "https://github.com/acme/demo-market.git",
            ],
        ),
        (
            MarketplaceRemove {
                name: "demo-market".to_owned(),
            },
            vec!["plugin", "marketplace", "remove", "demo-market"],
        ),
        (
            MarketplaceUpdate {
                name: Some("demo-market".to_owned()),
            },
            vec!["plugin", "marketplace", "upgrade", "demo-market"],
        ),
        (
            MarketplaceUpdate { name: None },
            vec!["plugin", "marketplace", "upgrade"],
        ),
    ];

    for (action, expected) in cases {
        assert_eq!(argv(&action), expected, "{action:?}");
    }
}

#[test]
fn a_plugin_needs_its_marketplace_and_hostile_values_are_refused() {
    use ExtensionAction::*;

    let mut actions = Vec::new();
    for value in ["lint-tools", "@demo-market", "", "-y", "a b@m", "x;rm@m", "$(x)@m"] {
        actions.push(PluginInstall {
            target: value.to_owned(),
            scope: InstallScope::User,
        });
        actions.push(PluginUninstall { name: value.to_owned() });
    }
    for source in [
        "",
        "--help",
        "./local",
        "git@github.com:a/b.git",
        "//host/share",
        "https://user:pass@example.com/m.git",
    ] {
        actions.push(MarketplaceAdd {
            source: source.to_owned(),
        });
    }
    actions.push(MarketplaceRemove { name: "-x".to_owned() });
    actions.push(MarketplaceUpdate {
        name: Some("a b".to_owned()),
    });
    actions.push(PluginInstall {
        target: PLUGIN.to_owned(),
        scope: InstallScope::Project,
    });

    for action in actions {
        let error = Codex.cli_action(&action, None).expect_err(&format!("{action:?}"));
        assert!(matches!(error, BusinessError::InvalidInput(_)), "{action:?}: {error:?}");

        for value in ["x;rm", "$(x)", "user:pass", "git@github.com:a/b.git"] {
            if format!("{action:?}").contains(value) {
                assert!(!error.to_string().contains(value), "{value:?} is echoed: {error}");
            }
        }
    }
}

#[test]
fn switching_updating_and_packages_are_unsupported() {
    use ExtensionAction::*;

    for action in [
        PluginEnable {
            name: PLUGIN.to_owned(),
        },
        PluginDisable {
            name: PLUGIN.to_owned(),
        },
        PluginUpgrade {
            name: Some(PLUGIN.to_owned()),
        },
        PluginUpgrade { name: None },
        PackageInstall {
            source: "npm:demo".to_owned(),
        },
        PackageRemove {
            source: "npm:demo".to_owned(),
        },
        PackageUpdate { source: None },
    ] {
        let error = Codex.cli_action(&action, None).expect_err(&format!("{action:?}"));
        assert!(matches!(error, BusinessError::Unsupported(_)), "{action:?}: {error:?}");
    }
}

async fn run(
    fake: &FakeCodex,
    home: &Path,
    action: &ExtensionAction,
    dry_run: bool,
) -> harness_business::Result<harness_business::model::CliResult> {
    let env = Env::for_tests(home);
    let codex = binary(home);

    run_cli_action(CliRun {
        harness: &Codex,
        env: &env,
        runner: fake,
        binary: &codex,
        action,
        dry_run,
        project: None,
    })
    .await
}

fn install() -> ExtensionAction {
    ExtensionAction::PluginInstall {
        target: PLUGIN.to_owned(),
        scope: InstallScope::User,
    }
}

#[tokio::test]
async fn a_dry_run_reports_the_command_and_runs_nothing() {
    let home = tempfile::tempdir().unwrap();
    let fake = FakeCodex::new(&home.path().join(".codex/config.toml"));

    let preview = run(&fake, home.path(), &install(), true).await.unwrap();

    assert_eq!(
        (preview.ok, preview.command.as_slice(), fake.argvs().len()),
        (
            true,
            ["codex", "plugin", "add", PLUGIN].map(str::to_owned).as_slice(),
            0
        ),
        "the command line, and nothing run"
    );
}

#[tokio::test]
async fn a_run_reports_its_last_line_in_the_home_directory_and_the_exit_code_decides() {
    let home = tempfile::tempdir().unwrap();
    let fake = FakeCodex::new(&home.path().join(".codex/config.toml"));

    fake.answer(exit(
        0,
        "Installing lint-tools…\nInstalled plugin lint-tools@demo-market.\n",
        "",
    ));
    let done = run(&fake, home.path(), &install(), false).await.unwrap();

    assert!(done.ok);
    assert_eq!(
        done.summary.as_deref(),
        Some("Installed plugin lint-tools@demo-market.")
    );
    assert_eq!(fake.specs()[0].cwd.as_deref(), Some(home.path()));

    fake.answer(exit(
        1,
        "",
        "Error: plugin `lint-tools` was not found in marketplace `demo-market`",
    ));
    let failed = run(&fake, home.path(), &install(), false).await.unwrap();

    assert!(!failed.ok);
}
