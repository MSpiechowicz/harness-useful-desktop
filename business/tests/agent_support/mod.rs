//! Runs the real agent engine against `fake_agent` scenarios (`tests/fixtures/streams`).
//!
//! [`ScenarioSpawner`] records the command line the engine asked for and starts `fake_agent`
//! with the scenario instead, in the same working directory and with the same extra environment
//! (`OPENCODE_SERVER_PASSWORD` for `--dialect opencode`), and counts the agents it saw exit.
//! Each test crate uses a different
//! subset, hence the `dead_code` allowance.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use harness_business::agent::{AgentDeps, AgentManager, CollectingSink, SessionOptions};
use harness_business::env::Env;
use harness_business::harness::{Harness, HarnessRegistry};
use harness_business::model::{AgentEvent, AgentSessionInfo, AgentStartRequest, HarnessId};
use harness_business::process::{
    BoxFuture, ChildHandle, ExecSpec, ExitInfo, ProcessSpawner, ResolvedBinary, SpawnedProcess, TokioSpawner,
};
use tempfile::TempDir;

/// How long tests wait for an event.
pub const WAIT: Duration = Duration::from_secs(10);

/// `tests/fixtures/streams/<name>`.
pub fn scenario(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/streams")
        .join(name)
}

/// Starts `fake_agent --dialect <dialect> --scenario <scenario>` for every spawn request.
pub struct ScenarioSpawner {
    dialect: &'static str,
    scenario: PathBuf,
    requested: Mutex<Vec<ExecSpec>>,
    exited: Arc<AtomicUsize>,
}

impl ScenarioSpawner {
    pub fn new(dialect: &'static str, scenario_name: &str) -> Arc<Self> {
        Arc::new(Self {
            dialect,
            scenario: scenario(scenario_name),
            requested: Mutex::new(Vec::new()),
            exited: Arc::default(),
        })
    }

    /// The specs the engine asked to spawn.
    pub fn requested(&self) -> Vec<ExecSpec> {
        self.requested.lock().unwrap().clone()
    }

    /// How many of the spawned agents were reaped.
    pub fn exited(&self) -> usize {
        self.exited.load(Ordering::SeqCst)
    }
}

impl ProcessSpawner for ScenarioSpawner {
    fn spawn(&self, spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
        self.requested.lock().unwrap().push(spec.clone());

        let mut fake = ExecSpec::new(env!("CARGO_BIN_EXE_fake_agent")).args([
            "--dialect".into(),
            self.dialect.to_owned(),
            "--scenario".into(),
            self.scenario.to_string_lossy().into_owned(),
        ]);
        fake.cwd = spec.cwd.clone();
        fake.env = spec.env.clone();

        let mut process = TokioSpawner.spawn(&fake)?;
        process.child = Box::new(CountedChild {
            inner: process.child,
            exited: self.exited.clone(),
        });

        Ok(process)
    }
}

/// Counts the exit of its child once `wait` returned.
struct CountedChild {
    inner: Box<dyn ChildHandle>,
    exited: Arc<AtomicUsize>,
}

impl ChildHandle for CountedChild {
    fn id(&self) -> Option<u32> {
        self.inner.id()
    }

    fn wait(&mut self) -> BoxFuture<'_, harness_business::Result<ExitInfo>> {
        Box::pin(async move {
            let result = self.inner.wait().await;
            self.exited.fetch_add(1, Ordering::SeqCst);
            result
        })
    }

    fn start_kill(&mut self) -> harness_business::Result<()> {
        self.inner.start_kill()
    }

    fn terminate(&mut self) {
        self.inner.terminate();
    }
}

/// A temporary project with a `src/` directory (touched-path containment needs it).
pub fn project() -> TempDir {
    let dir = tempfile::tempdir().expect("temp project");
    std::fs::create_dir_all(dir.path().join("src")).expect("create src");
    dir
}

/// Options with deterministic keys (`key-1`, `key-2`, …), a fixed clock and no delta batching.
pub fn options() -> SessionOptions {
    let counter = Arc::new(AtomicU64::new(0));

    SessionOptions {
        coalesce_window: Duration::ZERO,
        key: Arc::new(move || format!("key-{}", counter.fetch_add(1, Ordering::Relaxed) + 1)),
        clock: Arc::new(|| "2026-10-09T10:00:00Z".to_owned()),
        ..SessionOptions::default()
    }
}

