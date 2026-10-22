//! Harness detection: binary lookup, override validation, version parsing and support, install
//! hints, and the registry.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use harness_business::env::{Env, OsKind};
use harness_business::harness::{Harness, HarnessRegistry, first_semver, override_name_matches};
use harness_business::harnesses::{Claude, Omp, OpenCode, Pi};
use harness_business::model::{HarnessId, VersionSupport};
#[cfg(unix)]
use harness_business::process::TokioRunner;
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};
use harness_business::{BusinessError, Result};
use pretty_assertions::assert_eq;
use tempfile::TempDir;

/// A runner that answers every command with a canned output and records what it was asked.
struct CannedRunner {
    output: ExecOutput,
    seen: Mutex<Vec<ExecSpec>>,
}

impl CannedRunner {
    fn new(output: ExecOutput) -> Self {
        Self {
            output,
            seen: Mutex::new(Vec::new()),
        }
    }

    fn printing(stdout: &str) -> Self {
        Self::new(ExecOutput {
            status: Some(0),
            stdout: stdout.to_owned(),
            ..ExecOutput::default()
        })
    }
}

impl ProcessRunner for CannedRunner {
    fn run<'a>(&'a self, spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        self.seen.lock().unwrap().push(spec.clone());
        let output = self.output.clone();
        Box::pin(async move { Ok(output) })
    }
}

/// A runner that must never be called.
struct NoRunner;

impl ProcessRunner for NoRunner {
    fn run<'a>(&'a self, _spec: &'a ExecSpec) -> BoxFuture<'a, Result<ExecOutput>> {
        Box::pin(async { Err(BusinessError::Internal("unexpected process".into())) })
    }
}

/// An executable named `name` in `dir` (content is irrelevant for canned runners).
fn fake_binary(dir: &Path, name: &str, script: &str) -> PathBuf {
    let path = dir.join(name);
    fs::create_dir_all(dir).unwrap();
    fs::write(&path, script).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    path
}

