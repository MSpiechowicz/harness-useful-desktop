//! One live agent per session id: resumes, switches and claims of an id a running, starting or
//! stopping agent of any harness holds, and starts in flight when every session stops.

mod agent_support;

use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use agent_support::{ScenarioSpawner, WAIT, binary, options, project, request, start_with};
use harness_business::agent::{AgentDeps, AgentManager, CollectingSink, Generator, SessionOptions};
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
    manager: Arc<AgentManager>,
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
        manager: Arc::new(AgentManager::with_options(options)),
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
        self.resume_with(id, &self.spawner, sink).await
    }

    /// Starts an agent of `spawner` resuming OMP session `id`.
    async fn resume_with(
        &self,
        id: &str,
        spawner: &Arc<ScenarioSpawner>,
        sink: Arc<CollectingSink>,
    ) -> harness_business::Result<AgentSessionInfo> {
        let start = AgentStartRequest {
            resume: Some(SessionRef {
                harness: HarnessId::Omp,
                id: id.to_owned(),
            }),
            ..request(HarnessId::Omp, self.project.path())
        };

        self.manager.start(start, omp_deps(&self.env, spawner), sink).await
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
    assert_eq!(setup.spawner.exited(), 1, "the agent was stopped");
    assert!(sink.events().is_empty(), "a start given up emits nothing");
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
        let stale_exited = setup.spawner.exited() == 1;

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

#[tokio::test]
async fn a_start_whose_startup_completed_before_every_session_stopped_stops_its_agent() {
    // The `startedAt` clock runs after startup completed and before the agent is registered, so
    // stopping every session from it (once) lands exactly in that gap.
    let mut setup = setup("omp_out_of_order.jsonl");
    setup.manager = Arc::new_cyclic(|manager: &Weak<AgentManager>| {
        let target = Mutex::new(Some(manager.clone()));
        let clock: Generator = Arc::new(move || {
            if let Some(manager) = target.lock().unwrap().take().and_then(|weak| weak.upgrade()) {
                drop(manager.begin_stop_all());
            }

            "2026-10-14T00:00:00Z".to_owned()
        });

        AgentManager::with_options(SessionOptions { clock, ..options() })
    });

    let sink = CollectingSink::new();
    let stale = setup.resume(REPORTED_ID, sink.clone()).await.unwrap_err();

    assert_eq!(stale.code(), IpcErrorCode::Process);
    assert!(matches!(sink.events().last(), Some(AgentEvent::Exited { .. })));
    assert_eq!(setup.spawner.exited(), 1, "the agent was stopped");
    assert!(setup.manager.list().is_empty());
    assert!(!setup.manager.holds_session(REPORTED_ID));

    setup.resume(REPORTED_ID, CollectingSink::new()).await.unwrap();
    assert!(setup.manager.holds_session(REPORTED_ID));

    setup.manager.stop_all().await;
}

/// Startup deadline of the hung-startup test.
const HUNG_DEADLINE: Duration = Duration::from_millis(500);

#[tokio::test]
async fn a_start_hung_behind_a_startup_dialog_fails_at_its_deadline_and_frees_the_id() {
    let setup = setup_with(
        "omp_startup_dialog_hang.jsonl",
        SessionOptions {
            startup_deadline: HUNG_DEADLINE,
            ..options()
        },
    );

    let started = Instant::now();
    let hung = tokio::time::timeout(WAIT, setup.resume(REPORTED_ID, CollectingSink::new()));
    let error = hung.await.expect("the start gives up").unwrap_err();

    assert_eq!(error.code(), IpcErrorCode::Timeout);
    assert!(started.elapsed() < HUNG_DEADLINE * 4, "{:?}", started.elapsed());
    assert_eq!(setup.spawner.exited(), 1, "the hung agent was killed");
    assert!(!setup.manager.holds_session(REPORTED_ID));

    let working = ScenarioSpawner::new("omp", "omp_out_of_order.jsonl");
    setup
        .resume_with(REPORTED_ID, &working, CollectingSink::new())
        .await
        .unwrap();
    assert!(setup.manager.holds_session(REPORTED_ID));

    setup.manager.stop_all().await;
}

#[tokio::test]
async fn stopping_every_session_aborts_a_hung_start_at_once() {
    // The production deadline: only the stop can end the start quickly.
    let setup = setup("omp_startup_dialog_hang.jsonl");

    let stop_all = async {
        while setup.spawner.requested().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
        setup.manager.stop_all().await;
    };
    let started = Instant::now();
    let hung = tokio::time::timeout(WAIT, setup.resume(REPORTED_ID, CollectingSink::new()));
    let (result, ()) = tokio::join!(hung, stop_all);

    assert_eq!(
        result.expect("the start gives up").unwrap_err().code(),
        IpcErrorCode::Process
    );
    assert!(started.elapsed() < Duration::from_secs(3), "{:?}", started.elapsed());
    assert_eq!(setup.spawner.exited(), 1, "the hung agent was killed");
    assert!(!setup.manager.holds_session(REPORTED_ID));
}
