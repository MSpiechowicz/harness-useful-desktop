//! Codex detection: its `codex-cli x.y.z` version line, the tested versions, the home it reports
//! (`CODEX_HOME` or `~/.codex`) and the npm install hint.

use harness_business::harness::Harness;
use harness_business::harnesses::Codex;
use harness_business::model::VersionSupport;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use crate::detection::{CannedRunner, NoRunner, binary_file, env_with_bin, fake_binary};

#[test]
fn codex_0_154_to_0_156_are_supported_and_everything_else_untested() {
    assert_eq!(Codex.parse_version("codex-cli 0.154.0\n").as_deref(), Some("0.154.0"));
    assert_eq!(Codex.version_support("0.154.0"), VersionSupport::Supported);
    assert_eq!(Codex.version_support("0.155.2"), VersionSupport::Supported);
    assert_eq!(Codex.version_support("0.156.0"), VersionSupport::Supported);
    assert_eq!(Codex.version_support("0.157.0"), VersionSupport::Untested);
    assert_eq!(Codex.version_support("0.153.1"), VersionSupport::Untested);
    assert_eq!(Codex.version_support("1.0.0"), VersionSupport::Untested);
}

#[tokio::test]
async fn codex_is_detected_with_its_cli_version_and_shows_its_home() {
    let home = TempDir::new().unwrap();
    let (env, bin) = env_with_bin(&home);
    let custom = home.path().join("codex-work");
    let env = env.with_var("CODEX_HOME", custom.to_string_lossy());
    let binary = fake_binary(&bin, &binary_file("codex"), "");
    let runner = CannedRunner::printing("codex-cli 0.154.0\n");

    let detection = Codex.detect(&env, &runner, None).await;

    assert!(detection.installed);
    assert_eq!(detection.version.as_deref(), Some("0.154.0"));
    assert_eq!(detection.version_support, VersionSupport::Supported);
    assert_eq!(detection.error, None);
    assert_eq!(detection.config_dir, Some(custom.to_string_lossy().into_owned()));

    let seen = runner.seen.lock().unwrap();
    assert_eq!(seen[0].program, binary);
    assert_eq!(seen[0].args, ["--version"]);
}

#[tokio::test]
async fn a_newer_codex_is_untested() {
    let home = TempDir::new().unwrap();
    let (env, bin) = env_with_bin(&home);
    fake_binary(&bin, &binary_file("codex"), "");
    let runner = CannedRunner::printing("codex-cli 0.160.1\n");

    let detection = Codex.detect(&env, &runner, None).await;

    assert!(detection.installed);
    assert_eq!(detection.version.as_deref(), Some("0.160.1"));
    assert_eq!(detection.version_support, VersionSupport::Untested);
    assert_eq!(
        detection.config_dir,
        Some(home.path().join(".codex").to_string_lossy().into_owned())
    );
}

#[tokio::test]
async fn a_missing_codex_offers_the_npm_package() {
    let home = TempDir::new().unwrap();
    let (env, _) = env_with_bin(&home);

    let detection = Codex.detect(&env, &NoRunner, None).await;

    assert!(!detection.installed);
    let hint = detection.install_hint.unwrap();
    assert_eq!(hint.commands.len(), 1);
    assert_eq!(hint.commands[0].label, None);
    assert_eq!(hint.commands[0].command, "npm install -g @openai/codex");
    assert_eq!(hint.docs_url, "https://developers.openai.com/codex/cli");
}
