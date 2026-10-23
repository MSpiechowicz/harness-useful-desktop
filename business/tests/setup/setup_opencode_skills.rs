//! The Harness Skills for OpenCode in the "Recommended setup" step: installed from their install
//! record, offered as their installer script when `node` is there, unavailable without it, and an
//! installer run that is ok only once the record holds a version.

use crate::setup_support;

use std::path::{Path, PathBuf};
use std::time::Duration;

use harness_business::ecosystem::catalog::{SKILLS_INSTALL_PS1, SKILLS_INSTALL_SH};
use harness_business::ecosystem::{Companion, installer, run_installer_with, setup_status};
use harness_business::env::{Env, OsKind};
use harness_business::harness::{Harness, HarnessRegistry};
use harness_business::harnesses::OpenCode;
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

/// The Harness Skills' OpenCode install record below `home`.
fn skills_record(home: &Path) -> PathBuf {
    home.join(".config/opencode/plugins/useful-skills/install-record.json")
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
fn env_with_node(home: &Path) -> Env {
    Env::for_tests(home).with_var("PATH", place_node(&home.join("node-bin")))
}

/// The OpenCode skills offer of `env`, with OpenCode detected.
async fn opencode_skills(env: &Env) -> SkillsSetup {
    let summary = HarnessSummary {
        info: OpenCode::default().info(),
        capabilities: OpenCode::default().capabilities(),
        detection: Detection {
            installed: true,
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

    let opencode = setup
        .harnesses
        .iter()
        .find(|entry| entry.harness == HarnessId::OpenCode);
    opencode.expect("OpenCode is registered").skills.clone()
}

#[tokio::test]
async fn without_a_record_the_installer_is_offered_with_what_it_changes() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path()).with_os(OsKind::Linux);

    let SkillsSetup::Script {
        command, side_effects, ..
    } = opencode_skills(&env).await
    else {
        panic!("the installer is offered");
    };

    assert_eq!(command, SKILLS_INSTALL_SH);
    assert!(side_effects.iter().any(|line| line.contains("~/.config/opencode")));
    assert!(
        side_effects
            .iter()
            .any(|line| line.contains("useful_skills MCP server"))
    );
    assert!(side_effects.iter().any(|line| line.contains("Node.js 22 or newer")));
    assert!(side_effects.iter().any(|line| line.contains("/useful-skills update")));
    assert!(side_effects.iter().all(|line| !line.contains(';')), "{side_effects:?}");
}

#[tokio::test]
async fn without_node_the_skills_are_unavailable() {
    let home = tempfile::tempdir().unwrap();
    let empty = home.path().join("empty-bin");
    std::fs::create_dir_all(&empty).unwrap();
    // Windows: the usual install directories all lie below the temporary home.
    let env = Env::for_tests(home.path())
        .with_os(OsKind::Windows)
        .with_var("PATH", empty.to_string_lossy())
        .with_var("APPDATA", home.path().join("AppData").to_string_lossy())
        .with_var("USERPROFILE", home.path().to_string_lossy());

    assert_eq!(
        opencode_skills(&env).await,
        SkillsSetup::Unavailable {
            reason: "Needs Node.js 22 or newer.".to_owned(),
            code: Some(SkillsUnavailableCode::NodeMissing),
        }
    );

    let with_node = env.with_var("PATH", place_node(&home.path().join("node-bin")));
    let SkillsSetup::Script {
        command, side_effects, ..
    } = opencode_skills(&with_node).await
    else {
        panic!("the installer is offered once node is there");
    };
    assert_eq!(command, SKILLS_INSTALL_PS1);
    assert!(
        side_effects
            .iter()
            .any(|line| line.contains(r"%USERPROFILE%\.config\opencode"))
    );
}

#[tokio::test]
async fn a_record_with_a_version_counts_as_installed() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path());
    write(
        &skills_record(home.path()),
        &json!({ "version": "0.4.0", "installedAt": "2026-10-20T10:00:00Z" }).to_string(),
    );

    assert_eq!(
        opencode_skills(&env).await,
        SkillsSetup::Installed {
            version: Some("0.4.0".to_owned())
        }
    );
}

#[tokio::test]
async fn the_record_follows_xdg_config_home() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join("xdg");
    let env = Env::for_tests(home.path()).with_var("XDG_CONFIG_HOME", config.to_string_lossy());
    write(
        &config.join("opencode/plugins/useful-skills/install-record.json"),
        r#"{"version":"1.0.0"}"#,
    );

    assert_eq!(
        opencode_skills(&env).await,
        SkillsSetup::Installed {
            version: Some("1.0.0".to_owned())
        }
    );
}

