//! The Harness Skills for Codex in the "Recommended setup" step: installed from their install
//! record in the Codex home (version 0.3.13 or newer), offered as their installer script with
//! `HARNESS_SKILLS_HOST=codex` when `node` is there, unavailable without it, and an installer run
//! that is ok only once the record holds a version.

use crate::setup_support;

use std::path::{Path, PathBuf};
use std::time::Duration;

use harness_business::ecosystem::catalog::{CODEX_SKILLS_INSTALL_PS1, CODEX_SKILLS_INSTALL_SH};
use harness_business::ecosystem::{Companion, installer, run_installer_with, setup_status};
use harness_business::env::{Env, OsKind};
use harness_business::harness::{Harness, HarnessRegistry};
use harness_business::harnesses::Codex;
use harness_business::model::{
    DashboardInstallResult, Detection, HarnessId, HarnessSummary, SkillsSetup, SkillsUnavailableCode, VersionSupport,
};
use harness_business::process::{ExecSpec, ExitInfo};
use harness_business::testing::DuplexSpawner;
use pretty_assertions::assert_eq;
use serde_json::json;
use setup_support::{CountingRunner, EXITED_0, assert_unix_installer, assert_windows_installer, play};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The Harness Skills' Codex install record below `home`.
pub(crate) fn skills_record(home: &Path) -> PathBuf {
    home.join(".codex/harness-useful-skills/install-record.json")
}

/// The record Harness Skills before 0.4.0 wrote.
pub(crate) fn legacy_record(home: &Path) -> PathBuf {
    home.join(".codex/useful-skills/install-record.json")
}

/// Places an executable `node` (`node.exe` on Windows) in `dir` and returns `dir`, for `PATH`.
fn place_node(dir: &Path) -> String {
    let node = dir.join(if cfg!(windows) { "node.exe" } else { "node" });
    write(&node, "");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&node, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    dir.to_string_lossy().into_owned()
}

/// A home with `node` on `PATH`.
pub(crate) fn env_with_node(home: &Path) -> Env {
    Env::for_tests(home).with_var("PATH", place_node(&home.join("node-bin")))
}

/// A Windows machine without `node` anywhere: the usual install directories all lie below the
/// temporary home.
fn windows_without_node(home: &Path) -> Env {
    let empty = home.join("empty-bin");
    std::fs::create_dir_all(&empty).unwrap();

    Env::for_tests(home)
        .with_os(OsKind::Windows)
        .with_var("PATH", empty.to_string_lossy())
        .with_var("APPDATA", home.join("AppData").to_string_lossy())
        .with_var("USERPROFILE", home.to_string_lossy())
}

/// The whole setup of `env`, with Codex detected or not.
async fn codex_skills_with(env: &Env, installed: bool) -> SkillsSetup {
    let summary = HarnessSummary {
        info: Codex.info(),
        capabilities: Codex.capabilities(),
        detection: Detection {
            installed,
            binary_path: None,
            is_batch_script: false,
            version: None,
            version_support: VersionSupport::Untested,
            config_dir: None,
            config_dir_exists: false,
            install_hint: None,
            error: None,
        },
        native_approval_mode: None,
    };
    let setup = setup_status(
        env,
        &CountingRunner::default(),
        &HarnessRegistry::with_defaults(),
        &[summary],
    )
    .await;

    let codex = setup.harnesses.iter().find(|entry| entry.harness == HarnessId::Codex);
    assert_eq!(codex.expect("Codex is registered").installed, installed);

    codex.unwrap().skills.clone()
}

/// The Codex skills offer of `env`, with Codex detected.
pub(crate) async fn codex_skills(env: &Env) -> SkillsSetup {
    codex_skills_with(env, true).await
}

/// The installer offer for an install or update, with the version it updates from.
pub(crate) fn script_of(setup: SkillsSetup) -> (String, Vec<String>, Option<String>) {
    let SkillsSetup::Script {
        command,
        side_effects,
        outdated_from,
    } = setup
    else {
        panic!("the installer is offered, not {setup:?}");
    };

    (command, side_effects, outdated_from)
}

