//! Running a harness's own updater: its command line, what counts as an update and what is
//! reported when it does not.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use harness_business::BusinessError;
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harness::update::{UPDATE_TIMEOUT, run_update};
use harness_business::harnesses::{Claude, Codex, Omp, OpenCode, Pi};
use harness_business::model::HarnessUpdateResult;
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use crate::detection::{NoRunner, binary_file, env_with_bin, fake_binary};

/// Answers `--version` with the versions in turn (the last one repeats) and every other command
/// with `update`, recording what it was asked.
struct FakeUpdater {
    versions: Vec<&'static str>,
    update: ExecOutput,
    seen: Mutex<Vec<ExecSpec>>,
}

impl FakeUpdater {
    fn new(versions: &[&'static str], update: ExecOutput) -> Self {
        Self {
            versions: versions.to_vec(),
            update,
            seen: Mutex::new(Vec::new()),
        }
    }

    fn updates(&self) -> Vec<ExecSpec> {
        let seen = self.seen.lock().unwrap();

        seen.iter().filter(|spec| spec.args != ["--version"]).cloned().collect()
    }

    fn probes(&self) -> Vec<ExecSpec> {
        let seen = self.seen.lock().unwrap();

        seen.iter().filter(|spec| spec.args == ["--version"]).cloned().collect()
    }
}

impl ProcessRunner for FakeUpdater {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, harness_business::Result<ExecOutput>> {
        let mut seen = self.seen.lock().unwrap();
        let probes = seen.iter().filter(|seen| seen.args == ["--version"]).count();
        seen.push(spec.clone());

        let output = if spec.args == ["--version"] {
            let version = self.versions[probes.min(self.versions.len() - 1)];

            exited(0, &format!("{version}\n"), "")
        } else {
            self.update.clone()
        };