#[tokio::test]
async fn an_unusable_record_does_not_count() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path());
    let record = skills_record(home.path());

    for text in [
        "not json",
        r#"["0.4.0"]"#,
        r#"{"name":"useful-skills"}"#,
        r#"{"version":""}"#,
        r#"{"version":7}"#,
        r#"{"version":"0.4.0\u001b[2J"}"#,
    ] {
        write(&record, text);

        assert!(
            matches!(opencode_skills(&env).await, SkillsSetup::Script { .. }),
            "{text:?} does not count as installed"
        );
    }

    std::fs::remove_file(&record).unwrap();
    std::fs::create_dir_all(&record).unwrap();
    assert!(
        matches!(opencode_skills(&env).await, SkillsSetup::Script { .. }),
        "a folder does not count as installed"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_symlinked_record_does_not_count() {
    let home = tempfile::tempdir().unwrap();
    let env = env_with_node(home.path());
    let target = home.path().join("elsewhere.json");
    write(&target, r#"{"version":"0.4.0"}"#);
    let record = skills_record(home.path());
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &record).unwrap();

    assert!(matches!(opencode_skills(&env).await, SkillsSetup::Script { .. }));
}

#[test]
fn the_installer_command_is_fixed_per_os() {
    let home = tempfile::tempdir().unwrap();
    let download = "curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.sh";
    assert_unix_installer(home.path(), Companion::OpenCodeSkills, SKILLS_INSTALL_SH, download);

    let windows = installer(
        &Env::for_tests(home.path()).with_os(OsKind::Windows),
        Companion::OpenCodeSkills,
    );
    let script = "irm https://raw.githubusercontent.com/MSpiechowicz/harness-useful-skills/main/install.ps1 | iex";
    assert_windows_installer(&windows, script, SKILLS_INSTALL_PS1);
}

/// Runs the skills installer against a peer that writes `stderr`, then exits with `exit`; returns
/// the result, the spec it was started with and how many version probes ran.
async fn run_scripted(home: &Path, stderr: &'static str, exit: ExitInfo) -> (DashboardInstallResult, ExecSpec, usize) {
    let env = Env::for_tests(home).with_os(OsKind::Linux);
    let runner = CountingRunner::default();
    let (spawner, mut peers) = DuplexSpawner::new();

    let player = tokio::spawn(async move { play(peers.recv().await.unwrap(), "", stderr, exit).await });
    let result = run_installer_with(
        &env,
        Companion::OpenCodeSkills,
        &spawner,
        &runner,
        Duration::from_secs(30),
        |_| {},
    )
    .await
    .unwrap();

    let spec = player.await.unwrap();
    let probes = runner.probes();
    (result, spec, probes)
}

#[tokio::test]
async fn an_install_without_a_record_afterwards_is_not_ok() {
    let home = tempfile::tempdir().unwrap();

    let (result, spec, probes) = run_scripted(home.path(), "", EXITED_0).await;

    assert_eq!(spec.args[1], SKILLS_INSTALL_SH);
    assert!(
        spec.search_dirs.contains(&home.path().join(".local/bin")),
        "node is looked for in the usual install directories too"
    );
    assert!(
        spec.env
            .iter()
            .any(|(name, value)| name == "OPENCODE_DISABLE_AUTOUPDATE" && value == "1"),
        "an opencode the installer runs never updates itself: {:?}",
        spec.env
    );
    assert!(!result.ok);
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(result.version, None);
    assert_eq!(result.command, SKILLS_INSTALL_SH);
    let summary = result.summary.unwrap();
    assert!(summary.contains("install record was not found afterwards"), "{summary}");
    assert!(!summary.contains(';'), "{summary}");
    assert_eq!(probes, 0, "no binary is probed for the skills");
}

#[tokio::test]
async fn a_record_without_a_version_is_not_ok() {
    let home = tempfile::tempdir().unwrap();
    write(&skills_record(home.path()), r#"{"name":"useful-skills"}"#);

    let (result, _, _) = run_scripted(home.path(), "", EXITED_0).await;

    assert!(!result.ok);
    assert_eq!(
        result.summary.as_deref(),
        Some("The installer finished, but the Harness Skills install record did not report a version.")
    );
}

#[tokio::test]
async fn the_install_is_ok_once_the_record_holds_a_version() {
    let home = tempfile::tempdir().unwrap();
    write(&skills_record(home.path()), r#"{"version":"0.4.0"}"#);

    let (result, _, _) = run_scripted(home.path(), "", EXITED_0).await;

    assert!(result.ok);
    assert_eq!(result.version.as_deref(), Some("0.4.0"));
    assert_eq!(
        result.summary.as_deref(),
        Some("Installed Harness Skills for OpenCode 0.4.0.")
    );
}

#[tokio::test]
async fn a_failing_installer_is_not_ok_with_an_older_record() {
    let home = tempfile::tempdir().unwrap();
    write(&skills_record(home.path()), r#"{"version":"0.3.0"}"#);
    let failed = ExitInfo {
        code: Some(1),
        signal: None,
    };

    let (result, _, _) = run_scripted(home.path(), "error: Node.js 22 or newer is required\n", failed).await;

    assert!(!result.ok);
    assert_eq!(
        result.summary.as_deref(),
        Some("error: Node.js 22 or newer is required")
    );
}
