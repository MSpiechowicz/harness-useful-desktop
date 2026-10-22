//! The probe start of an OpenCode chat with an approval mode against the scripted
//! `opencode serve`: it reads the agents of the directory the chat works in, refuses what it
//! cannot apply exactly, shares the startup budget with the chat's server, and is reaped whenever
//! it fails.

use crate::agent_support;
use crate::opencode_engine_support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use agent_support::{ScenarioSpawner, options};
use harness_business::agent::SessionOptions;
use harness_business::model::{ApprovalMode, IpcErrorCode};
use harness_business::opencode_server::mode_config::config_rules;
use harness_business::opencode_server::modes::{Rule, agent_rules, evaluate};
use harness_business::process::{ExecSpec, ProcessSpawner, SpawnedProcess};
use harness_business::{BusinessError, Result};
use opencode_engine_support::{Chat, Launch, SESSION, env, start_as, start_env, start_through, until_marker};
use pretty_assertions::assert_eq;
use serde_json::Value;

const CONTENT: &str = "OPENCODE_CONFIG_CONTENT";

fn launch(name: &str, approval: Option<ApprovalMode>, options: SessionOptions) -> Launch<'_> {
    Launch {
        name,
        resume: None,
        approval,
        utility: false,
        options,
        env: &[],
    }
}

/// The error a start ended with.
fn refused(chat: &Chat) -> &BusinessError {
    chat.session.as_ref().err().expect("the start fails")
}

/// Waits until every server the chat spawned was reaped.
async fn all_reaped(chat: &Chat) {
    let deadline = tokio::time::Instant::now() + agent_support::WAIT;

    while chat.spawner.exited() < chat.spawner.requested().len() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "{} of {} reaped",
            chat.spawner.exited(),
            chat.spawner.requested().len()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_resumed_session_in_a_folder_gets_the_mode_of_that_folders_agents() {
    let chat = start_as(
        "opencode_mode_resume_folder.jsonl",
        Some(SESSION),
        Some(ApprovalMode::Yolo),
        false,
        options(),
    )
    .await;
    assert!(chat.session.is_ok(), "{:?}", chat.session.as_ref().err());
    until_marker(&chat).await;

    let server = chat.spawner.requested().pop().expect("the chat's server");
    let content: Value = serde_json::from_str(&env(&server, CONTENT).expect("the mode")).expect("JSON");

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/opencode/server/agent-list-src.json"
    );
    let folder: Value = serde_json::from_str(&std::fs::read_to_string(path).expect("recording")).expect("JSON");
    let mut rules: Vec<Rule> = agent_rules(&folder["body"], "build");
    rules.extend(config_rules(&content["agent"]["build"]["permission"], None));
    let decide = |command: &str| evaluate(&rules, "bash", command).map(|rule| rule.action.clone());

    assert_eq!(
        decide("rm -rf target").as_deref(),
        Some("deny"),
        "the folder's deny stays in Yolo"
    );
    assert_eq!(decide("ls").as_deref(), Some("allow"));

    chat.session().stop().await;
}

#[tokio::test]
async fn a_resumed_session_with_rules_of_its_own_refuses_a_mode() {
    let chat = start_as(
        "opencode_mode_resume_stored.jsonl",
        Some(SESSION),
        Some(ApprovalMode::AlwaysAsk),
        false,
        options(),
    )
    .await;

    let error = refused(&chat);
    assert_eq!(error.code(), IpcErrorCode::Unsupported);
    assert!(error.to_string().contains("permission rules of its own"), "{error}");
    assert_eq!(chat.spawner.requested().len(), 1, "only the probe started");
    all_reaped(&chat).await;
}

#[tokio::test]
async fn the_same_session_without_a_mode_starts_as_before() {
    let chat = start_as(
        "opencode_mode_resume_stored.jsonl",
        Some(SESSION),
        None,
        false,
        options(),
    )
    .await;

    assert!(chat.session.is_ok(), "{:?}", chat.session.as_ref().err());
    chat.session().stop().await;
}

