//! OpenCode chats started through the agent manager against the scripted `opencode serve`: the
//! command line and environment, the server password, resume, the approval override and helper
//! chats.

use crate::agent_support;
use crate::opencode_engine_support;

use std::ffi::OsString;
use std::time::Duration;

use agent_support::{Run, ScenarioSpawner, binary, options, project, request, start_with};
use harness_business::agent::{AgentDeps, AgentManager, CollectingSink, SessionOptions};
use harness_business::env::Env;
use harness_business::harnesses::OpenCode;
use harness_business::model::{AgentEvent, AgentStartRequest, ApprovalMode, HarnessId, SessionRef};
use harness_business::process::ExecSpec;
use opencode_engine_support::{SESSION, env};
use pretty_assertions::assert_eq;

const SERVE: [&str; 6] = ["serve", "--hostname", "127.0.0.1", "--port", "0", "--mdns=false"];

/// Starts scenario `name` as an OpenCode chat through the manager.
async fn start(name: &str, customize: impl FnOnce(&mut AgentStartRequest)) -> Run {
    start_with(HarnessId::OpenCode, "opencode", name, options(), customize)
        .await
        .expect("the OpenCode chat starts")
}

/// The one spawn of `run`.
fn spawned(run: &Run) -> ExecSpec {
    let mut requested = run.spawner.requested();
    assert_eq!(requested.len(), 1, "one server per chat");

    requested.remove(0)
}

/// The server password the engine gave the spawned server.
fn server_secret(spec: &ExecSpec) -> String {
    env(spec, "OPENCODE_SERVER_PASSWORD").expect("a server password")
}

fn args(spec: &ExecSpec) -> Vec<String> {
    spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

/// Waits for the `Marker` title the scenarios set once startup is over.
async fn marker(run: &Run) {
    run.wait(
        "marker",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Marker")),
    )
    .await;
}

#[tokio::test]
async fn a_chat_serves_on_loopback_with_a_fresh_password_per_launch() {
    let first = start("opencode_hang.jsonl", |_| {}).await;
    let second = start("opencode_hang.jsonl", |_| {}).await;
    first.catalog().await;
    second.catalog().await;

    let spec = spawned(&first);
    let project = dunce::canonicalize(first.project.path()).unwrap();
    assert_eq!(args(&spec), SERVE);
    assert_eq!(spec.cwd.as_deref(), Some(project.as_path()));
    assert_eq!(env(&spec, "OPENCODE_DISABLE_AUTOUPDATE").as_deref(), Some("1"));
    assert_eq!(env(&spec, "OPENCODE_SERVER_USERNAME").as_deref(), Some("opencode"));
    for prefix in ["OPENCODE_SERVER_", "OPENCODE_WORKSPACE_ID"] {
        assert!(spec.env_remove_prefixes.contains(&OsString::from(prefix)), "{prefix}");
    }

    let secret = server_secret(&spec);
    let other = server_secret(&spawned(&second));
    assert!(secret.len() >= 32, "{} characters", secret.len());
    assert_ne!(secret, other, "every launch gets its own password");

    let info = serde_json::to_string(&first.info).unwrap();
    let listed = serde_json::to_string(&first.manager.list()).unwrap();
    let events = serde_json::to_string(&first.events()).unwrap();
    let debugged = format!("{spec:?}");
    assert!(debugged.contains("OPENCODE_SERVER_PASSWORD=<redacted>"), "{debugged}");
    for shown in [info, listed, events, debugged] {
        assert!(!shown.contains(&secret), "the password leaked: {shown}");
    }
    assert_eq!(first.info.protocol_version, None);
    assert!(
        first.manager.holds_session(SESSION),
        "the new session is bound to the chat"
    );

    first.manager.stop(first.key()).await.unwrap();
    second.manager.stop(second.key()).await.unwrap();
    assert_eq!(first.spawner.exited(), 1);
}

#[tokio::test]
async fn a_failed_startup_never_shows_the_password() {
    let project = project();
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path());
    let manager = AgentManager::with_options(SessionOptions {
        startup_timeout: Duration::from_millis(300),
        ..options()
    });
    let spawner = ScenarioSpawner::new("opencode", "opencode_no_listen.jsonl");
    let sink = CollectingSink::new();
    let deps = AgentDeps {
        harness: &OpenCode::default(),
        env: &env,
        spawner: spawner.clone(),
        binary: binary(),
    };

    let error = manager
        .start(request(HarnessId::OpenCode, project.path()), deps, sink.clone())
        .await
        .expect_err("startup times out");

    let secret = server_secret(&spawner.requested()[0]);
    assert!(!error.to_string().contains(&secret), "{error}");
    assert!(!format!("{error:?}").contains(&secret));
    assert!(sink.events().is_empty(), "a failed start emits nothing");
    assert!(manager.list().is_empty());
}

#[tokio::test]
async fn resume_binds_the_session_without_naming_it_on_the_command_line() {
    let run = start("opencode_resume.jsonl", |request| {
        request.resume = Some(SessionRef {
            harness: HarnessId::OpenCode,
            id: SESSION.to_owned(),
        });
    })
    .await;

    run.wait(
        "the resumed session",
        |event| matches!(event, AgentEvent::State { state } if state.session_id == SESSION),
    )
    .await;
    assert_eq!(args(&spawned(&run)), SERVE);
    assert!(run.manager.holds_session(SESSION));

    run.manager.stop(run.key()).await.unwrap();
    assert!(!run.manager.holds_session(SESSION));
}

#[tokio::test]
async fn an_approval_override_configures_the_chats_server() {
    let run = start("opencode_mode_write.jsonl", |request| {
        request.approval_override = Some(ApprovalMode::Write);
        request.trust_once = true;
    })
    .await;

    marker(&run).await;
    assert_eq!(run.info.approval_override, Some(ApprovalMode::Write));
    assert_eq!(run.info.configured_approval_mode, None, "OpenCode's own settings apply");
    assert!(!run.info.trusted_once, "OpenCode has no project trust");

    let spawns = run.spawner.requested();
    assert_eq!(spawns.len(), 2, "the probe, then the chat's server");
    assert!(spawns.iter().all(|spec| args(spec) == SERVE));
    assert!(env(&spawns[1], "OPENCODE_CONFIG_CONTENT").is_some());

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_helper_chat_denies_every_tool_and_deletes_its_session() {
    let run = start("opencode_utility.jsonl", |request| request.utility = true).await;

    marker(&run).await;
    assert_eq!(args(&spawned(&run)), SERVE);

    run.manager.stop(run.key()).await.unwrap();
    assert!(
        run.events()
            .iter()
            .any(|event| matches!(event, AgentEvent::Stderr { line } if line == "deleted")),
        "the helper session was deleted at stop"
    );
}
