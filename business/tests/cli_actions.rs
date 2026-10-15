//! Extension actions: the argv each harness builds, target validation, and runs of fake harness
//! binaries (shell scripts in temporary directories; the real `omp`/`pi` never run here).

use std::path::Path;

use harness_business::BusinessError;
use harness_business::harness::{CliInvocation, Harness};
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::{ExtensionAction, InstallScope};
use pretty_assertions::assert_eq;

fn invocation(harness: &dyn Harness, action: ExtensionAction, project: Option<&Path>) -> CliInvocation {
    harness
        .cli_action(&action, project)
        .unwrap_or_else(|error| panic!("{action:?}: {error}"))
}

fn some(text: &str) -> Option<String> {
    Some(text.to_owned())
}

/// An action, its project, the expected arguments and the expected preview flag.
type OmpCase<'a> = (ExtensionAction, Option<&'a Path>, Vec<&'a str>, Option<&'a str>);

#[test]
fn omp_actions_map_onto_omp_plugin_with_dry_run_only_where_omp_honours_it() {
    use ExtensionAction::*;

    let project = Path::new("/home/demo/Projekty/demo");
    let sidebar = "harness-useful-sidebar@harness-useful-sidebar";
    let cases: Vec<OmpCase<'_>> = vec![
        (
            PluginInstall {
                target: sidebar.to_owned(),
                scope: InstallScope::User,
            },
            None,
            vec!["plugin", "install", sidebar, "--scope", "user"],
            Some("--dry-run"),
        ),
        (
            PluginInstall {
                target: sidebar.to_owned(),
                scope: InstallScope::Project,
            },
            Some(project),
            vec!["plugin", "install", sidebar, "--scope", "project"],
            Some("--dry-run"),
        ),
        (
            PluginUninstall {
                name: "demo-plugin".to_owned(),
            },
            None,
            vec!["plugin", "uninstall", "demo-plugin"],
            Some("--dry-run"),
        ),
        (
            PluginEnable {
                name: sidebar.to_owned(),
            },
            None,
            vec!["plugin", "enable", sidebar],
            None,
        ),
        (
            PluginDisable {
                name: sidebar.to_owned(),
            },
            Some(project),
            vec!["plugin", "disable", sidebar],
            None,
        ),
        (PluginUpgrade { name: None }, None, vec!["plugin", "upgrade"], None),
        (
            PluginUpgrade { name: some(sidebar) },
            None,
            vec!["plugin", "upgrade", sidebar],
            None,
        ),
        (
            MarketplaceAdd {
                source: "MSpiechowicz/harness-useful-sidebar".to_owned(),
            },
            None,
            vec!["plugin", "marketplace", "add", "MSpiechowicz/harness-useful-sidebar"],
            None,
        ),
        (
            MarketplaceRemove {
                name: "harness-useful-sidebar".to_owned(),
            },
            None,
            vec!["plugin", "marketplace", "remove", "harness-useful-sidebar"],
            None,
        ),
        (
            MarketplaceUpdate { name: None },
            None,
            vec!["plugin", "marketplace", "update"],
            None,
        ),
        (
            MarketplaceUpdate { name: some("demo") },
            None,
            vec!["plugin", "marketplace", "update", "demo"],
            None,
        ),
    ];

    for (action, project, args, dry_run) in cases {
        let built = invocation(&Omp, action.clone(), project);

        assert_eq!(built.args, args, "{action:?}");
        assert_eq!(built.dry_run_flag, dry_run, "{action:?}");
    }
}

