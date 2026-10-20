//! OpenCode extension actions through its CLI: `opencode plugin <module> -g` for npm registry
//! modules only, with the self-updater switched off, nothing run for a preview, a hostile module
//! or an unsupported action (a fake `opencode` script, never the real one).

use std::path::Path;

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::OpenCode;
use harness_business::model::{ExtensionAction, InstallScope};
use pretty_assertions::assert_eq;

fn install(target: &str) -> ExtensionAction {
    ExtensionAction::PluginInstall {
        target: target.to_owned(),
        scope: InstallScope::User,
    }
}

/// Hostile or non-registry modules: flags, paths, whitespace, control characters, URLs, other
/// schemes, credentials and names over npm's 214 characters.
fn hostile_modules() -> Vec<String> {
    let mut modules: Vec<String> = [
        "",
        "-g",
        "--help",
        "../x",
        "./x",
        ".x",
        "/abs/plugin",
        "a\\b",
        "a b",
        "a\nb",
        "a\u{7}b",
        "file:///x",
        "file:x",
        "github:a/b",
        "git+https://github.com/a/b",
        "https://x",
        "user:pass@x",
        "a/b",
        "@scope/a/b",
        "x.tgz",
        "x@../y",
        "x@-g",
    ]
    .iter()
    .map(|module| (*module).to_owned())
    .collect();

    modules.push("x".repeat(215));
    modules
}

/// Actions OpenCode's CLI cannot run.
fn unsupported_actions() -> Vec<ExtensionAction> {
    use ExtensionAction::*;

    let name = || "demo".to_owned();

    vec![
        PluginInstall {
            target: name(),
            scope: InstallScope::Project,
        },
        PluginUpgrade { name: Some(name()) },
        PluginUpgrade { name: None },
        PluginEnable { name: name() },
        PluginDisable { name: name() },
        MarketplaceAdd { source: name() },
        MarketplaceRemove { name: name() },
        MarketplaceUpdate { name: None },
        PackageInstall {
            source: "npm:demo".to_owned(),
        },
    ]
}

#[test]
fn opencode_installs_npm_modules_globally_without_a_preview_flag() {
    let project = Path::new("/home/demo/Projekty/demo");

    for module in [
        "opencode-wakatime",
        "@scope/opencode-plugin",
        "@scope/opencode-plugin@1.2.3",
        "opencode-plugin@latest",
        "Legacy.Name_x@=1.0.0",
        &"x".repeat(214),
    ] {
        for project in [None, Some(project)] {
            let built = OpenCode.cli_action(&install(module), project).expect(module);

            assert_eq!(built.args, ["plugin", module, "-g"]);
            assert_eq!(built.dry_run_flag, None, "{module}");
        }
    }
}

#[test]
fn hostile_modules_are_refused_without_echoing_them() {
    for module in hostile_modules() {
        let error = OpenCode.cli_action(&install(&module), None).expect_err(&module);

        assert!(matches!(error, BusinessError::InvalidInput(_)), "{module:?}: {error:?}");
        if module.len() > 2 && module.len() < 100 {
            assert!(!error.to_string().contains(&module), "{module:?} is echoed: {error}");
        }
    }
}

#[test]
fn opencode_actions_other_than_a_user_install_are_unsupported_by_its_cli() {
    let mut actions = unsupported_actions();
    actions.push(ExtensionAction::PluginUninstall {
        name: "demo".to_owned(),
    });

    for action in actions {
        let error = OpenCode
            .cli_action(&action, Some(Path::new("/home/demo/p")))
            .expect_err("unsupported");

        assert!(matches!(error, BusinessError::Unsupported(_)), "{action:?}: {error:?}");
    }
}

#[test]
fn only_plugin_removal_is_a_config_edit() {
    let home = tempfile::tempdir().expect("home");
    let env = Env::for_tests(home.path());

    let mut actions = unsupported_actions();
    actions.push(install("opencode-wakatime"));

    for action in actions {
        for dry_run in [true, false] {
            let routed = OpenCode
                .config_action(&env, &action, None, dry_run)
                .expect("not refused");

            assert_eq!(routed, None, "{action:?} goes to the CLI");
        }
    }
}