        Box::pin(async move { Ok(output) })
    }
}

fn exited(code: i32, stdout: &str, stderr: &str) -> ExecOutput {
    ExecOutput {
        status: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
        ..ExecOutput::default()
    }
}

/// A home whose PATH holds a fake binary named like `harness`.
fn installed(harness: &dyn Harness) -> (TempDir, Env, PathBuf) {
    let home = TempDir::new().unwrap();
    let (env, bin) = env_with_bin(&home);
    let binary = fake_binary(&bin, &binary_file(harness.binary_name()), "");

    (home, env, binary)
}

async fn update(harness: &dyn Harness, env: &Env, runner: &FakeUpdater) -> HarnessUpdateResult {
    run_update(harness, env, runner, None).await.unwrap()
}

#[tokio::test]
async fn a_changed_version_after_a_clean_exit_is_an_update() {
    let (_home, env, binary) = installed(&Claude);
    let runner = FakeUpdater::new(&["2.1.300", "2.1.319"], exited(0, "Updating...\nUpdated.\n", ""));

    let result = update(&Claude, &env, &runner).await;

    assert!(result.updated);
    assert_eq!(result.message, None);
    assert_eq!(result.command, "claude update");
    assert_eq!(result.summary.detection.version.as_deref(), Some("2.1.319"));

    let ran = runner.updates();
    assert_eq!(ran.len(), 1);
    assert_eq!(ran[0].program, binary);
    assert_eq!(ran[0].args, ["update"]);
    assert_eq!(ran[0].stdin, None);
    assert_eq!(ran[0].cwd.as_deref(), Some(env.home.as_path()));
    assert_eq!(ran[0].timeout, UPDATE_TIMEOUT);
    assert_eq!(UPDATE_TIMEOUT, Duration::from_secs(600));
}

#[tokio::test]
async fn pi_updates_itself_with_its_self_flag() {
    let (_home, env, _binary) = installed(&Pi);
    let runner = FakeUpdater::new(&["1.1.0", "1.5.0"], exited(0, "", ""));

    let result = update(&Pi, &env, &runner).await;

    assert!(result.updated);
    assert_eq!(result.command, "pi update --self");
    assert_eq!(runner.updates()[0].args, ["update", "--self"]);
}

#[tokio::test]
async fn the_updater_runs_without_the_apps_self_update_switches() {
    let (_home, env, _binary) = installed(&Claude);
    let runner = FakeUpdater::new(&["2.1.300", "2.1.319"], exited(0, "", ""));
    update(&Claude, &env, &runner).await;

    let (_home, opencode_env, _binary) = installed(&OpenCode::default());
    let opencode_runner = FakeUpdater::new(&["1.19.3", "1.19.5"], exited(0, "", ""));
    update(&OpenCode::default(), &opencode_env, &opencode_runner).await;

    for (runner, switch) in [
        (&runner, "DISABLE_UPDATES"),
        (&opencode_runner, "OPENCODE_DISABLE_AUTOUPDATE"),
    ] {
        let spec = &runner.updates()[0];

        assert!(
            spec.env.iter().all(|(name, _)| name != switch),
            "{switch}: {:?}",
            spec.env
        );
        // Nothing is removed from the inherited environment either: a variable the user exported stays.
        assert!(spec.env_remove_prefixes.is_empty(), "{switch}");

        // The probes of the same detection still carry it.
        for probe in runner.probes() {
            assert!(probe.env.iter().any(|(name, _)| name == switch), "{switch}");
        }
    }
}

#[tokio::test]
async fn the_updater_keeps_what_else_a_spawn_of_the_harness_needs() {
    let (_home, env, _binary) = installed(&Codex);
    let env = env.with_var("CODEX_HOME", "relative/home");
    let runner = FakeUpdater::new(&["0.130.0", "0.167.0"], exited(0, "", ""));

    update(&Codex, &env, &runner).await;

    assert_eq!(runner.updates()[0].env_remove_prefixes, ["CODEX_HOME"]);
}

#[tokio::test]
async fn a_clean_exit_without_a_new_version_is_not_an_update() {
    let (_home, env, _binary) = installed(&Claude);
    let runner = FakeUpdater::new(
        &["2.1.300"],
        exited(
            0,
            "Claude was installed with brew\nRun brew upgrade claude-code to update\n",
            "",
        ),
    );

    let result = update(&Claude, &env, &runner).await;

    assert!(!result.updated);
    assert_eq!(
        result.message.as_deref(),
        Some("Run brew upgrade claude-code to update")
    );
    assert_eq!(result.summary.detection.version.as_deref(), Some("2.1.300"));
}

#[tokio::test]
async fn a_failed_exit_reports_the_last_line_of_stderr_without_secrets() {
    let (_home, env, _binary) = installed(&Claude);
    let token = format!("ghp_{}", "aB3dE5gH7j".repeat(4));
    let stderr = format!("\x1b[31mnpm warn\x1b[0m something\n\nerror: cannot write, token {token} rejected\n\n");
    let runner = FakeUpdater::new(&["2.1.300", "2.1.300"], exited(1, "Updating...\n", &stderr));

    let result = update(&Claude, &env, &runner).await;

    assert!(!result.updated);
    let message = result.message.expect("a message");
    assert!(message.starts_with("error: cannot write"), "{message}");
    assert!(!message.contains(&token), "{message}");
    assert!(!message.contains('\x1b'), "{message}");
}

#[tokio::test]
async fn a_failed_exit_falls_back_to_stdout_and_then_to_the_exit_code() {
    let (_home, env, _binary) = installed(&Claude);

    let with_stdout = FakeUpdater::new(&["2.1.300"], exited(2, "no network\n", ""));
    let result = update(&Claude, &env, &with_stdout).await;
    assert_eq!(result.message.as_deref(), Some("no network"));

    let silent = FakeUpdater::new(&["2.1.300"], exited(7, "", "  \n"));
    let result = update(&Claude, &env, &silent).await;
    assert!(!result.updated);
    assert!(result.message.expect("a message").contains('7'));
}

#[tokio::test]
async fn control_and_format_characters_never_reach_the_message() {
    let (_home, env, _binary) = installed(&Claude);
    let hostile = "\u{1b}(B\u{1b}7denied\u{202e}txt.exe\u{7}\u{9b}31m by\u{200b} policy\n";
    let runner = FakeUpdater::new(&["2.1.300"], exited(1, "", hostile));

    let message = update(&Claude, &env, &runner).await.message.expect("a message");

    assert!(message.starts_with("denied"), "{message:?}");
    assert!(message.ends_with("by policy"), "{message:?}");
    assert!(
        !message
            .chars()
            .any(|c| c.is_control() || matches!(c, '\u{200b}' | '\u{202e}')),
        "{message:?}"
    );
}

#[tokio::test]
async fn a_secret_split_by_a_removed_character_stays_redacted() {
    let (_home, env, _binary) = installed(&Claude);
    let tail = "aB3dE5gH7j".repeat(4);

    for separator in ["\u{200b}", "\u{1b}(B", "\u{7}"] {
        let token = format!("{}{tail}", ["gh", "p_"].concat());
        let split = format!("{}{separator}{tail}", ["gh", "p_"].concat());
        let runner = FakeUpdater::new(&["2.1.300"], exited(1, "", &format!("error: token {split} rejected\n")));

        let message = update(&Claude, &env, &runner).await.message.expect("a message");

        assert!(!message.contains(&token), "{message:?}");
    }
}

#[tokio::test]
async fn a_long_last_line_is_cut_short() {
    let (_home, env, _binary) = installed(&Claude);
    let runner = FakeUpdater::new(&["2.1.300"], exited(1, "", &"x".repeat(5000)));

    let message = update(&Claude, &env, &runner).await.message.expect("a message");

    assert!(message.chars().count() <= 200, "{}", message.chars().count());
}

#[tokio::test]
async fn an_update_that_ran_out_of_time_says_so_and_changes_nothing() {
    let (_home, env, _binary) = installed(&Claude);
    let output = ExecOutput {
        timed_out: true,
        ..ExecOutput::default()
    };
    let runner = FakeUpdater::new(&["2.1.300", "2.1.319"], output);

    let result = update(&Claude, &env, &runner).await;

    assert!(!result.updated);
    assert!(result.message.expect("a message").contains("10 minutes"));
}

#[tokio::test]
async fn a_version_that_cannot_be_read_afterwards_is_not_an_update() {
    let (_home, env, _binary) = installed(&Claude);
    let runner = FakeUpdater::new(&["2.1.300", "no version here"], exited(0, "done\n", ""));

    let result = update(&Claude, &env, &runner).await;

    assert!(!result.updated);
    assert!(result.message.is_some());
}

#[tokio::test]
async fn the_updater_runs_the_override_binary() {
    let home = TempDir::new().unwrap();
    let (env, _bin) = env_with_bin(&home);
    let elsewhere = fake_binary(&home.path().join("opt"), &binary_file("claude"), "");
    let runner = FakeUpdater::new(&["2.1.300", "2.1.319"], exited(0, "", ""));

    let result = run_update(&Claude, &env, &runner, Some(&elsewhere)).await.unwrap();

    assert!(result.updated);
    assert_eq!(runner.updates()[0].program, elsewhere);
}

#[tokio::test]
async fn a_harness_that_is_not_installed_is_not_updated() {
    let home = TempDir::new().unwrap();
    let (env, _bin) = env_with_bin(&home);

    let error = run_update(&Claude, &env, &NoRunner, None).await.unwrap_err();

    assert!(matches!(error, BusinessError::NotFound(_)), "{error:?}");
}

/// Each harness at its oldest version with the updater and the version just below it: `(harness,
/// minimum, below)`. OpenCode 1 and 2 have their own.
fn minimums() -> Vec<(Box<dyn Harness>, &'static str, &'static str)> {
    vec![
        (Box::new(Claude), "2.0.67", "2.0.66"),
        (Box::new(Codex), "0.126.0", "0.125.9"),
        (Box::new(Omp), "18.12.5", "18.12.4"),
        (Box::new(Pi), "0.70.6", "0.70.5"),
        (Box::new(OpenCode::default()), "1.0.0", "0.9.9"),
        (Box::new(OpenCode::default()), "2.2.6", "2.2.5"),
    ]
}

#[tokio::test]
async fn a_version_below_the_minimum_is_not_updated_and_gets_the_install_command() {
    for (harness, _minimum, below) in minimums() {
        let (_home, env, _binary) = installed(harness.as_ref());
        let runner = FakeUpdater::new(&[below], exited(0, "", ""));

        let result = update(harness.as_ref(), &env, &runner).await;

        assert!(!result.updated, "{below}");
        assert_eq!(
            result.message.as_deref(),
            Some("This version is too old to update from the app. The command below installs the latest one.")
        );
        assert_eq!(result.summary.detection.version.as_deref(), Some(below));
        assert!(runner.updates().is_empty(), "{below}: only the version probe may run");
        assert_eq!(runner.probes().len(), 1, "{below}");
    }
}

#[tokio::test]
async fn the_install_command_of_an_old_cli_is_the_first_of_its_install_hint() {
    let (_home, env, _binary) = installed(&Pi);
    let runner = FakeUpdater::new(&["0.70.5"], exited(0, "", ""));

    let result = update(&Pi, &env, &runner).await;

    assert_eq!(result.command, Pi.install_hint().commands[0].command);
    assert_ne!(result.command, "pi update --self");
}

#[tokio::test]
async fn an_old_opencode_gets_the_install_command_of_its_generation() {
    let hint = OpenCode::default().install_hint();
    let command_of = |label: &str| {
        hint.commands
            .iter()
            .find(|entry| entry.label.as_deref() == Some(label))
            .expect("a labelled command")
            .command
            .clone()
    };

    for (version, label) in [("0.9.9", "OpenCode 1"), ("2.2.5", "OpenCode 2")] {
        let (_home, env, _binary) = installed(&OpenCode::default());
        let runner = FakeUpdater::new(&[version], exited(0, "", ""));

        let result = update(&OpenCode::default(), &env, &runner).await;

        assert_eq!(result.command, command_of(label), "{version}");
    }
}

#[tokio::test]
async fn a_version_exactly_at_the_minimum_runs_the_updater() {
    for (harness, minimum, _below) in minimums() {
        let (_home, env, _binary) = installed(harness.as_ref());
        let runner = FakeUpdater::new(&[minimum, "99.0.0"], exited(0, "", ""));

        let result = update(harness.as_ref(), &env, &runner).await;

        assert!(result.updated, "{minimum}");
        assert_eq!(runner.updates().len(), 1, "{minimum}");
    }
}

#[tokio::test]
async fn a_version_that_cannot_be_read_is_not_updated() {
    let (_home, env, _binary) = installed(&Claude);
    let runner = FakeUpdater::new(&["no version here"], exited(0, "", ""));

    let result = update(&Claude, &env, &runner).await;

    assert!(!result.updated);
    assert_eq!(result.command, Claude.install_hint().commands[0].command);
    assert!(runner.updates().is_empty());
}

#[tokio::test]
async fn a_suffixed_version_counts_as_its_numeric_base() {
    let (_home, env, _binary) = installed(&Codex);
    let old = FakeUpdater::new(&["0.125.0-beta.1"], exited(0, "", ""));
    update(&Codex, &env, &old).await;
    assert!(old.updates().is_empty(), "below the minimum despite the suffix");

    let current = FakeUpdater::new(&["0.126.0-beta.1", "0.127.0"], exited(0, "", ""));
    update(&Codex, &env, &current).await;
    assert_eq!(current.updates().len(), 1, "the base reaches the minimum");
}
