//! The embedder supervisor against the stand-in of `fake_agent` (see
//! `src/bin/fake_agent/memory_embedder.rs`), spawned through the tracked spawner like the app
//! does: the events reach the state a page watches, a crash is started again with a backoff and
//! given up on after three restarts, `busy` hands it to another instance for good, stopping
//! closes its stdin and leaves no process behind, and a server that does not list `embedder`
//! starts nothing.
#![cfg(unix)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use harness_business::ecosystem::{EmbedderSupervisor, MemoryFeatures, SupervisorOptions, embedder_spec};
use harness_business::model::{EmbedderPhase, MemoryEmbedderState};
use harness_business::process::{
    ChildRegistry, ExecSpec, ProcessIdentity, ProcessSpawner, SpawnedProcess, TrackedSpawner,
};
use memory_stand_in::{Setup, Vars};
use pretty_assertions::assert_eq;
use tokio::sync::watch::Receiver;

use crate::memory_stand_in;

const EMBEDDER: &[&str] = &["embedder", "run"];

/// Starts the stand-in through a [`TrackedSpawner`] and remembers each pid.
struct Spawner {
    inner: TrackedSpawner,
    pids: Mutex<Vec<u32>>,
}

impl ProcessSpawner for Spawner {
    fn spawn(&self, spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
        let process = self.inner.spawn(spec)?;
        self.pids.lock().unwrap().push(process.child.id().expect("a pid"));

        Ok(process)
    }
}

struct Embedder {
    setup: Setup,
    registry_dir: tempfile::TempDir,
    registry: Arc<ChildRegistry>,
    spawner: Arc<Spawner>,
    supervisor: EmbedderSupervisor,
}

/// A supervisor over a stand-in server that plays `script`, restarting after `backoff`.
fn embedder(vars: Vars, backoff: &[u64]) -> Embedder {
    let setup = Setup::installed(EMBEDDER, vars);
    let registry_dir = tempfile::tempdir().unwrap();
    let app = ProcessIdentity::current().expect("this process has an identity");
    let registry = ChildRegistry::with_owner(registry_dir.path(), app);
    let spawner = Arc::new(Spawner {
        inner: TrackedSpawner::new(setup.stand_in.clone(), registry.clone()),
        pids: Mutex::new(Vec::new()),
    });

    let options = SupervisorOptions {
        backoff: backoff.iter().map(|ms| Duration::from_millis(*ms)).collect(),
        window: Duration::from_secs(600),
        stop_grace: Duration::from_millis(300),
    };

    Embedder {
        supervisor: EmbedderSupervisor::with_options(spawner.clone(), options),
        setup,
        registry_dir,
        registry,
        spawner,
    }
}

impl Embedder {
    fn spec(&self) -> ExecSpec {
        embedder_spec(&self.setup.env, &self.setup.binary)
    }

    /// How many times the embedder was started.
    fn starts(&self) -> usize {
        self.spawner.pids.lock().unwrap().len()
    }

    fn pids(&self) -> Vec<u32> {
        self.spawner.pids.lock().unwrap().clone()
    }
}

/// Waits until the watched state satisfies `done`; panics after 10 s.
async fn until(
    updates: &mut Receiver<MemoryEmbedderState>,
    done: impl Fn(&MemoryEmbedderState) -> bool,
) -> MemoryEmbedderState {
    let waited = tokio::time::timeout(Duration::from_secs(10), updates.wait_for(|state| done(state))).await;

    waited
        .expect("the state in time")
        .expect("the supervisor lives")
        .clone()
}

/// Whether process `pid` still exists (a zombie counts as gone: the spawner reaps its children).
fn alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

