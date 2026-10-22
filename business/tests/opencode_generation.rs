//! OpenCode 1 or 2: detection reads the version of the `opencode` binary (a fake script, never the
//! real one) and the OpenCode harness follows its generation (chat protocol, install hint).

use harness_business::harness::{ChatProtocol, Harness, OpenCodeGeneration};
use harness_business::harnesses::OpenCode;
use harness_business::model::VersionSupport;
use pretty_assertions::assert_eq;

#[cfg(unix)]
mod fake_cli_support;

#[test]
fn versions_with_major_two_or_later_are_opencode_2() {
    assert_eq!(OpenCodeGeneration::from_version("2.1.2"), OpenCodeGeneration::V2);
    assert_eq!(OpenCodeGeneration::from_version("v2.0.0"), OpenCodeGeneration::V2);
}

#[test]
fn opencode_1_and_unreadable_versions_are_opencode_1() {
    assert_eq!(OpenCodeGeneration::from_version("1.19.3"), OpenCodeGeneration::V1);
    assert_eq!(OpenCodeGeneration::from_version(""), OpenCodeGeneration::V1);
    assert_eq!(OpenCodeGeneration::from_version("garbage"), OpenCodeGeneration::V1);
}

#[test]
fn opencode_2_1_is_supported() {
    let harness = OpenCode::default();

    assert_eq!(harness.version_support("2.1.2"), VersionSupport::Supported);
    assert_eq!(harness.version_support("2.0.0"), VersionSupport::Untested);
    assert_eq!(harness.version_support("1.19.3"), VersionSupport::Supported);
}

#[test]
fn opencode_2_installs_from_npm_next_to_opencode_1_whatever_was_detected() {
    let v1 = OpenCode::with_generation(OpenCodeGeneration::V1).install_hint();
    let v2 = OpenCode::with_generation(OpenCodeGeneration::V2).install_hint();

    assert_eq!(v1, v2);
    let opencode_2 = v2
        .commands
        .iter()
        .find(|command| command.label.as_deref() == Some("OpenCode 2"))
        .expect("an OpenCode 2 route");
    assert_eq!(opencode_2.command, "npm install -g @opencode/cli");
    assert_eq!(v2.docs_url, "https://opencode.ai/docs/");
}

#[test]
fn an_undetected_opencode_is_opencode_1() {
    let harness = OpenCode::default();

    assert_eq!(harness.generation(), OpenCodeGeneration::V1);
    assert!(matches!(harness.chat_protocol(), ChatProtocol::OpenCodeServer));
}

#[cfg(unix)]
mod detected {
    use harness_business::env::Env;
    use harness_business::harness::HarnessRegistry;
    use harness_business::model::HarnessId;
    use harness_business::process::TokioRunner;
    use pretty_assertions::assert_eq;

    use super::fake_cli_support::FakeCli;
    use super::{ChatProtocol, Harness, OpenCode, OpenCodeGeneration, VersionSupport};

    /// An `opencode` script that prints `version` for `--version`, and the environment of its home.
    fn opencode_printing(version: &str) -> (FakeCli, Env) {
        let cli = FakeCli::script("opencode", &format!("echo '{version}'"));
        let env = Env::for_tests(cli.home());

        (cli, env)
    }

    #[tokio::test]
    async fn opencode_v2_is_detected_as_opencode_2_with_its_chat_protocol() {
        let (cli, env) = opencode_printing("opencode v2.1.2");
        let registry = HarnessRegistry::with_defaults();
        let harness = registry.get(HarnessId::OpenCode).expect("OpenCode is registered");

        let detection = harness.detect(&env, &TokioRunner, Some(&cli.binary.path)).await;

        assert!(detection.installed);
        assert_eq!(detection.version.as_deref(), Some("2.1.2"));
        assert_eq!(detection.version_support, VersionSupport::Supported);
        assert_eq!(detection.error, None);
        assert!(matches!(harness.chat_protocol(), ChatProtocol::OpenCodeServerV2));
    }

    #[tokio::test]
    async fn opencode_1_19_is_detected_as_opencode_1() {
        let (cli, env) = opencode_printing("1.19.3");
        let harness = OpenCode::with_generation(OpenCodeGeneration::V2);

        let detection = harness.detect(&env, &TokioRunner, Some(&cli.binary.path)).await;

        assert_eq!(detection.version.as_deref(), Some("1.19.3"));
        assert_eq!(detection.version_support, VersionSupport::Supported);
        assert_eq!(harness.generation(), OpenCodeGeneration::V1);
        assert!(matches!(harness.chat_protocol(), ChatProtocol::OpenCodeServer));
    }

    #[tokio::test]
    async fn opencode_2_is_detected_as_opencode_2() {
        let (cli, env) = opencode_printing("opencode v2.1.2");
        let harness = OpenCode::default();

        harness.detect(&env, &TokioRunner, Some(&cli.binary.path)).await;

        assert_eq!(harness.generation(), OpenCodeGeneration::V2);
    }

    #[tokio::test]
    async fn a_removed_opencode_2_offers_both_release_lines() {
        let (cli, env) = opencode_printing("opencode v2.1.2");
        let harness = OpenCode::default();
        harness.detect(&env, &TokioRunner, Some(&cli.binary.path)).await;

        std::fs::remove_file(&cli.binary.path).expect("remove opencode");
        let detection = harness.detect(&env, &TokioRunner, None).await;

        assert!(!detection.installed);
        assert_eq!(harness.generation(), OpenCodeGeneration::V1);
        let labels: Vec<Option<String>> = detection
            .install_hint
            .expect("install hint")
            .commands
            .into_iter()
            .map(|command| command.label)
            .collect();
        assert_eq!(labels, [Some("OpenCode 1".to_owned()), Some("OpenCode 2".to_owned())]);
    }
}