#[tokio::test]
async fn without_a_record_the_installer_is_offered_with_what_it_changes() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path()).with_os(OsKind::Linux);

    let (command, side_effects, outdated_from) = script_of(codex_skills(&env).await);

    assert_eq!(command, CODEX_SKILLS_INSTALL_SH);
    assert_eq!(outdated_from, None);
    assert!(side_effects.iter().any(|line| line.contains("(~/.codex)")));
    assert!(
        side_effects
            .iter()
            .any(|line| line.contains("harness-useful-skills MCP server with codex mcp add"))
    );
    assert!(side_effects.iter().any(|line| line.contains("Node.js 22 or newer")));
    assert!(side_effects.iter().any(|line| line.contains("this installer again")));
    assert!(side_effects.iter().all(|line| !line.contains(';')), "{side_effects:?}");
}

#[tokio::test]
async fn the_offer_names_the_codex_home_that_is_set() {
    let home = tempfile::tempdir().unwrap();
    let codex_home = home.path().join("elsewhere");
    let env = env_with_node(home.path()).with_var("CODEX_HOME", codex_home.to_string_lossy());

    let (_, side_effects, _) = script_of(codex_skills(&env).await);

    let folder = codex_home.to_string_lossy().into_owned();
    assert!(
        side_effects.iter().any(|line| line.contains(&folder)),
        "{side_effects:?}"
    );

    // A relative CODEX_HOME is ignored, as for the Codex harness itself.
    let relative = env_with_node(home.path()).with_var("CODEX_HOME", "relative/home");
    let (_, side_effects, _) = script_of(codex_skills(&relative).await);
    assert!(
        side_effects.iter().any(|line| line.contains("(~/.codex)")),
        "{side_effects:?}"
    );
}

#[tokio::test]
async fn without_node_the_skills_are_unavailable() {
    let home = tempfile::tempdir().unwrap();
    let env = windows_without_node(home.path());

    assert_eq!(
        codex_skills(&env).await,
        SkillsSetup::Unavailable {
            reason: "Needs Node.js 22 or newer.".to_owned(),
            code: Some(SkillsUnavailableCode::NodeMissing),
        }
    );

    let with_node = env.with_var("PATH", place_node(&home.path().join("node-bin")));
    let (command, side_effects, _) = script_of(codex_skills(&with_node).await);
    assert_eq!(command, CODEX_SKILLS_INSTALL_PS1);
    assert!(side_effects.iter().any(|line| line.contains(r"(%USERPROFILE%\.codex)")));
}

#[tokio::test]
async fn an_outdated_install_without_node_is_unavailable_too() {
    let home = tempfile::tempdir().unwrap();
    write(&skills_record(home.path()), r#"{"version":"0.3.12"}"#);

    assert_eq!(
        codex_skills(&windows_without_node(home.path())).await,
        SkillsSetup::Unavailable {
            reason: "Needs Node.js 22 or newer.".to_owned(),
            code: Some(SkillsUnavailableCode::NodeMissing),
        }
    );
}

#[tokio::test]
async fn a_record_with_the_first_codex_release_or_newer_counts_as_installed() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path());

    for version in ["0.3.13", "0.3.100", "0.4.0", "1.0.0"] {
        write(
            &skills_record(home.path()),
            &json!({ "version": version, "installedAt": "2026-10-23T10:00:00Z" }).to_string(),
        );

        assert_eq!(
            codex_skills(&env).await,
            SkillsSetup::Installed {
                version: Some(version.to_owned())
            },
            "{version}"
        );
    }
}

#[tokio::test]
async fn an_older_record_is_updated_by_the_installer() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path()).with_os(OsKind::Linux);

    // Compared by number, not by text: 0.3.9 is older than 0.3.13.
    for version in ["0.3.12", "0.3.9", "0.2.41"] {
        write(&skills_record(home.path()), &json!({ "version": version }).to_string());

        let (command, _, outdated_from) = script_of(codex_skills(&env).await);

        assert_eq!(command, CODEX_SKILLS_INSTALL_SH);
        assert_eq!(outdated_from.as_deref(), Some(version));
    }
}

#[tokio::test]
async fn a_machine_without_codex_still_reports_the_skills() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path());

    assert!(matches!(
        codex_skills_with(&env, false).await,
        SkillsSetup::Script { .. }
    ));
}

