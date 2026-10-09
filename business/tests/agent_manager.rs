//! Launching sessions: command lines per harness, the start-request rules the manager enforces,
//! the configured approval mode and the session limit.

mod agent_support;
mod common;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_support::{ScenarioSpawner, binary, options, project, request, start_with};
use common::{demo_project, omp_home};
use harness_business::agent::{AgentDeps, AgentManager, CollectingSink, MAX_SESSIONS};
use harness_business::env::Env;
use harness_business::harness::{Harness, LaunchRequest};
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ApprovalMode, HarnessId, IpcErrorCode, SessionRef};
use harness_business::process::ResolvedBinary;
use pretty_assertions::assert_eq;

/// A manager, a project and an empty home for OMP sessions of `scenario`.
struct Setup {
    project: tempfile::TempDir,
    _home: tempfile::TempDir,
    env: Env,
    manager: AgentManager,
    spawner: Arc<ScenarioSpawner>,
}

fn setup(scenario: &str) -> Setup {
    let home = tempfile::tempdir().unwrap();

    Setup {
        project: project(),
        env: Env::for_tests(home.path()),
        _home: home,
        manager: AgentManager::with_options(options()),
        spawner: ScenarioSpawner::new("omp", scenario),
    }
}

/// OMP start dependencies over `spawner`.
fn omp_deps<'a>(env: &'a Env, spawner: &Arc<ScenarioSpawner>) -> AgentDeps<'a> {
    AgentDeps {
        harness: &Omp,
        env,
        spawner: spawner.clone(),
        binary: binary(),
    }
}