#[test]
fn opencode_offers_cli_actions() {
    assert!(OpenCode.capabilities().cli_actions);
}

#[cfg(unix)]
mod runs {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use harness_business::env::Env;
    use harness_business::harness::{CliRun, run_cli_action};
    use harness_business::harnesses::OpenCode;
    use harness_business::model::{CliResult, ExtensionAction};
    use harness_business::process::{ResolvedBinary, TokioRunner};
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;

    use super::{hostile_modules, install, unsupported_actions};

    /// A fake `opencode` that records its arguments and `OPENCODE_DISABLE_AUTOUPDATE`.
    struct FakeOpenCode {
        dir: TempDir,
        binary: ResolvedBinary,
    }

    impl FakeOpenCode {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let bin = dir.path().join("bin");
            std::fs::create_dir_all(&bin).expect("create bin");

            let shebang = "#!/bin/sh"; // guard-allow: installer-shell
            let script = format!(
                "{shebang}\nd=$(dirname \"$0\")\nprintf '%s\\n' \"$@\" > \"$d/argv\"\n\
                 printf '%s' \"$OPENCODE_DISABLE_AUTOUPDATE\" > \"$d/autoupdate\"\necho 'Installed plugin'\n"
            );
            let path = bin.join("opencode");
            std::fs::write(&path, script).expect("write script");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");

            Self {
                dir,
                binary: ResolvedBinary {
                    path,
                    is_batch_script: false,
                },
            }
        }

        fn recorded(&self, file: &str) -> Option<String> {
            std::fs::read_to_string(self.dir.path().join("bin").join(file)).ok()
        }

        fn home(&self) -> PathBuf {
            let home = self.dir.path().join("home");
            std::fs::create_dir_all(&home).expect("create home");
            home
        }

        async fn run(&self, action: ExtensionAction, dry_run: bool) -> harness_business::Result<CliResult> {
            let env = Env::for_tests(self.home());

            run_cli_action(CliRun {
                harness: &OpenCode,
                env: &env,
                runner: &TokioRunner,
                binary: &self.binary,
                action: &action,
                dry_run,
                project: None,
            })
            .await
        }
    }

    #[tokio::test]
    async fn an_install_runs_opencode_plugin_with_the_self_updater_off() {
        let cli = FakeOpenCode::new();

        let result = cli.run(install("@scope/demo@1.0.0"), false).await.expect("runs");

        assert!(result.ok, "{result:?}");
        assert_eq!(result.argv, ["opencode", "plugin", "@scope/demo@1.0.0", "-g"]);
        assert_eq!(result.command, result.argv);
        assert_eq!(result.config_edit, None);
        assert_eq!(cli.recorded("argv").as_deref(), Some("plugin\n@scope/demo@1.0.0\n-g\n"));
        assert_eq!(cli.recorded("autoupdate").as_deref(), Some("1"));
    }

    #[tokio::test]
    async fn a_preview_of_an_install_runs_nothing() {
        let cli = FakeOpenCode::new();

        let result = cli.run(install("demo"), true).await.expect("a preview");

        assert!(result.ok);
        assert_eq!(result.exit_code, None);
        assert_eq!(result.command, ["opencode", "plugin", "demo", "-g"]);
        assert_eq!(cli.recorded("argv"), None, "the CLI never started");
    }

    #[tokio::test]
    async fn hostile_modules_and_unsupported_actions_run_nothing() {
        let cli = FakeOpenCode::new();

        for module in hostile_modules() {
            for dry_run in [true, false] {
                assert!(cli.run(install(&module), dry_run).await.is_err(), "{module:?}");
            }
        }

        for action in unsupported_actions() {
            assert!(cli.run(action.clone(), false).await.is_err(), "{action:?}");
        }

        assert_eq!(cli.recorded("argv"), None, "the CLI never started");
    }
}