#[tokio::test]
async fn the_record_follows_codex_home() {
    let home = tempfile::tempdir().unwrap();
    let codex_home = home.path().join("custom-codex");
    let env = env_with_node(home.path()).with_var("CODEX_HOME", codex_home.to_string_lossy());
    write(
        &codex_home.join("harness-useful-skills/install-record.json"),
        r#"{"version":"0.3.14"}"#,
    );
    // The default folder's record is not the one that counts.
    write(&skills_record(home.path()), r#"{"version":"0.3.12"}"#);

    assert_eq!(
        codex_skills(&env).await,
        SkillsSetup::Installed {
            version: Some("0.3.14".to_owned())
        }
    );

    let relative = env_with_node(home.path()).with_var("CODEX_HOME", "custom-codex");
    let (_, _, outdated_from) = script_of(codex_skills(&relative).await);
    assert_eq!(
        outdated_from.as_deref(),
        Some("0.3.12"),
        "a relative CODEX_HOME is ignored"
    );
}

#[tokio::test]
async fn an_unusable_record_does_not_count() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path());
    let record = skills_record(home.path());

    for text in [
        "not json",
        r#"["0.3.13"]"#,
        r#"{"name":"useful-skills"}"#,
        r#"{"version":""}"#,
        r#"{"version":13}"#,
        r#"{"version":"0.3.13\u001b[2J"}"#,
    ] {
        write(&record, text);

        let (_, _, outdated_from) = script_of(codex_skills(&env).await);
        assert_eq!(outdated_from, None, "{text:?} does not count as installed");
    }

    std::fs::remove_file(&record).unwrap();
    std::fs::create_dir_all(&record).unwrap();
    assert!(
        matches!(codex_skills(&env).await, SkillsSetup::Script { .. }),
        "a folder does not count as installed"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlinked_record_does_not_count() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path());
    let target = home.path().join("elsewhere.json");
    write(&target, r#"{"version":"0.3.13"}"#);
    let record = skills_record(home.path());
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &record).unwrap();

    assert!(matches!(codex_skills(&env).await, SkillsSetup::Script { .. }));
}

#[test]
fn the_installer_command_picks_the_codex_host_per_os() {
    let home = tempfile::tempdir().unwrap();
    let download = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.sh";
    assert_unix_installer(home.path(), Companion::CodexSkills, CODEX_SKILLS_INSTALL_SH, download);
    assert!(CODEX_SKILLS_INSTALL_SH.ends_with("| HARNESS_SKILLS_HOST=codex sh"));

    let windows = installer(
        &Env::for_tests(home.path()).with_os(OsKind::Windows),
        Companion::CodexSkills,
    );
    let script = "$env:HARNESS_SKILLS_HOST = 'codex'; irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.ps1 | iex";
    assert_windows_installer(&windows, script, CODEX_SKILLS_INSTALL_PS1);
}

/// Runs the installer of `companion` against a peer that writes `stderr`, then exits with `exit`;
/// returns the result, the spec it was started with and how many version probes ran.
async fn run_scripted(
    env: &Env,
    companion: Companion,
    stderr: &'static str,
    exit: ExitInfo,
) -> (DashboardInstallResult, ExecSpec, usize) {
    let runner = CountingRunner::default();
    let (spawner, mut peers) = DuplexSpawner::new();

    let player = tokio::spawn(async move { play(peers.recv().await.unwrap(), "", stderr, exit).await });
    let result = run_installer_with(env, companion, &spawner, &runner, Duration::from_secs(30), |_| {})
        .await
        .unwrap();

    let spec = player.await.unwrap();
    let probes = runner.probes();
    (result, spec, probes)
}

fn codex_env(home: &Path) -> Env {
    Env::for_tests(home).with_os(OsKind::Linux)
}