#[test]
fn pi_actions_map_onto_pi_packages_and_never_update_pi_itself() {
    use ExtensionAction::*;

    let project = Path::new("/home/demo/Projekty/demo");
    let source = "npm:@example/pi-tools@1.0.0";
    let cases: Vec<(ExtensionAction, Option<&Path>, Vec<&str>)> = vec![
        (
            PackageInstall {
                source: source.to_owned(),
            },
            None,
            vec!["install", source],
        ),
        (
            PackageInstall {
                source: "git:github.com/example/pi-tools@v1".to_owned(),
            },
            Some(project),
            vec!["install", "git:github.com/example/pi-tools@v1", "--local"],
        ),
        (
            PackageRemove {
                source: source.to_owned(),
            },
            Some(project),
            vec!["remove", source, "--local"],
        ),
        (PackageUpdate { source: some(source) }, None, vec!["update", source]),
        (PackageUpdate { source: None }, None, vec!["update", "--extensions"]),
        (
            PackageUpdate { source: None },
            Some(project),
            vec!["update", "--extensions"],
        ),
    ];

    for (action, project, args) in cases {
        let built = invocation(&Pi, action.clone(), project);

        assert_eq!(built.args, args, "{action:?}");
        assert_eq!(built.dry_run_flag, None, "{action:?}");
    }
}

#[test]
fn actions_of_the_other_harness_are_unsupported() {
    let package = ExtensionAction::PackageInstall {
        source: "npm:x".to_owned(),
    };
    let plugin = ExtensionAction::PluginEnable { name: "x".to_owned() };

    for (harness, action) in [(&Omp as &dyn Harness, &package), (&Pi, &plugin)] {
        let error = harness.cli_action(action, None).expect_err("unsupported");
        assert!(matches!(error, BusinessError::Unsupported(_)), "{error:?}");
    }
}

#[test]
fn a_project_scope_install_needs_a_project() {
    let action = ExtensionAction::PluginInstall {
        target: "demo@demo".to_owned(),
        scope: InstallScope::Project,
    };

    let error = Omp.cli_action(&action, None).expect_err("no project");
    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
}

#[test]
fn targets_that_could_be_flags_shell_text_or_carry_credentials_are_refused() {
    let refused = [
        "",
        "-rf",
        "--scope=project",
        "two words",
        "name;rm",
        "$(whoami)",
        "name`id`",
        "a\nb",
        "https://user:hunter2@github.com/o/r",
        "https://ghp_abcdefghijklmnopqrstuvwxyz0123@github.com/o/r",
        "git:token@github.com/o/r",
        "git:user:pw@github.com/o/r",
        &"x".repeat(513),
    ];

    for target in refused {
        let install = ExtensionAction::PackageInstall {
            source: target.to_owned(),
        };
        let error = Pi.cli_action(&install, None).expect_err(target);

        assert!(matches!(error, BusinessError::InvalidInput(_)), "{target}: {error:?}");
        if !target.is_empty() && target.len() < 100 {
            assert!(!error.to_string().contains(target), "the value is not echoed: {error}");
        }

        let add = ExtensionAction::MarketplaceAdd {
            source: target.to_owned(),
        };
        assert!(Omp.cli_action(&add, None).is_err(), "{target}");
    }
}

#[test]
fn package_specs_urls_and_local_paths_are_accepted() {
    let accepted = [
        "npm:@example/pi-tools@1.0.0",
        "npm:pi-mcp-adapter",
        "git:github.com/example/pi-tools@v1",
        "git:git@github.com:example/pi-tools",
        "https://github.com/example/pi-tools",
        "./local-package",
        "/home/demo/pi-local",
    ];

    for source in accepted {
        let action = ExtensionAction::PackageInstall {
            source: source.to_owned(),
        };

        assert_eq!(invocation(&Pi, action, None).args, ["install", source]);
    }
}

#[cfg(unix)]
mod runs {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    use harness_business::env::Env;
    use harness_business::harness::{CliRun, Harness, run_cli_action};
    use harness_business::harnesses::{Omp, Pi};
    use harness_business::model::{CliResult, ExtensionAction, InstallScope, MAX_CLI_OUTPUT_BYTES};
    use harness_business::process::{ResolvedBinary, TokioRunner};
    use tempfile::TempDir;