fn args(harness: &dyn Harness, launch: LaunchRequest<'_>) -> Vec<String> {
    let spec = harness.launch_spec(&launch).expect("launch spec");
    assert_eq!(spec.cwd.as_deref(), Some(launch.project));

    spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

fn launch<'a>(binary: &'a ResolvedBinary, project: &'a Path) -> LaunchRequest<'a> {
    LaunchRequest {
        binary,
        project,
        resume: None,
        approval_override: None,
        trust_once: false,
    }
}

#[test]
fn omp_runs_in_rpc_mode_with_optional_approval_override_and_resume() {
    let binary = binary();
    let project = PathBuf::from("/work/demo");
    let base = launch(&binary, &project);

    assert_eq!(args(&Omp, base), ["--mode", "rpc"]);

    let full = LaunchRequest {
        approval_override: Some(ApprovalMode::AlwaysAsk),
        resume: Some("0199c1f2"),
        trust_once: true,
        ..base
    };
    assert_eq!(
        args(&Omp, full),
        ["--mode", "rpc", "--approval-mode", "always-ask", "--resume", "0199c1f2"]
    );
}

#[test]
fn pi_passes_approve_only_for_trust_once_and_resumes_with_session() {
    let binary = binary();
    let project = PathBuf::from("/work/demo");
    let base = launch(&binary, &project);

    assert_eq!(args(&Pi, base), ["--mode", "rpc"]);

    let full = LaunchRequest {
        trust_once: true,
        resume: Some("pi-session-1"),
        approval_override: Some(ApprovalMode::Yolo),
        ..base
    };
    let expected = ["--mode", "rpc", "--approve", "--session", "pi-session-1"]; // guard-allow: approve-flag
    assert_eq!(args(&Pi, full), expected);
}

#[test]
fn no_launch_ever_disables_dialogs_skips_approvals_or_drops_the_session() {
    let binary = binary();
    let project = PathBuf::from("/work/demo");
    let forbidden = ["no-ui", "auto", "no-session", "yolo-mode", "login"]; // guard-allow: login-rpc

    for harness in [&Omp as &dyn Harness, &Pi] {
        for approval in [None, Some(ApprovalMode::Write)] {
            let all = LaunchRequest {
                approval_override: approval,
                trust_once: true,
                ..launch(&binary, &project)
            };

            for arg in args(harness, all) {
                assert!(!forbidden.iter().any(|word| arg.contains(word)), "{arg}");
            }
        }
    }
}

#[test]
fn batch_script_launchers_accept_the_rpc_arguments() {
    let batch = ResolvedBinary {
        path: PathBuf::from("C:/npm/omp.cmd"),
        is_batch_script: true,
    };
    let project = PathBuf::from("C:/work/demo");

    let plain = Omp.launch_spec(&launch(&batch, &project)).expect("safe arguments");
    assert_eq!(plain.args, [OsString::from("--mode"), OsString::from("rpc")]);
}

#[test]
fn configured_approval_mode_comes_from_the_omp_config_and_defaults_to_yolo() {
    let configured = Env::for_tests(omp_home());
    let empty = tempfile::tempdir().unwrap();

    assert_eq!(
        Omp.configured_approval_mode(&configured, None),
        Some(ApprovalMode::Write)
    );
    assert_eq!(
        Omp.configured_approval_mode(&Env::for_tests(empty.path()), Some(&demo_project())),
        Some(ApprovalMode::Yolo)
    );
    assert_eq!(Pi.configured_approval_mode(&configured, None), None);
}

#[test]
fn sessions_live_under_the_agent_dirs() {
    let env = Env::for_tests("/home/demo");

    assert_eq!(Omp.sessions_dir(&env), PathBuf::from("/home/demo/.omp/agent/sessions"));
    assert_eq!(Pi.sessions_dir(&env), PathBuf::from("/home/demo/.pi/agent/sessions"));

    let custom = env.with_var("PI_CODING_AGENT_SESSION_DIR", "/data/pi-sessions");
    assert_eq!(Pi.sessions_dir(&custom), PathBuf::from("/data/pi-sessions"));
}

#[tokio::test]
async fn start_reports_overrides_only_where_the_harness_supports_them() {
    let omp = start_with(HarnessId::Omp, "omp", "omp_out_of_order.jsonl", options(), |request| {
        request.approval_override = Some(ApprovalMode::AlwaysAsk);
        request.trust_once = true;
    })
    .await
    .unwrap();

    assert_eq!(omp.info.approval_override, Some(ApprovalMode::AlwaysAsk));
    assert_eq!(omp.info.configured_approval_mode, Some(ApprovalMode::Yolo));
    assert!(!omp.info.trusted_once);
    assert_eq!(
        (omp.info.key.as_str(), omp.info.started_at.as_str()),
        ("key-1", "2026-10-09T10:00:00Z")
    );
    let spec = &omp.spawner.requested()[0];
    assert!(spec.args.contains(&OsString::from("always-ask")));
    omp.manager.stop(omp.key()).await.unwrap();

    let pi = start_with(HarnessId::Pi, "pi", "pi_startup.jsonl", options(), |request| {
        request.approval_override = Some(ApprovalMode::AlwaysAsk);
        request.trust_once = true;
    })
    .await
    .unwrap();

    assert_eq!(pi.info.approval_override, None);
    assert_eq!(pi.info.configured_approval_mode, None);
    assert!(pi.info.trusted_once);
    pi.manager.stop(pi.key()).await.unwrap();
}

#[tokio::test]
async fn invalid_start_requests_are_refused_before_spawning() {
    let Setup {
        project,
        env,
        manager,
        spawner,
        ..
    } = setup("omp_hang.jsonl");

    let attempt = |request| manager.start(request, omp_deps(&env, &spawner), CollectingSink::new());

    let other_harness = request(HarnessId::Pi, project.path());
    assert_eq!(
        attempt(other_harness).await.unwrap_err().code(),
        IpcErrorCode::InvalidInput
    );

    let missing = request(HarnessId::Omp, &project.path().join("missing"));
    assert_eq!(attempt(missing).await.unwrap_err().code(), IpcErrorCode::NotFound);

    for id in ["--no-session", "../escape", ""] {
        let mut resume = request(HarnessId::Omp, project.path());
        resume.resume = Some(SessionRef {
            harness: HarnessId::Omp,
            id: id.to_owned(),
        });
        assert_eq!(
            attempt(resume).await.unwrap_err().code(),
            IpcErrorCode::InvalidInput,
            "{id}"
        );
    }

    assert!(spawner.requested().is_empty());
}

#[tokio::test]
async fn at_most_eight_sessions_run_at_once() {
    let Setup {
        project,
        env,
        manager,
        spawner,
        ..
    } = setup("omp_out_of_order.jsonl");

    let start = || {
        let request = request(HarnessId::Omp, project.path());
        manager.start(request, omp_deps(&env, &spawner), CollectingSink::new())
    };

    for _ in 0..MAX_SESSIONS {
        start().await.unwrap();
    }
    assert_eq!(start().await.unwrap_err().code(), IpcErrorCode::Unsupported);
    assert_eq!(manager.list().len(), MAX_SESSIONS);

    let first = manager.list()[0].key.clone();
    manager.stop(&first).await.unwrap();
    start().await.unwrap();

    manager.stop_all().await;
    assert!(manager.list().is_empty());
    manager.stop("unknown").await.unwrap();
}

#[tokio::test]
async fn switch_session_resolves_the_id_to_a_session_file_and_refreshes_the_state() {
    let run = start_with(HarnessId::Omp, "omp", "omp_switch.jsonl", options(), |_| {})
        .await
        .unwrap();
    let switch = |harness: HarnessId, id: &str| AgentAction::SwitchSession {
        session: SessionRef {
            harness,
            id: id.to_owned(),
        },
    };

    let sessions = run.home.path().join(".omp/agent/sessions/-Projects-demo");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("2026-10-09T10-00-00-000Z_abc123.jsonl"), "{}\n").unwrap();

    let missing = run.manager.send(run.key(), switch(HarnessId::Omp, "missing")).await;
    assert_eq!(missing.unwrap_err().code(), IpcErrorCode::NotFound);
    let foreign = run.manager.send(run.key(), switch(HarnessId::Pi, "abc123")).await;
    assert_eq!(foreign.unwrap_err().code(), IpcErrorCode::InvalidInput);

    let reply = run
        .manager
        .send(run.key(), switch(HarnessId::Omp, "abc123"))
        .await
        .unwrap();
    assert_eq!(reply, AgentReply::Ack);
    run.wait(
        "refreshed state",
        |event| matches!(event, AgentEvent::State { state } if state.message_count == 6),
    )
    .await;

    run.manager.stop(run.key()).await.unwrap();
}
