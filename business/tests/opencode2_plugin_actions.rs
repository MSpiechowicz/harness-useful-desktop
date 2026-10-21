//! OpenCode 2 extension actions through its CLI: `opencode plugin add <module>` (no `-g`, no
//! background service) for npm registry modules only, picked from the detected version, with the
//! self-updater switched off and nothing run for a preview or a hostile module (a fake `opencode`
//! script, never the real one).

use std::path::Path;

use harness_business::BusinessError;
use harness_business::harness::{Harness, OpenCodeGeneration};
use harness_business::harnesses::OpenCode;
use harness_business::model::{ExtensionAction, InstallScope};
use pretty_assertions::assert_eq;

#[cfg(unix)]
mod fake_cli_support;

fn install(target: &str) -> ExtensionAction {
    ExtensionAction::PluginInstall {
        target: target.to_owned(),
        scope: InstallScope::User,
    }
}

fn opencode2() -> OpenCode {
    OpenCode::with_generation(OpenCodeGeneration::V2)
}

#[test]
fn opencode_2_adds_npm_modules_without_a_global_flag_or_a_preview_flag() {
    let project = Path::new("/home/demo/Projekty/demo");

    for module in [
        "opencode-wakatime",
        "@scope/opencode-plugin@1.2.3",
        "opencode-plugin@latest",
    ] {
        for project in [None, Some(project)] {
            let built = opencode2().cli_action(&install(module), project).expect(module);

            assert_eq!(built.args, ["plugin", "add", module]);
            assert_eq!(built.dry_run_flag, None, "{module}");
        }
    }
}

#[test]
fn the_detected_version_picks_the_command() {
    let opencode = OpenCode::default();

    opencode.note_version(Some("2.1.2"));
    let added = opencode.cli_action(&install("demo"), None).expect("v2");
    assert_eq!(added.args, ["plugin", "add", "demo"]);

    opencode.note_version(Some("1.19.3"));
    let global = opencode.cli_action(&install("demo"), None).expect("v1");
    assert_eq!(global.args, ["plugin", "demo", "-g"]);
}

#[test]
fn hostile_modules_project_installs_and_other_actions_are_refused() {
    for module in [
        "",
        "-g",
        "--help",
        "./x",
        "/abs/plugin",
        "a b",
        "github:a/b",
        "https://x",
        "x@-g",
    ] {
        let error = opencode2().cli_action(&install(module), None).expect_err(module);

        assert!(matches!(error, BusinessError::InvalidInput(_)), "{module:?}: {error:?}");
    }

    let unsupported = [
        ExtensionAction::PluginInstall {
            target: "demo".to_owned(),
            scope: InstallScope::Project,
        },
        ExtensionAction::PluginUninstall {
            name: "demo".to_owned(),
        },
        ExtensionAction::PluginUpgrade { name: None },
        ExtensionAction::MarketplaceAdd {
            source: "demo".to_owned(),
        },
    ];

    for action in unsupported {
        let error = opencode2().cli_action(&action, None).expect_err("unsupported");

        assert!(matches!(error, BusinessError::Unsupported(_)), "{action:?}: {error:?}");
    }
}

#[cfg(unix)]
mod runs {
    use harness_business::env::Env;
    use harness_business::harness::{CliRun, run_cli_action};
    use harness_business::model::{CliResult, ExtensionAction};
    use harness_business::process::TokioRunner;
    use pretty_assertions::assert_eq;

    use super::fake_cli_support::FakeCli;
    use super::{install, opencode2};

    /// A fake `opencode` that records its arguments and `OPENCODE_DISABLE_AUTOUPDATE`.
    fn fake() -> FakeCli {
        let body = "printf '%s\\n' \"$@\" > \"$d/argv\"\n\
                    printf '%s' \"$OPENCODE_DISABLE_AUTOUPDATE\" > \"$d/autoupdate\"\n\
                    echo 'Plugin \"demo\" installed and added to opencode.json'\n";
        FakeCli::script("opencode", body)
    }

    async fn run(cli: &FakeCli, action: ExtensionAction, dry_run: bool) -> harness_business::Result<CliResult> {
        let env = Env::for_tests(cli.home());

        run_cli_action(CliRun {
            harness: &opencode2(),
            env: &env,
            runner: &TokioRunner,
            binary: &cli.binary,
            action: &action,
            dry_run,
            project: None,
        })
        .await
    }

    #[tokio::test]
    async fn an_install_runs_opencode_plugin_add_with_the_self_updater_off() {
        let cli = fake();

        let result = run(&cli, install("@scope/demo@1.0.0"), false).await.expect("runs");

        assert!(result.ok, "{result:?}");
        assert_eq!(result.argv, ["opencode", "plugin", "add", "@scope/demo@1.0.0"]);
        assert_eq!(result.config_edit, None);
        assert_eq!(
            cli.recorded("argv").as_deref(),
            Some("plugin\nadd\n@scope/demo@1.0.0\n")
        );
        assert_eq!(cli.recorded("autoupdate").as_deref(), Some("1"));
    }

    #[tokio::test]
    async fn a_preview_or_a_hostile_module_runs_nothing() {
        let cli = fake();

        let preview = run(&cli, install("demo"), true).await.expect("a preview");
        assert!(preview.ok);
        assert_eq!(preview.command, ["opencode", "plugin", "add", "demo"]);

        for module in ["--help", "../x", "a\nb"] {
            assert!(run(&cli, install(module), false).await.is_err(), "{module:?}");
        }

        assert_eq!(cli.recorded("argv"), None, "the CLI never started");
    }
}