/// A started session and everything needed to drive and inspect it.
pub struct Run {
    pub manager: AgentManager,
    pub sink: Arc<CollectingSink>,
    pub spawner: Arc<ScenarioSpawner>,
    pub info: AgentSessionInfo,
    pub project: TempDir,
    pub home: TempDir,
}

impl Run {
    pub fn key(&self) -> &str {
        &self.info.key
    }

    /// True while the manager lists the session as running.
    pub fn is_listed(&self) -> bool {
        self.manager.list().iter().any(|info| info.key == self.info.key)
    }

    pub fn events(&self) -> Vec<AgentEvent> {
        self.sink.events()
    }

    /// Waits until `condition` holds for the events so far; panics after [`WAIT`].
    pub async fn wait_until(&self, what: &str, condition: impl Fn(&[AgentEvent]) -> bool) -> Vec<AgentEvent> {
        let deadline = tokio::time::Instant::now() + WAIT;

        loop {
            let events = self.events();
            if condition(&events) {
                return events;
            }

            assert!(tokio::time::Instant::now() < deadline, "never {what}; got {events:#?}");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// The startup catalog (the third event), once delivered.
    pub async fn catalog(&self) -> Vec<harness_business::model::CommandSpec> {
        let event = self
            .wait("catalog", |event| matches!(event, AgentEvent::CommandsUpdated { .. }))
            .await;

        match event {
            AgentEvent::CommandsUpdated { commands } => commands,
            _ => unreachable!(),
        }
    }

    /// Waits for an event matching `predicate`; panics after [`WAIT`].
    pub async fn wait(&self, what: &str, predicate: impl Fn(&AgentEvent) -> bool) -> AgentEvent {
        wait_event(&self.sink, what, predicate).await
    }
}

/// Waits for an event of `sink` matching `predicate`; panics after [`WAIT`].
pub async fn wait_event(sink: &CollectingSink, what: &str, predicate: impl Fn(&AgentEvent) -> bool) -> AgentEvent {
    sink.wait_for(WAIT, predicate)
        .await
        .unwrap_or_else(|| panic!("no {what} event; got {:#?}", sink.events()))
}

/// The start request for `harness` in `project`.
pub fn request(harness: HarnessId, project: &Path) -> AgentStartRequest {
    AgentStartRequest {
        harness,
        project: project.to_string_lossy().into_owned(),
        resume: None,
        approval_override: None,
        trust_once: false,
        utility: false,
    }
}

/// A binary path for the launch spec (the spawner replaces it).
pub fn binary() -> ResolvedBinary {
    ResolvedBinary {
        path: PathBuf::from(env!("CARGO_BIN_EXE_fake_agent")),
        is_batch_script: false,
    }
}

/// Starts `scenario_name` as `harness` (fake dialect `dialect`) with `options`.
pub async fn start_with(
    harness: HarnessId,
    dialect: &'static str,
    scenario_name: &str,
    options: SessionOptions,
    customize: impl FnOnce(&mut AgentStartRequest),
) -> harness_business::Result<Run> {
    let registry = HarnessRegistry::with_defaults();
    let adapter: &dyn Harness = registry.get(harness).expect("registered harness");
    let project = project();
    let home = tempfile::tempdir().expect("temp home");
    let env = Env::for_tests(home.path());

    let manager = AgentManager::with_options(options);
    let sink = CollectingSink::new();
    let spawner = ScenarioSpawner::new(dialect, scenario_name);

    let mut start = request(harness, project.path());
    customize(&mut start);

    let deps = AgentDeps {
        harness: adapter,
        env: &env,
        spawner: spawner.clone(),
        binary: binary(),
    };
    let info = manager.start(start, deps, sink.clone()).await?;

    Ok(Run {
        manager,
        sink,
        spawner,
        info,
        project,
        home,
    })
}

/// Starts an OMP scenario with the default test options.
pub async fn start_omp(scenario_name: &str) -> Run {
    start_with(HarnessId::Omp, "omp", scenario_name, options(), |_| {})
        .await
        .expect("OMP session starts")
}

/// Starts a PI scenario with the default test options.
pub async fn start_pi(scenario_name: &str) -> Run {
    start_with(HarnessId::Pi, "pi", scenario_name, options(), |_| {})
        .await
        .expect("PI session starts")
}

/// The `type` tag of every event, for order assertions.
pub fn kinds(events: &[AgentEvent]) -> Vec<String> {
    events
        .iter()
        .map(|event| {
            serde_json::to_value(event).expect("events serialize")["type"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}