#[tokio::test]
async fn two_starts_share_twice_the_startup_deadline() {
    let roomy = SessionOptions {
        startup_deadline: Duration::from_millis(700),
        ..options()
    };
    let chat = start_through(
        launch("opencode_slow_listen.jsonl", Some(ApprovalMode::Write), roomy),
        |spawner| spawner,
        Box::pin(std::future::pending()),
    )
    .await;
    assert!(
        chat.session.is_ok(),
        "two 400 ms starts fit 1.4 s: {:?}",
        chat.session.as_ref().err()
    );
    chat.session().stop().await;

    let tight = SessionOptions {
        startup_deadline: Duration::from_millis(300),
        ..options()
    };
    let chat = start_through(
        launch("opencode_slow_listen.jsonl", Some(ApprovalMode::Write), tight),
        |spawner| spawner,
        Box::pin(std::future::pending()),
    )
    .await;
    let error = refused(&chat);
    assert!(matches!(error, BusinessError::Timeout(_)), "{error:?}");
    assert!(
        error.to_string().contains("within 0.6 s"),
        "one budget for both starts: {error}"
    );
    all_reaped(&chat).await;

    let during_probe = SessionOptions {
        startup_deadline: Duration::from_millis(150),
        ..options()
    };
    let chat = start_through(
        launch("opencode_slow_listen.jsonl", Some(ApprovalMode::Write), during_probe),
        |spawner| spawner,
        Box::pin(std::future::pending()),
    )
    .await;
    assert!(
        matches!(refused(&chat), BusinessError::Timeout(_)),
        "{:?}",
        refused(&chat)
    );
    assert_eq!(chat.spawner.requested().len(), 1, "given up during the probe");
    all_reaped(&chat).await;

    let single = SessionOptions {
        startup_deadline: Duration::from_millis(300),
        ..options()
    };
    let chat = start_through(
        launch("opencode_slow_listen.jsonl", None, single),
        |spawner| spawner,
        Box::pin(std::future::pending()),
    )
    .await;
    assert!(
        refused(&chat).to_string().contains("within 0.3 s"),
        "no mode, one start, one deadline"
    );
    all_reaped(&chat).await;
}

#[tokio::test]
async fn a_probe_that_fails_is_reaped_with_a_clear_error() {
    let no_address = SessionOptions {
        startup_timeout: Duration::from_millis(300),
        ..options()
    };

    for (scenario, options, expected) in [
        ("opencode_no_listen.jsonl", no_address, "server address"),
        ("opencode_exit_early.jsonl", options(), "exited during startup"),
        ("opencode_health_error.jsonl", options(), "HTTP 500"),
    ] {
        let chat = start_through(
            launch(scenario, Some(ApprovalMode::Yolo), options),
            |spawner| spawner,
            Box::pin(std::future::pending()),
        )
        .await;

        let error = refused(&chat);
        assert!(error.to_string().contains(expected), "{scenario}: {error}");
        assert_eq!(chat.spawner.requested().len(), 1, "{scenario}: only the probe started");
        all_reaped(&chat).await;
    }
}

#[tokio::test]
async fn a_stop_during_the_probe_gives_up_and_reaps_it() {
    let abort = Box::pin(tokio::time::sleep(Duration::from_millis(150)));
    let chat = start_through(
        launch("opencode_slow_listen.jsonl", Some(ApprovalMode::Yolo), options()),
        |spawner| spawner,
        abort,
    )
    .await;

    assert!(
        refused(&chat).to_string().contains("stopped while it started"),
        "{}",
        refused(&chat)
    );
    assert_eq!(chat.spawner.requested().len(), 1);
    all_reaped(&chat).await;
}

#[tokio::test]
async fn a_configuration_too_large_for_the_environment_refuses_the_mode() {
    let large = format!("{{\"instructions\": [\"{}\"]}}", "x".repeat(120 * 1024));
    let chat = start_env(
        "opencode_mode_write.jsonl",
        None,
        Some(ApprovalMode::Write),
        options(),
        &[(CONTENT, &large)],
    )
    .await;

    let error = refused(&chat);
    assert_eq!(error.code(), IpcErrorCode::Unsupported);
    assert!(error.to_string().contains("KiB"), "{error}");
    assert_eq!(chat.spawner.requested().len(), 1, "only the probe started");
    all_reaped(&chat).await;
}

/// Hands the first spawn to the scenario spawner and fails every later one.
struct FailsAfterFirst {
    inner: Arc<dyn ProcessSpawner>,
    spawned: AtomicUsize,
}

impl ProcessSpawner for FailsAfterFirst {
    fn spawn(&self, spec: &ExecSpec) -> Result<SpawnedProcess> {
        if self.spawned.fetch_add(1, Ordering::SeqCst) == 0 {
            return self.inner.spawn(spec);
        }

        Err(BusinessError::Process("argument list too long".to_owned()))
    }
}

#[tokio::test]
async fn a_server_that_cannot_start_with_the_mode_is_a_clear_refusal() {
    let failing = |inner: Arc<ScenarioSpawner>| -> Arc<dyn ProcessSpawner> {
        Arc::new(FailsAfterFirst {
            inner,
            spawned: AtomicUsize::new(0),
        })
    };
    let chat = start_through(
        launch("opencode_mode_write.jsonl", Some(ApprovalMode::Write), options()),
        failing,
        Box::pin(std::future::pending()),
    )
    .await;

    let error = refused(&chat);
    assert_eq!(error.code(), IpcErrorCode::Unsupported);
    assert!(error.to_string().contains("approval mode's configuration"), "{error}");
    assert!(error.to_string().contains("argument list too long"), "{error}");
    all_reaped(&chat).await;
}