fn binary_file(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// Asserts that the first command `runner` saw ran `binary --version` with `switch=1` set.
fn assert_version_probe(runner: &CannedRunner, binary: &Path, switch: &str) {
    let seen = runner.seen.lock().unwrap();

    assert_eq!(seen[0].program, binary);
    assert_eq!(seen[0].args, ["--version"]);
    assert!(
        seen[0].env.iter().any(|(name, value)| name == switch && value == "1"),
        "{:?}",
        seen[0].env
    );
}

/// An empty home whose PATH holds only `bin`.
fn env_with_bin(home: &TempDir) -> (Env, PathBuf) {
    let bin = home.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let env = Env::for_tests(home.path()).with_var("PATH", bin.to_string_lossy());

    (env, bin)
}

#[test]
fn versions_are_read_from_omp_and_pi_output() {
    assert_eq!(first_semver("omp/18.6.1\n").as_deref(), Some("18.6.1"));
    assert_eq!(first_semver("1.1.0").as_deref(), Some("1.1.0"));
    assert_eq!(
        first_semver("pi v2.0.0-beta.1 (linux)").as_deref(),
        Some("2.0.0-beta.1")
    );
    assert_eq!(first_semver("unknown"), None);
}

#[test]
fn omp_18_is_supported_and_everything_else_untested() {
    assert_eq!(Omp.version_support("18.6.1"), VersionSupport::Supported);
    assert_eq!(Omp.version_support("18.0.0"), VersionSupport::Supported);
    assert_eq!(Omp.version_support("19.0.0"), VersionSupport::Untested);
    assert_eq!(Omp.version_support("1.18.0"), VersionSupport::Untested);
}

#[test]
fn pi_1_1_is_supported_and_everything_else_untested() {
    assert_eq!(Pi.version_support("1.1.0"), VersionSupport::Supported);
    assert_eq!(Pi.version_support("1.1.7"), VersionSupport::Supported);
    assert_eq!(Pi.version_support("1.2.0"), VersionSupport::Untested);
    assert_eq!(Pi.version_support("11.1.0"), VersionSupport::Untested);
}

#[test]
fn overrides_must_be_named_like_the_harness_binary() {
    let cases = [
        (OsKind::Linux, "/opt/omp/bin/omp", true),
        (OsKind::Linux, "/opt/bin/omp.sh", false),
        (OsKind::Linux, "/opt/bin/OMP", false),
        (OsKind::Linux, "/tmp/evil", false),
        (OsKind::MacOs, "/usr/local/bin/omp", true),
        (OsKind::Windows, "C:/npm/omp.cmd", true),
        (OsKind::Windows, "C:/npm/OMP.EXE", true),
        (OsKind::Windows, "C:/npm/omp.bat", true),
        (OsKind::Windows, "C:/npm/omp.ps1", false),
        (OsKind::Windows, "C:/npm/omp-helper.exe", false),
    ];

    for (os, path, expected) in cases {
        assert_eq!(
            override_name_matches(os, "omp", Path::new(path)),
            expected,
            "{os:?} {path}"
        );
    }
}

#[tokio::test]
async fn a_found_binary_is_probed_with_its_version_flag() {
    let home = TempDir::new().unwrap();
    let (env, bin) = env_with_bin(&home);
    let binary = fake_binary(&bin, &binary_file("omp"), "");
    let runner = CannedRunner::printing("omp/18.6.1\n");

    let detection = Omp.detect(&env, &runner, None).await;

    assert!(detection.installed);
    assert_eq!(
        detection.binary_path.as_deref(),
        Some(binary.to_string_lossy().as_ref())
    );
    assert_eq!(detection.version.as_deref(), Some("18.6.1"));
    assert_eq!(detection.version_support, VersionSupport::Supported);
    assert_eq!(detection.install_hint, None);
    assert_eq!(detection.error, None);
    assert_eq!(
        detection.config_dir,
        Some(home.path().join(".omp").to_string_lossy().into_owned())
    );
    assert!(!detection.config_dir_exists);

    let seen = runner.seen.lock().unwrap();
    assert_eq!(seen[0].program, binary);
    assert_eq!(seen[0].args, ["--version"]);
    assert_eq!(seen[0].timeout.as_secs(), 5);
}

#[tokio::test]
async fn a_missing_binary_reports_the_install_hint() {
    let home = TempDir::new().unwrap();
    let (env, _) = env_with_bin(&home);

    let detection = Pi.detect(&env, &NoRunner, None).await;

    assert!(!detection.installed);
    assert_eq!(detection.version_support, VersionSupport::Unknown);
    let hint = detection.install_hint.unwrap();
    assert_eq!(hint.commands.len(), 1);
    assert_eq!(hint.commands[0].label, None);
    assert_eq!(hint.commands[0].command, "curl -fsSL https://pi.dev/install.sh | sh");
    assert_eq!(hint.docs_url, "https://github.com/earendil-works/pi");
}

#[tokio::test]
async fn a_misnamed_override_is_ignored_with_an_explanation() {
    let home = TempDir::new().unwrap();
    let (env, _) = env_with_bin(&home);
    let impostor = fake_binary(&home.path().join("tools"), &binary_file("not-omp"), "");

    let detection = Omp.detect(&env, &NoRunner, Some(&impostor)).await;

    assert!(!detection.installed);
    assert_eq!(
        detection.error.as_deref(),
        Some("Ignored the custom binary: its file name must be `omp`.")
    );
}

#[tokio::test]
async fn probe_failures_are_reported_without_hiding_the_binary() {
    let home = TempDir::new().unwrap();
    let (env, bin) = env_with_bin(&home);
    fake_binary(&bin, &binary_file("omp"), "");

    let timed_out = CannedRunner::new(ExecOutput {
        timed_out: true,
        ..ExecOutput::default()
    });
    let failed = CannedRunner::new(ExecOutput {
        status: Some(2),
        ..ExecOutput::default()
    });
    let garbled = CannedRunner::printing("no version here");

    let cases = [
        (timed_out, "`omp --version` did not answer within 5 s."),
        (failed, "`omp --version` failed with exit code 2."),
        (garbled, "Could not read a version from `omp --version`."),
    ];

    for (runner, message) in cases {
        let detection = Omp.detect(&env, &runner, None).await;

        assert!(detection.installed);
        assert_eq!(detection.version, None);
        assert_eq!(detection.version_support, VersionSupport::Unknown);
        assert_eq!(detection.error.as_deref(), Some(message));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_real_fake_omp_on_path_is_detected() {
    let home = TempDir::new().unwrap();
    let (env, bin) = env_with_bin(&home);
    fake_binary(&bin, "omp", "#!/bin/sh\necho omp/18.6.1\n"); // guard-allow: installer-shell
    fs::create_dir_all(home.path().join(".omp")).unwrap();

    let detection = Omp.detect(&env, &TokioRunner, None).await;

    assert_eq!(detection.version.as_deref(), Some("18.6.1"));
    assert_eq!(detection.version_support, VersionSupport::Supported);
    assert!(detection.config_dir_exists);
}

/// A Finder-launched app has launchd's `PATH`: a bun-installed `omp` (`#!/usr/bin/env bun`) is
/// found in `~/.bun/bin`, and its interpreter next to it must be found too.
#[cfg(unix)]
#[tokio::test]
async fn a_launcher_script_finds_its_interpreter_outside_the_inherited_path() {
    let home = TempDir::new().unwrap();
    let bun_bin = home.path().join(".bun/bin");
    fake_binary(&bun_bin, "harness-test-bun", "#!/bin/sh\necho omp/18.6.1\n"); // guard-allow: installer-shell
    fake_binary(&bun_bin, "omp", "#!/usr/bin/env harness-test-bun\n");
    let env = Env::for_tests(home.path()).with_var("PATH", "/usr/bin:/bin");

    let detection = Omp.detect(&env, &TokioRunner, None).await;

    assert_eq!(detection.error, None);
    assert_eq!(detection.version.as_deref(), Some("18.6.1"));
}

#[cfg(unix)]
#[tokio::test]
async fn the_registry_detects_every_harness_and_honours_overrides() {
    use harness_business::model::BinaryOverride;

    let home = TempDir::new().unwrap();
    let (env, _) = env_with_bin(&home);
    let custom = fake_binary(&home.path().join("custom"), "pi", "#!/bin/sh\necho 1.1.0\n"); // guard-allow: installer-shell
    let overrides = [BinaryOverride {
        harness: HarnessId::Pi,
        path: custom.to_string_lossy().into_owned(),
    }];

    let summaries = HarnessRegistry::with_defaults()
        .summaries(&env, &TokioRunner, &overrides)
        .await;

    let ids: Vec<HarnessId> = summaries.iter().map(|summary| summary.info.id).collect();
    assert_eq!(ids, HarnessId::ALL);
    assert!(!summaries[0].detection.installed);
    assert_eq!(summaries[1].detection.version.as_deref(), Some("1.1.0"));
    assert_eq!(summaries[1].detection.version_support, VersionSupport::Supported);
}

#[test]
fn the_registry_lists_every_harness() {
    let registry = HarnessRegistry::with_defaults();

    let ids: Vec<HarnessId> = registry.all().map(|harness| harness.id()).collect();
    assert_eq!(ids, HarnessId::ALL);

    for id in HarnessId::ALL {
        let harness = registry.get(id).unwrap();
        assert_eq!(harness.info().id, id);
        assert_eq!(harness.binary_name(), id.binary_name());
    }

    assert_eq!(registry.get(HarnessId::Omp).unwrap().info().homepage, "https://omp.sh");
    assert_eq!(
        registry.get(HarnessId::OpenCode).unwrap().info().homepage,
        "https://opencode.ai"
    );
    assert_eq!(
        registry.get(HarnessId::Claude).unwrap().info().homepage,
        "https://code.claude.com"
    );
}

#[test]
fn opencode_1_18_and_1_19_are_supported_and_everything_else_untested() {
    assert_eq!(
        OpenCode::default().version_support("1.18.35"),
        VersionSupport::Supported
    );
    assert_eq!(OpenCode::default().version_support("1.19.3"), VersionSupport::Supported);
    assert_eq!(OpenCode::default().version_support("1.17.9"), VersionSupport::Untested);
    assert_eq!(OpenCode::default().version_support("2.0.0"), VersionSupport::Untested);
    assert_eq!(OpenCode::default().version_support("11.18.0"), VersionSupport::Untested);
}

#[tokio::test]
async fn opencode_prints_a_bare_version_and_is_probed_without_updating_itself() {
    let home = TempDir::new().unwrap();
    let (env, bin) = env_with_bin(&home);
    let binary = fake_binary(&bin, &binary_file("opencode"), "");
    let runner = CannedRunner::printing("1.18.35\n");

    let detection = OpenCode::default().detect(&env, &runner, None).await;

    assert!(detection.installed);
    assert_eq!(detection.version.as_deref(), Some("1.18.35"));
    assert_eq!(detection.version_support, VersionSupport::Supported);
    assert_eq!(detection.error, None);
    assert_eq!(
        detection.config_dir,
        Some(
            home.path()
                .join(".config")
                .join("opencode")
                .to_string_lossy()
                .into_owned()
        )
    );

    assert_version_probe(&runner, &binary, "OPENCODE_DISABLE_AUTOUPDATE");
}

#[tokio::test]
async fn opencode_is_found_where_its_install_script_puts_it() {
    let home = TempDir::new().unwrap();
    let (env, _) = env_with_bin(&home);
    let binary = fake_binary(&home.path().join(".opencode").join("bin"), &binary_file("opencode"), "");
    let runner = CannedRunner::printing("1.19.3\n");

    let detection = OpenCode::default().detect(&env, &runner, None).await;

    assert!(detection.installed);
    assert_eq!(
        detection.binary_path.as_deref(),
        Some(binary.to_string_lossy().as_ref())
    );
    assert_eq!(detection.version_support, VersionSupport::Supported);
}

#[tokio::test]
async fn a_missing_opencode_offers_both_release_lines() {
    let home = TempDir::new().unwrap();
    let (env, _) = env_with_bin(&home);

    let detection = OpenCode::default().detect(&env, &NoRunner, None).await;

    assert!(!detection.installed);
    let hint = detection.install_hint.unwrap();
    let opencode_1 = if cfg!(windows) {
        "npm install -g opencode-ai"
    } else {
        "curl -fsSL https://opencode.ai/install | bash"
    };
    let commands: Vec<(Option<&str>, &str)> = hint
        .commands
        .iter()
        .map(|command| (command.label.as_deref(), command.command.as_str()))
        .collect();
    assert_eq!(
        commands,
        [
            (Some("OpenCode 1"), opencode_1),
            (Some("OpenCode 2"), "npm install -g @opencode/cli"),
        ]
    );
    assert_eq!(hint.docs_url, "https://opencode.ai/docs/");
}

#[tokio::test]
async fn the_first_detection_of_opencode_2_shows_the_config_dir_it_reads() {
    let home = TempDir::new().unwrap();
    let (env, bin) = env_with_bin(&home);
    let team = home.path().join("team-opencode");
    let env = env.with_var("OPENCODE_CONFIG_DIR", team.to_string_lossy());
    fake_binary(&bin, &binary_file("opencode"), "");
    let runner = CannedRunner::printing("opencode v2.1.2\n");

    let detection = OpenCode::default().detect(&env, &runner, None).await;

    assert_eq!(detection.version.as_deref(), Some("2.1.2"));
    assert_eq!(detection.config_dir, Some(team.to_string_lossy().into_owned()));
}

#[test]
fn claude_2_1_is_supported_and_everything_else_untested() {
    assert_eq!(first_semver("2.1.304 (Claude Code)\n").as_deref(), Some("2.1.304"));
    assert_eq!(Claude.version_support("2.1.304"), VersionSupport::Supported);
    assert_eq!(Claude.version_support("2.2.0"), VersionSupport::Untested);
    assert_eq!(Claude.version_support("1.0.128"), VersionSupport::Untested);
}

#[tokio::test]
async fn claude_is_probed_with_its_updater_off_and_shows_its_config_dir() {
    let home = TempDir::new().unwrap();
    let (env, bin) = env_with_bin(&home);
    let custom = home.path().join("claude-work");
    let env = env.with_var("CLAUDE_CONFIG_DIR", custom.to_string_lossy());
    let binary = fake_binary(&bin, &binary_file("claude"), "");
    let runner = CannedRunner::printing("2.1.304 (Claude Code)\n");

    let detection = Claude.detect(&env, &runner, None).await;

    assert!(detection.installed);
    assert_eq!(detection.version.as_deref(), Some("2.1.304"));
    assert_eq!(detection.version_support, VersionSupport::Supported);
    assert_eq!(detection.error, None);
    assert_eq!(detection.config_dir, Some(custom.to_string_lossy().into_owned()));

    assert_version_probe(&runner, &binary, "DISABLE_UPDATES");
}

#[tokio::test]
async fn a_missing_claude_offers_the_native_installer_of_the_os() {
    let home = TempDir::new().unwrap();
    let (env, _) = env_with_bin(&home);

    let detection = Claude.detect(&env, &NoRunner, None).await;

    assert!(!detection.installed);
    let hint = detection.install_hint.unwrap();
    let command = if cfg!(windows) {
        "irm https://claude.ai/install.ps1 | iex"
    } else {
        "curl -fsSL https://claude.ai/install.sh | bash"
    };
    assert_eq!(hint.commands.len(), 1);
    assert_eq!(hint.commands[0].label, None);
    assert_eq!(hint.commands[0].command, command);
    assert_eq!(hint.docs_url, "https://code.claude.com/docs/en/setup");
}