    /// A fake CLI in `<dir>/bin/<name>` that records its arguments, working directory and
    /// `NO_COLOR`, prints `<dir>/bin/stdout` and `<dir>/bin/stderr` and exits with `code`.
    struct FakeCli {
        dir: TempDir,
        binary: ResolvedBinary,
    }

    impl FakeCli {
        fn new(name: &str, stdout: &str, stderr: &str, code: i32) -> Self {
            let dir = tempfile::tempdir().expect("temp dir");
            let bin = dir.path().join("bin");
            std::fs::create_dir_all(&bin).expect("create bin");
            std::fs::write(bin.join("stdout"), stdout).expect("write stdout");
            std::fs::write(bin.join("stderr"), stderr).expect("write stderr");

            let shebang = "#!/bin/sh"; // guard-allow: installer-shell
            let script = format!(
                "{shebang}\nd=$(dirname \"$0\")\nprintf '%s\\n' \"$@\" > \"$d/argv\"\npwd -P > \"$d/cwd\"\n\
                 printf '%s' \"$NO_COLOR\" > \"$d/no_color\"\ncat \"$d/stdout\"\ncat \"$d/stderr\" >&2\nexit {code}\n"
            );
            let path = bin.join(name);
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
    }

    async fn run(
        harness: &dyn Harness,
        cli: &FakeCli,
        action: ExtensionAction,
        dry_run: bool,
        project: Option<&Path>,
    ) -> CliResult {
        let env = Env::for_tests(cli.home());

        run_cli_action(CliRun {
            harness,
            env: &env,
            runner: &TokioRunner,
            binary: &cli.binary,
            action: &action,
            dry_run,
            project,
        })
        .await
        .expect("the action runs")
    }

    fn sidebar_install(scope: InstallScope) -> ExtensionAction {
        ExtensionAction::PluginInstall {
            target: "harness-useful-sidebar@harness-useful-sidebar".to_owned(),
            scope,
        }
    }

    #[tokio::test]
    async fn an_omp_failure_with_exit_code_zero_is_not_ok() {
        let line = "✘ Failed to disable demo: Error: Plugin demo not found in runtime config";
        let cli = FakeCli::new("omp", "", &format!("{line}\n"), 0);

        let result = run(
            &Omp,
            &cli,
            ExtensionAction::PluginDisable {
                name: "demo".to_owned(),
            },
            false,
            None,
        )
        .await;

        assert!(!result.ok);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.summary.as_deref(), Some(line));
        assert_eq!(result.argv, ["omp", "plugin", "disable", "demo"]);
        assert_eq!(cli.recorded("argv").as_deref(), Some("plugin\ndisable\ndemo\n"));
        assert_eq!(cli.recorded("no_color").as_deref(), Some("1"));
    }

    #[tokio::test]
    async fn a_dry_run_install_runs_in_the_project_with_the_preview_flag() {
        let cli = FakeCli::new(
            "omp",
            "[dry-run] Would install harness-useful-sidebar@harness-useful-sidebar\n",
            "",
            0,
        );
        let project = tempfile::tempdir().expect("project");

        let result = run(
            &Omp,
            &cli,
            sidebar_install(InstallScope::Project),
            true,
            Some(project.path()),
        )
        .await;

        assert!(result.ok, "{result:?}");
        assert_eq!(
            result.summary.as_deref(),
            Some("[dry-run] Would install harness-useful-sidebar@harness-useful-sidebar")
        );
        assert_eq!(result.argv.last().map(String::as_str), Some("--dry-run"));
        assert_eq!(
            cli.recorded("cwd").map(|cwd| PathBuf::from(cwd.trim())),
            Some(dunce::canonicalize(project.path()).unwrap())
        );
    }

