//! One live agent per session id: resumes, switches and claims of an id a running, starting or
//! stopping agent of any harness holds, and starts in flight when every session stops.

mod agent_support;

use std::sync::Arc;
use std::time::Duration;

use agent_support::{ScenarioSpawner, WAIT, binary, options, project, request, start_with};
use harness_business::agent::{AgentDeps, AgentManager, CollectingSink, SessionOptions};
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::{Omp, Pi};
use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, AgentSessionInfo, AgentStartRequest, HarnessId, IpcErrorCode, SessionRef,
};
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
    setup_with(scenario, options())
}

fn setup_with(scenario: &str, options: SessionOptions) -> Setup {
    let home = tempfile::tempdir().unwrap();

    Setup {
        project: project(),
        env: Env::for_tests(home.path()),
        _home: home,
        manager: AgentManager::with_options(options),
        spawner: ScenarioSpawner::new("omp", scenario),
    }
}

/// The session id every OMP fixture reports in its state.
const REPORTED_ID: &str = "0199c1f2-5d3e-7a10-9b2c-4e6f8a1d3c57";

/// Stop grace of [`hanging_setup`].
const HANG_GRACE: Duration = Duration::from_millis(300);

/// Sessions whose agent ignores stdin EOF, so it exits only when killed after [`HANG_GRACE`].
fn hanging_setup() -> Setup {
    setup_with(
        "omp_hang.jsonl",
        SessionOptions {
            stop_grace: HANG_GRACE,
            ..options()
        },
    )
}

impl Setup {
    /// Starts an agent resuming OMP session `id`.
    async fn resume(&self, id: &str, sink: Arc<CollectingSink>) -> harness_business::Result<AgentSessionInfo> {
        let start = AgentStartRequest {
            resume: Some(SessionRef {
                harness: HarnessId::Omp,
                id: id.to_owned(),
            }),
            ..request(HarnessId::Omp, self.project.path())
        };

        self.manager
            .start(start, omp_deps(&self.env, &self.spawner), sink)
            .await
    }

    /// Starts an agent on a new OMP session and waits for its startup state.
    async fn start_new(&self) -> AgentSessionInfo {
        let sink = CollectingSink::new();
        let start = request(HarnessId::Omp, self.project.path());
        let info = self
            .manager
            .start(start, omp_deps(&self.env, &self.spawner), sink.clone());
        let info = info.await.unwrap();

        let state = sink.wait_for(WAIT, |event| matches!(event, AgentEvent::State { .. }));
        assert!(state.await.is_some(), "no startup state");
        info
    }

    /// Writes an OMP session file for `id`, so a switch to it resolves.
    fn write_session(&self, id: &str) {
        let project = dunce::canonicalize(self.project.path()).unwrap();
        let sessions = Omp.sessions_dir(&self.env, &project);
        std::fs::create_dir_all(&sessions).unwrap();
        std::fs::write(sessions.join(format!("2026-10-09T10-00-00-000Z_{id}.jsonl")), "{}\n").unwrap();
    }
}