async fn dead(pid: u32) -> bool {
    for _ in 0..100 {
        if !alive(pid) {
            return true;
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    false
}

const FULL_START: &str = concat!(
    "emit {\"event\":\"download\",\"received\":1,\"total\":2}\n",
    "emit {\"event\":\"verifying\"}\n",
    "emit {\"event\":\"loading\"}\n",
    "emit {\"event\":\"ready\",\"model\":\"m1\",\"dim\":384}\n",
    "emit {\"event\":\"backfill\",\"store\":\"local:1\",\"done\":1,\"total\":4}\n",
);

#[tokio::test]
async fn the_events_reach_the_watcher_and_stopping_closes_stdin_and_leaves_no_process() {
    let embedder = embedder(&[("FAKE_EMBEDDER_SCRIPT", FULL_START)], &[10]);
    let mut updates = embedder.supervisor.subscribe();
    assert_eq!(*updates.borrow(), MemoryEmbedderState::default());

    assert!(embedder.supervisor.start(embedder.spec()).await);
    assert!(!embedder.supervisor.start(embedder.spec()).await, "already running");

    let backfilling = until(&mut updates, |state| state.phase == EmbedderPhase::Backfilling).await;
    assert_eq!(backfilling.progress, Some(0.25));
    assert_eq!(backfilling.model.as_deref(), Some("m1"));
    assert_eq!(backfilling.error, None);

    assert_eq!(embedder.starts(), 1);
    let requested = embedder.setup.requested();
    assert_eq!(requested.len(), 1);
    assert_eq!(requested[0].args, EMBEDDER);
    assert_eq!(requested[0].cwd.as_deref(), Some(embedder.setup.env.home.as_path()));
    assert!(
        embedder.registry.file().exists(),
        "the embedder is recorded as a child of the app"
    );

    let pid = embedder.pids()[0];
    assert!(alive(pid));

    embedder.supervisor.stop().await;

    assert_eq!(
        embedder.supervisor.state(),
        MemoryEmbedderState {
            model_present: Some(true),
            ..MemoryEmbedderState::default()
        },
        "the model it served is downloaded"
    );
    assert!(dead(pid).await, "stdin closed, the embedder exited");
    assert!(!embedder.registry.file().exists(), "nothing left to sweep");
    assert_eq!(embedder.starts(), 1, "stopping does not restart it");
    drop(embedder.registry_dir);
}

#[tokio::test]
async fn a_crash_starts_it_again_and_the_fourth_crash_in_a_row_fails_it_for_good() {
    let embedder = embedder(
        &[("FAKE_EMBEDDER_SCRIPT", "emit {\"event\":\"loading\"}\nexit 1")],
        &[10, 10, 10],
    );
    let mut updates = embedder.supervisor.subscribe();

    embedder.supervisor.start(embedder.spec()).await;
    let failed = until(&mut updates, |state| state.phase == EmbedderPhase::Failed).await;

    assert_eq!(embedder.starts(), 4, "the first start and three restarts");
    assert!(failed.error.is_some_and(|error| !error.is_empty()));

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(embedder.starts(), 4, "no fifth start");
    assert!(!embedder.registry.file().exists(), "every dead embedder was reaped");

    embedder.supervisor.stop().await;
    assert_eq!(embedder.supervisor.state(), MemoryEmbedderState::default());
}

#[tokio::test]
async fn a_crash_followed_by_a_good_start_is_ready_again() {
    let embedder = embedder(
        &[(
            "FAKE_EMBEDDER_SCRIPT",
            "exit-first 1\nemit {\"event\":\"ready\",\"model\":\"m1\",\"dim\":384}",
        )],
        &[10, 10, 10],
    );
    let mut updates = embedder.supervisor.subscribe();

    embedder.supervisor.start(embedder.spec()).await;
    let ready = until(&mut updates, |state| state.phase == EmbedderPhase::Ready).await;

    assert_eq!(ready.model.as_deref(), Some("m1"));
    assert_eq!(embedder.starts(), 2, "one crash, one restart");

    embedder.supervisor.stop().await;
}

#[tokio::test]
async fn busy_means_another_instance_runs_it_and_is_not_started_again() {
    let embedder = embedder(
        &[("FAKE_EMBEDDER_SCRIPT", "emit {\"event\":\"busy\"}\nexit 0")],
        &[10, 10, 10],
    );
    let mut updates = embedder.supervisor.subscribe();

    embedder.supervisor.start(embedder.spec()).await;
    until(&mut updates, |state| state.phase == EmbedderPhase::Elsewhere).await;

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(embedder.starts(), 1, "no retry loop");
    assert_eq!(embedder.supervisor.state().phase, EmbedderPhase::Elsewhere);

    assert!(
        embedder.supervisor.start(embedder.spec()).await,
        "an explicit start tries again"
    );
    for _ in 0..200 {
        if embedder.starts() == 2 {
            break;
        }

        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(embedder.starts(), 2);

    embedder.supervisor.stop().await;
}

#[tokio::test]
async fn an_embedder_that_ignores_stdin_is_killed_after_the_grace() {
    let embedder = embedder(&[("FAKE_EMBEDDER_SCRIPT", "emit {\"event\":\"loading\"}\nhang")], &[10]);
    let mut updates = embedder.supervisor.subscribe();

    embedder.supervisor.start(embedder.spec()).await;
    until(&mut updates, |state| state.phase == EmbedderPhase::Loading).await;
    let pid = embedder.pids()[0];

    embedder.supervisor.stop().await;

    assert!(dead(pid).await);
    assert!(!embedder.registry.file().exists());
    assert_eq!(embedder.supervisor.state().phase, EmbedderPhase::Off);
}

fn only_embedder_runs(setup: &Setup) -> Vec<Vec<String>> {
    (0..setup.runs().len()).map(|run| setup.run_args(run)).collect()
}

#[tokio::test]
async fn an_older_server_starts_no_embedder_and_a_disabled_one_is_enabled_once_before_it_starts() {
    let older = embedder(&[("FAKE_MEMORY_FEATURES", "notes-hide")], &[10]);
    let features = MemoryFeatures::new();

    older
        .supervisor
        .reconcile(&older.setup.env, older.setup.runner(), &features, true, &|| false)
        .await;

    assert_eq!(older.starts(), 0, "nothing spawned");
    assert_eq!(
        only_embedder_runs(&older.setup),
        [vec!["features"]],
        "not even the status was asked"
    );
    assert_eq!(older.supervisor.state(), MemoryEmbedderState::default());

    let current = embedder(
        &[("FAKE_EMBEDDER_ENABLED", "false"), ("FAKE_EMBEDDER_MODEL", "present")],
        &[10],
    );
    let mut updates = current.supervisor.subscribe();
    let features = MemoryFeatures::new();

    current
        .supervisor
        .reconcile(&current.setup.env, current.setup.runner(), &features, true, &|| false)
        .await;
    until(&mut updates, |state| state.phase == EmbedderPhase::Ready).await;

    assert_eq!(
        only_embedder_runs(&current.setup)[..3],
        [
            vec!["features"],
            vec!["embedder", "status", "--json"],
            vec!["embedder", "enable"]
        ]
    );
    assert_eq!(current.starts(), 1);

    current
        .supervisor
        .reconcile(&current.setup.env, current.setup.runner(), &features, false, &|| false)
        .await;
    assert_eq!(
        current.supervisor.state(),
        MemoryEmbedderState {
            model_present: Some(true),
            ..MemoryEmbedderState::default()
        }
    );
    assert!(dead(current.pids()[0]).await);
}

#[tokio::test]
async fn a_stopped_embedder_learns_from_the_status_whether_a_model_is_downloaded() {
    let answers: [(Vars, Option<bool>); 3] = [
        (&[("FAKE_EMBEDDER_MODEL", "present")], Some(true)),
        (&[("FAKE_EMBEDDER_MODEL", "absent")], Some(false)),
        (&[("FAKE_EMBEDDER_MODEL", "unknown")], None),
    ];

    for (vars, expected) in answers {
        let stopped = embedder(vars, &[10]);

        stopped
            .supervisor
            .reconcile(
                &stopped.setup.env,
                stopped.setup.runner(),
                &MemoryFeatures::new(),
                false,
                &|| false,
            )
            .await;

        assert_eq!(stopped.supervisor.state().model_present, expected, "{vars:?}");
        assert_eq!(stopped.supervisor.state().phase, EmbedderPhase::Off);
        assert_eq!(stopped.starts(), 0, "nothing runs");
    }

    let launching = embedder(
        &[("FAKE_EMBEDDER_ENABLED", "true"), ("FAKE_EMBEDDER_MODEL", "present")],
        &[10],
    );
    let mut updates = launching.supervisor.subscribe();

    launching
        .supervisor
        .reconcile(
            &launching.setup.env,
            launching.setup.runner(),
            &MemoryFeatures::new(),
            true,
            &|| false,
        )
        .await;
    until(&mut updates, |state| state.phase == EmbedderPhase::Ready).await;
    assert_eq!(launching.supervisor.state().model_present, Some(true));

    launching.supervisor.set_model_present(Some(false));
    assert_eq!(launching.supervisor.state().model_present, Some(false));
    launching.supervisor.stop().await;
}

#[tokio::test]
async fn an_enabled_embedder_is_not_enabled_again_and_a_missing_server_runs_nothing() {
    let enabled = embedder(&[("FAKE_EMBEDDER_ENABLED", "true")], &[10]);
    let features = MemoryFeatures::new();
    let mut updates = enabled.supervisor.subscribe();

    enabled
        .supervisor
        .reconcile(&enabled.setup.env, enabled.setup.runner(), &features, true, &|| false)
        .await;
    until(&mut updates, |state| state.phase == EmbedderPhase::Ready).await;

    let commands = only_embedder_runs(&enabled.setup);
    assert!(
        !commands.contains(&vec!["embedder".to_owned(), "enable".to_owned()]),
        "{commands:?}"
    );
    enabled.supervisor.stop().await;

    let absent = Setup::without_server(EMBEDDER, &[]);
    let supervisor = EmbedderSupervisor::new(absent.stand_in.clone());
    supervisor
        .reconcile(&absent.env, absent.runner(), &MemoryFeatures::new(), true, &|| false)
        .await;
    assert!(absent.requested().is_empty());
    assert_eq!(supervisor.state(), MemoryEmbedderState::default());
}

mod reconcile_cases;
