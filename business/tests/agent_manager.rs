//! Launching sessions: the start-request rules the manager enforces, the configured approval
//! mode and the session limit. One live agent per session id is in `agent_session_ids.rs`,
//! command lines are in `agent_launch.rs`.

mod agent_support;
mod common;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_support::{ScenarioSpawner, binary, options, project, request, start_with};
use common::{demo_project, omp_home};
use harness_business::agent::{AgentDeps, AgentManager, CollectingSink, MAX_SESSIONS};
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::{ApprovalMode, HarnessId, IpcErrorCode, SessionRef};
use pretty_assertions::assert_eq;

/// A manager of OMP agents playing `scenario`, a project and an empty home.
struct OmpManager {
    project: tempfile::TempDir,
    _home: tempfile::TempDir,
    env: Env,
    manager: AgentManager,
    spawner: Arc<ScenarioSpawner>,
}

fn omp_manager(scenario: &str) -> OmpManager {
    let home = tempfile::tempdir().unwrap();
    let env = Env::for_tests(home.path());

    OmpManager {
        project: project(),
        _home: home,
        env,
        manager: AgentManager::with_options(options()),
        spawner: ScenarioSpawner::new("omp", scenario),
    }
}

/// OMP start dependencies over `spawner`.
fn deps<'a>(env: &'a Env, spawner: &Arc<ScenarioSpawner>) -> AgentDeps<'a> {
    AgentDeps {
        harness: &Omp,
        env,
        spawner: spawner.clone(),
        binary: binary(),
    }
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
fn sessions_live_in_per_project_folders_under_the_agent_dirs() {
    let env = Env::for_tests("/home/demo");
    let project = Path::new("/home/demo/Projects/demo");

    assert_eq!(
        Omp.sessions_dir(&env, project),
        PathBuf::from("/home/demo/.omp/agent/sessions/-Projects-demo")
    );
    assert_eq!(
        Pi.sessions_dir(&env, project),
        PathBuf::from("/home/demo/.pi/agent/sessions/--home-demo-Projects-demo--")
    );

    let custom = env.with_var("PI_CODING_AGENT_SESSION_DIR", "/data/pi-sessions");
    assert_eq!(Pi.sessions_dir(&custom, project), PathBuf::from("/data/pi-sessions"));
    assert_eq!(Omp.sessions_dir(&custom, project), PathBuf::from("/data/pi-sessions"));
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
    let OmpManager {
        project,
        env,
        manager,
        spawner,
        ..
    } = omp_manager("omp_hang.jsonl");

    let attempt = |request| manager.start(request, deps(&env, &spawner), CollectingSink::new());

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
    let OmpManager {
        project,
        env,
        manager,
        spawner,
        ..
    } = omp_manager("omp_out_of_order.jsonl");

    let start = || {
        let request = request(HarnessId::Omp, project.path());
        manager.start(request, deps(&env, &spawner), CollectingSink::new())
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