    #[tokio::test]
    async fn without_a_project_the_cli_runs_in_the_home_directory() {
        let cli = FakeCli::new("omp", "✔ Installed\n", "", 0);

        let result = run(&Omp, &cli, sidebar_install(InstallScope::User), false, None).await;

        assert!(result.ok);
        assert_eq!(
            cli.recorded("cwd").map(|cwd| PathBuf::from(cwd.trim())),
            Some(dunce::canonicalize(cli.home()).unwrap())
        );
    }

    #[tokio::test]
    async fn a_dry_run_without_a_preview_flag_runs_nothing() {
        let cli = FakeCli::new("pi", "", "", 0);

        let action = ExtensionAction::PackageInstall {
            source: "npm:pi-mcp-adapter".to_owned(),
        };
        let result = run(&Pi, &cli, action, true, None).await;

        assert!(result.ok);
        assert_eq!(result.exit_code, None);
        assert_eq!(result.argv, ["pi", "install", "npm:pi-mcp-adapter"]);
        assert!(result.summary.unwrap().contains("nothing was run"));
        assert_eq!(cli.recorded("argv"), None, "the CLI never started");
    }

    #[tokio::test]
    async fn the_command_to_confirm_never_carries_the_preview_flag() {
        let uninstall = || ExtensionAction::PluginUninstall {
            name: "demo-plugin".to_owned(),
        };
        let applying = ["omp", "plugin", "uninstall", "demo-plugin"];

        let omp = FakeCli::new("omp", "[dry-run] Would uninstall demo-plugin\n", "", 0);
        let preview = run(&Omp, &omp, uninstall(), true, None).await;

        assert_eq!(preview.exit_code, Some(0));
        assert_eq!(preview.argv, ["omp", "plugin", "uninstall", "demo-plugin", "--dry-run"]);
        assert_eq!(preview.command, applying);

        let applied = run(&Omp, &omp, uninstall(), false, None).await;

        assert_eq!(applied.argv, applying);
        assert_eq!(applied.command, applied.argv);
        assert_eq!(
            applied.command, preview.command,
            "the preview shows what the run applies"
        );

        let pi = FakeCli::new("pi", "", "", 0);
        let action = ExtensionAction::PackageRemove {
            source: "npm:pi-mcp-adapter".to_owned(),
        };
        let pi_preview = run(&Pi, &pi, action, true, None).await;

        assert_eq!(pi_preview.exit_code, None);
        assert_eq!(pi_preview.command, ["pi", "remove", "npm:pi-mcp-adapter"]);
        assert_eq!(pi_preview.command, pi_preview.argv);
    }

    #[tokio::test]
    async fn output_is_stripped_of_escapes_and_redacted() {
        let stdout = "\u{1b}[32m✔ Installed demo\u{1b}[0m\nusing token=ghp_abcdefghijklmnopqrstuvwxyz0123456789\n";
        let cli = FakeCli::new("omp", stdout, "", 0);

        let result = run(&Omp, &cli, sidebar_install(InstallScope::User), false, None).await;

        assert!(result.stdout.starts_with("✔ Installed demo\n"), "{:?}", result.stdout);
        assert!(!result.stdout.contains('\u{1b}'));
        assert!(!result.stdout.contains("ghp_abcdefghij"), "{:?}", result.stdout);
    }

    #[tokio::test]
    async fn a_failing_exit_code_is_not_ok_and_large_output_is_capped() {
        let flood = "x".repeat(MAX_CLI_OUTPUT_BYTES + 10_000);
        let cli = FakeCli::new("pi", &flood, "npm error 404 Not Found\n", 1);

        let action = ExtensionAction::PackageRemove {
            source: "npm:missing".to_owned(),
        };
        let result = run(&Pi, &cli, action, false, None).await;

        assert!(!result.ok);
        assert_eq!(result.exit_code, Some(1));
        assert_eq!(result.summary.as_deref(), Some("npm error 404 Not Found"));
        assert!(result.truncated);
        assert!(result.stdout.len() <= MAX_CLI_OUTPUT_BYTES);
    }
}