/// Switches to OMP session `id`.
fn switch_to(id: &str) -> AgentAction {
    AgentAction::SwitchSession {
        session: SessionRef {
            harness: HarnessId::Omp,
            id: id.to_owned(),
        },
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

    let project = dunce::canonicalize(run.project.path()).unwrap();
    let sessions = Omp.sessions_dir(&Env::for_tests(run.home.path()), &project);
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::write(sessions.join("2026-10-09T10-00-00-000Z_abc123.jsonl"), "{}\n").unwrap();
    run.wait("startup state", |event| matches!(event, AgentEvent::State { .. }))
        .await;

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
    assert!(run.manager.holds_session("abc123"), "bound before the reply");
    assert!(!run.manager.holds_session(REPORTED_ID));
    run.wait(
        "refreshed state",
        |event| matches!(event, AgentEvent::State { state } if state.message_count == 6),
    )
    .await;

    run.manager.stop(run.key()).await.unwrap();
}

#[tokio::test]
async fn a_session_open_in_a_chat_cannot_be_resumed_in_another() {
    let setup = setup("omp_out_of_order.jsonl");
    let manager = &setup.manager;

    setup.resume(REPORTED_ID, CollectingSink::new()).await.unwrap();
    assert!(manager.holds_session(REPORTED_ID));

    let again = setup.resume(REPORTED_ID, CollectingSink::new()).await.unwrap_err();
    assert_eq!(again.code(), IpcErrorCode::Unsupported);
    assert_eq!(setup.spawner.requested().len(), 1);

    setup.resume("another-session", CollectingSink::new()).await.unwrap();
    assert_eq!(manager.list().len(), 2);

    manager.stop_all().await;
    assert!(!manager.holds_session(REPORTED_ID));
}

#[tokio::test]
async fn a_new_session_holds_the_id_it_reports() {
    let setup = setup("omp_out_of_order.jsonl");
    let sink = CollectingSink::new();
    let start = request(HarnessId::Omp, setup.project.path());
    let deps = omp_deps(&setup.env, &setup.spawner);

    let info = setup.manager.start(start, deps, sink.clone()).await.unwrap();
    let state = sink.wait_for(WAIT, |event| matches!(event, AgentEvent::State { .. }));
    assert!(state.await.is_some());
    assert!(setup.manager.holds_session(REPORTED_ID));

    let again = setup.resume(REPORTED_ID, CollectingSink::new()).await.unwrap_err();
    assert_eq!(again.code(), IpcErrorCode::Unsupported);

    setup.manager.stop(&info.key).await.unwrap();
    assert!(!setup.manager.holds_session(REPORTED_ID));
}

#[tokio::test]
async fn two_chats_resuming_one_session_at_once_start_one_agent() {
    let setup = setup("omp_out_of_order.jsonl");

    let (left, right) = tokio::join!(
        setup.resume(REPORTED_ID, CollectingSink::new()),
        setup.resume(REPORTED_ID, CollectingSink::new())
    );

    let refused = [&left, &right]
        .into_iter()
        .filter(|result| {
            result
                .as_ref()
                .is_err_and(|error| error.code() == IpcErrorCode::Unsupported)
        })
        .count();
    assert_eq!(refused, 1, "{left:?} {right:?}");
    assert_eq!(setup.spawner.requested().len(), 1);

    setup.manager.stop_all().await;
}

#[tokio::test]
async fn resuming_a_stopping_session_waits_for_its_agent_to_exit() {
    let setup = hanging_setup();
    let manager = &setup.manager;

    let first = CollectingSink::new();
    let info = setup.resume(REPORTED_ID, first.clone()).await.unwrap();

    let stopping = manager.begin_stop(&info.key).expect("a running session");
    assert!(manager.list().is_empty());
    let stopped = tokio::spawn(stopping.finish());

    let started = tokio::time::Instant::now();
    setup.resume(REPORTED_ID, CollectingSink::new()).await.unwrap();

    assert!(started.elapsed() >= HANG_GRACE, "{:?}", started.elapsed());
    assert!(matches!(first.events().last(), Some(AgentEvent::Exited { .. })));
    assert_eq!(setup.spawner.requested().len(), 2);

    stopped.await.unwrap();
    manager.stop_all().await;
}

#[tokio::test]
async fn stopping_sessions_hold_their_ids_until_their_agents_exit() {
    let setup = hanging_setup();
    let manager = &setup.manager;

    setup.resume(REPORTED_ID, CollectingSink::new()).await.unwrap();
    assert!(manager.begin_stop("unknown").is_none());

    let stopping = manager.begin_stop_all();
    assert!(manager.list().is_empty());
    assert!(manager.holds_session(REPORTED_ID));

    stopping.finish().await;
    assert!(!manager.holds_session(REPORTED_ID));

    setup.resume(REPORTED_ID, CollectingSink::new()).await.unwrap();
    manager.stop_all().await;
}

#[tokio::test]
async fn a_claimed_session_cannot_be_resumed_until_the_claim_is_dropped() {
    let setup = setup("omp_out_of_order.jsonl");
    let claim = setup.manager.claim_session(REPORTED_ID).expect("a free id");
    assert!(setup.manager.claim_session(REPORTED_ID).is_none());

    let refused = setup.resume(REPORTED_ID, CollectingSink::new()).await.unwrap_err();
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);
    assert!(setup.spawner.requested().is_empty());

    drop(claim);
    setup.resume(REPORTED_ID, CollectingSink::new()).await.unwrap();
    assert!(setup.manager.claim_session(REPORTED_ID).is_none(), "its agent holds it");

    setup.manager.stop_all().await;
}