#[tokio::test]
async fn the_codex_installer_runs_the_script_for_the_codex_host() {
    let home = tempfile::tempdir().unwrap();

    let (result, spec, probes) = run_scripted(&codex_env(home.path()), Companion::CodexSkills, "", EXITED_0).await;

    assert_eq!(spec.args[1], CODEX_SKILLS_INSTALL_SH);
    assert!(
        spec.env
            .iter()
            .any(|(name, value)| name == "HARNESS_SKILLS_HOST" && value == "codex"),
        "{:?}",
        spec.env
    );
    assert!(
        spec.search_dirs.contains(&home.path().join(".npm-global/bin")),
        "node and codex are looked for in the usual install directories too"
    );
    assert!(
        spec.env.iter().all(|(name, _)| name != "OPENCODE_DISABLE_AUTOUPDATE"),
        "{:?}",
        spec.env
    );
    assert!(spec.env_remove_prefixes.is_empty(), "{:?}", spec.env_remove_prefixes);

    assert!(!result.ok);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.version, None);
    assert_eq!(result.command, CODEX_SKILLS_INSTALL_SH);
    let summary = result.summary.unwrap();
    assert!(summary.contains("install record was not found afterwards"), "{summary}");
    assert!(!summary.contains(';'), "{summary}");
    assert_eq!(probes, 0, "no binary is probed for the skills");
}

#[tokio::test]
async fn a_relative_codex_home_never_reaches_the_installer() {
    let home = tempfile::tempdir().unwrap();
    let env = codex_env(home.path()).with_var("CODEX_HOME", "relative/home");

    let (_, spec, _) = run_scripted(&env, Companion::CodexSkills, "", EXITED_0).await;
    assert_eq!(spec.env_remove_prefixes, ["CODEX_HOME"]);

    let absolute = codex_env(home.path()).with_var("CODEX_HOME", home.path().join("codex").to_string_lossy());
    let (_, spec, _) = run_scripted(&absolute, Companion::CodexSkills, "", EXITED_0).await;
    assert!(
        spec.env_remove_prefixes.is_empty(),
        "an absolute CODEX_HOME is the one Codex uses too"
    );
}

#[tokio::test]
async fn the_opencode_installer_never_inherits_a_host() {
    let home = tempfile::tempdir().unwrap();

    let (_, spec, _) = run_scripted(&codex_env(home.path()), Companion::OpenCodeSkills, "", EXITED_0).await;

    assert_eq!(spec.env_remove_prefixes, ["HARNESS_SKILLS_HOST"]);
    assert!(
        spec.env.iter().all(|(name, _)| name != "HARNESS_SKILLS_HOST"),
        "{:?}",
        spec.env
    );
}

#[tokio::test]
async fn a_record_without_a_version_is_not_ok() {
    let home = tempfile::tempdir().unwrap();
    write(&skills_record(home.path()), r#"{"name":"useful-skills"}"#);

    let (result, _, _) = run_scripted(&codex_env(home.path()), Companion::CodexSkills, "", EXITED_0).await;

    assert!(!result.ok);
    assert_eq!(
        result.summary.as_deref(),
        Some("The installer finished, but the Harness Skills install record did not report a version.")
    );
}

#[tokio::test]
async fn the_install_is_ok_once_the_record_holds_a_version() {
    let home = tempfile::tempdir().unwrap();
    write(&skills_record(home.path()), r#"{"version":"0.3.13"}"#);

    let (result, _, _) = run_scripted(&codex_env(home.path()), Companion::CodexSkills, "", EXITED_0).await;

    assert!(result.ok);
    assert_eq!(result.version.as_deref(), Some("0.3.13"));
    assert_eq!(
        result.summary.as_deref(),
        Some("Installed Harness Skills for Codex 0.3.13.")
    );
}

#[tokio::test]
async fn a_failing_installer_is_not_ok_with_an_older_record() {
    let home = tempfile::tempdir().unwrap();
    write(&skills_record(home.path()), r#"{"version":"0.3.12"}"#);
    let failed = ExitInfo {
        code: Some(1),
        signal: None,
    };

    let (result, _, _) = run_scripted(
        &codex_env(home.path()),
        Companion::CodexSkills,
        "error: installing for Codex needs the 'codex' command\n",
        failed,
    )
    .await;

    assert!(!result.ok);
    assert_eq!(
        result.summary.as_deref(),
        Some("error: installing for Codex needs the 'codex' command")
    );
}
