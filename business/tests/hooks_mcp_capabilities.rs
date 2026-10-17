//! Which Harness family members the MCP server mode (`harness-desktop mcp`) counts as there:
//! Harness Memory only when it reports its version and its memory setting is on, like the app.
//! Unix only: `~/.local/bin` is the installer's folder only on Linux and macOS.
#![cfg(unix)]

use harness_business::app_dirs::config_dir;
use harness_business::env::Env;
use harness_business::hooks_mcp::machine_capabilities;
use harness_business::process::{BoxFuture, ExecOutput, ExecSpec, ProcessRunner};

/// Answers `--version` like an installed Harness Memory (`answers`) or not at all.
struct MemoryVersion {
    answers: bool,
}

impl ProcessRunner for MemoryVersion {
    fn run<'a>(&'a self, _spec: &'a ExecSpec) -> BoxFuture<'a, harness_business::Result<ExecOutput>> {
        let stdout = if self.answers { "harness-memory 1.4.0\n" } else { "" };

        Box::pin(async move {
            Ok(ExecOutput {
                status: Some(0),
                stdout: stdout.to_owned(),
                ..ExecOutput::default()
            })
        })
    }
}

#[tokio::test]
async fn harness_memory_counts_only_when_it_answers_and_its_setting_is_on() {
    use std::os::unix::fs::PermissionsExt;

    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path());
    let bin = home.path().join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    let binary = bin.join("harness-memory");
    std::fs::write(&binary, "").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();

    let memory = |answers| {
        let env = env.clone();
        async move {
            machine_capabilities(&env, &MemoryVersion { answers })
                .await
                .memory_available
        }
    };

    assert!(
        !memory(false).await,
        "a binary that reports no version is not installed"
    );
    assert!(memory(true).await);

    let config = config_dir(&env);
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("prefs.json"), r#"{ "memoryEnabled": false }"#).unwrap();
    assert!(!memory(true).await, "the memory setting is off");
}