#[tokio::test]
async fn a_start_in_flight_when_every_session_stops_stops_its_agent() {
    let setup = setup("omp_slow_startup.jsonl");
    let sink = CollectingSink::new();

    // Stops everything once the agent was spawned, while its startup is still pending.
    let stop_all = async {
        while setup.spawner.requested().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        setup.manager.stop_all().await;
    };
    let (started, ()) = tokio::join!(setup.resume(REPORTED_ID, sink.clone()), stop_all);

    assert_eq!(started.unwrap_err().code(), IpcErrorCode::Process);
    assert!(matches!(sink.events().last(), Some(AgentEvent::Exited { .. })));
    assert!(setup.manager.list().is_empty());
    assert!(!setup.manager.holds_session(REPORTED_ID));
}

#[tokio::test]
async fn a_chat_cannot_switch_to_a_session_held_elsewhere() {
    let setup = setup("omp_switch.jsonl");
    let manager = &setup.manager;
    setup.write_session("abc123");
    setup.write_session("deleting");

    let left = setup.start_new().await;
    let right = setup.start_new().await;
    assert_eq!(
        manager.send(&left.key, switch_to("abc123")).await.unwrap(),
        AgentReply::Ack
    );

    let taken = manager.send(&right.key, switch_to("abc123")).await.unwrap_err();
    assert_eq!(taken.code(), IpcErrorCode::Unsupported);

    let claim = manager.claim_session("deleting").expect("a free id");
    let claimed = manager.send(&right.key, switch_to("deleting")).await.unwrap_err();
    assert_eq!(claimed.code(), IpcErrorCode::Unsupported);

    drop(claim);
    assert_eq!(
        manager.send(&right.key, switch_to("deleting")).await.unwrap(),
        AgentReply::Ack
    );

    manager.stop_all().await;
}

#[tokio::test]
async fn an_omp_agent_holds_its_session_id_for_pi_too() {
    let setup = setup("omp_out_of_order.jsonl");
    setup.resume(REPORTED_ID, CollectingSink::new()).await.unwrap();

    let pi_spawner = ScenarioSpawner::new("pi", "pi_startup.jsonl");
    let start = AgentStartRequest {
        resume: Some(SessionRef {
            harness: HarnessId::Pi,
            id: REPORTED_ID.to_owned(),
        }),
        ..request(HarnessId::Pi, setup.project.path())
    };
    let deps = AgentDeps {
        harness: &Pi,
        env: &setup.env,
        spawner: pi_spawner.clone(),
        binary: binary(),
    };

    let refused = setup
        .manager
        .start(start, deps, CollectingSink::new())
        .await
        .unwrap_err();
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);
    assert!(pi_spawner.requested().is_empty());
    assert!(setup.manager.claim_session(REPORTED_ID).is_none());

    setup.manager.stop_all().await;
}

#[tokio::test]
async fn a_resume_after_every_session_stopped_waits_for_a_start_in_flight() {
    let setup = setup("omp_slow_startup.jsonl");
    let stale = CollectingSink::new();

    // A page reload while the start is pending: every session stops, then the chat resumes.
    let reload = async {
        while setup.spawner.requested().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        setup.manager.begin_stop_all().finish().await;
        let resumed = setup.resume(REPORTED_ID, CollectingSink::new()).await;
        let stale_exited = matches!(stale.events().last(), Some(AgentEvent::Exited { .. }));

        (resumed, stale_exited)
    };
    let (stale_start, (resumed, stale_exited)) = tokio::join!(setup.resume(REPORTED_ID, stale.clone()), reload);

    assert_eq!(stale_start.unwrap_err().code(), IpcErrorCode::Process);
    resumed.unwrap();
    assert!(stale_exited, "the resume started before the stale agent exited");
    assert_eq!(setup.spawner.requested().len(), 2);
    assert_eq!(setup.manager.list().len(), 1);

    setup.manager.stop_all().await;
}
