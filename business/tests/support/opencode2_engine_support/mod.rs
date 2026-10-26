//! Starts the OpenCode 2 chat engine directly ([`AgentSession::start`] with
//! [`ChatProtocol::OpenCodeServerV2`]) against the scripted OpenCode 2 server (`fake_agent
//! --dialect opencode2`, see `src/bin/fake_agent/opencode2.rs` for the scenario format).
//!
//! [`FakeServerSpawner`] starts the fake for every spawn with the engine's own arguments after the
//! scenario (so the fake checks `serve --hostname 127.0.0.1 --port 0` and refuses `--mdns`), the
//! engine's working directory and environment, and a request log the tests read back with
//! [`Chat::requests`].
#![allow(dead_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use harness_business::agent::{AgentSession, CollectingSink, SessionLaunch, SessionOptions, StartAbort};
use harness_business::harness::ChatProtocol;
use harness_business::model::{AgentAction, AgentEvent, ApprovalMode};
use harness_business::process::{
    BoxFuture, ChildHandle, ExecSpec, ExitInfo, ProcessSpawner, SpawnedProcess, TokioSpawner,
};
use serde_json::Value;
use tempfile::TempDir;

use crate::agent_support::{WAIT, options, project, scenario, wait_event};

/// The session the startup scenario creates (`recordings/turn.jsonl`).
pub const SESSION: &str = "ses_ea2843d6fffeeHJKZwBKUXqULv";

/// The arguments the engine starts `opencode serve` 2.x with.
pub const SERVE_ARGS: [&str; 5] = ["serve", "--hostname", "127.0.0.1", "--port", "0"];

/// Starts `fake_agent --dialect opencode2 --scenario <scenario> <engine arguments…>` for every
/// spawn request, logging the requests the fake receives to [`FakeServerSpawner::log`].
pub struct FakeServerSpawner {
    scenario: PathBuf,
    log: PathBuf,
    requested: Mutex<Vec<ExecSpec>>,
    exited: Arc<AtomicUsize>,
}

impl FakeServerSpawner {
    /// A spawner for `tests/fixtures/streams/<scenario_name>` logging to `log`.
    pub fn new(scenario_name: &str, log: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            scenario: scenario(scenario_name),
            log,
            requested: Mutex::new(Vec::new()),
            exited: Arc::default(),
        })
    }

    /// The specs the engine asked to spawn.
    pub fn requested(&self) -> Vec<ExecSpec> {
        self.requested.lock().unwrap().clone()
    }

    /// How many of the spawned servers were reaped.
    pub fn exited(&self) -> usize {
        self.exited.load(Ordering::SeqCst)
    }

    /// The request log file (one JSON object per line).
    pub fn log(&self) -> &Path {
        &self.log
    }

    /// Every request the spawned servers received so far.
    pub fn requests(&self) -> Vec<LoggedRequest> {
        read_log(&self.log)
    }
}

impl ProcessSpawner for FakeServerSpawner {
    fn spawn(&self, spec: &ExecSpec) -> harness_business::Result<SpawnedProcess> {
        self.requested.lock().unwrap().push(spec.clone());

        let mut args: Vec<OsString> = vec![
            "--dialect".into(),
            "opencode2".into(),
            "--scenario".into(),
            self.scenario.clone().into_os_string(),
        ];
        args.extend(spec.args.iter().cloned());

        let mut fake = ExecSpec::new(env!("CARGO_BIN_EXE_fake_agent")).args(args);
        fake.cwd = spec.cwd.clone();
        fake.env = spec.env.clone();
        fake.env_remove_prefixes = spec.env_remove_prefixes.clone();
        fake.env
            .push(("FAKE_OPENCODE_REQUEST_LOG".into(), self.log.clone().into_os_string()));

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

/// One request the fake received.
#[derive(Debug, Clone, PartialEq)]
pub struct LoggedRequest {
    pub method: String,
    /// Path and raw query, as sent.
    pub target: String,
    /// The JSON body (`null` when empty, a string when it is not JSON).
    pub body: Value,
    /// Whether it carried the server password.
    pub authorized: bool,
}

impl LoggedRequest {
    /// The path without the query.
    pub fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }

    /// Whether this is `route`: `METHOD /path` with `*` for one path segment.
    pub fn is(&self, route: &str) -> bool {
        let Some((method, pattern)) = route.split_once(' ') else {
            return false;
        };

        let wanted: Vec<&str> = pattern.split('/').collect();
        let actual: Vec<&str> = self.path().split('/').collect();

        method == self.method
            && wanted.len() == actual.len()
            && wanted
                .iter()
                .zip(&actual)
                .all(|(wanted, actual)| *wanted == "*" || wanted == actual)
    }
}

/// The requests in the log at `path` (none when it does not exist yet).
pub fn read_log(path: &Path) -> Vec<LoggedRequest> {
    let text = std::fs::read_to_string(path).unwrap_or_default();

    text.lines()
        .map(|line| {
            let entry: Value = serde_json::from_str(line).expect("a JSON log line");

            LoggedRequest {
                method: entry["method"].as_str().unwrap_or_default().to_owned(),
                target: entry["target"].as_str().unwrap_or_default().to_owned(),
                body: entry["body"].clone(),
                authorized: entry["authorized"].as_bool().unwrap_or(false),
            }
        })
        .collect()
}

/// A started (or refused) chat and everything to inspect it.
pub struct Chat {
    pub session: harness_business::Result<AgentSession>,
    pub sink: Arc<CollectingSink>,
    pub spawner: Arc<FakeServerSpawner>,
    /// The canonical project directory.
    pub project: PathBuf,
    _dir: TempDir,
    _log_dir: TempDir,
}

impl Chat {
    /// The running session; panics when startup failed.
    pub fn session(&self) -> &AgentSession {
        self.session.as_ref().expect("the OpenCode 2 chat starts")
    }

    pub fn events(&self) -> Vec<AgentEvent> {
        self.sink.events()
    }

    /// Waits for an event matching `predicate`; panics after [`WAIT`].
    pub async fn wait(&self, what: &str, predicate: impl Fn(&AgentEvent) -> bool) -> AgentEvent {
        wait_event(&self.sink, what, predicate).await
    }

    /// The server password the engine put in the spawned server's environment.
    pub fn password(&self) -> String {
        let spec = self.spawner.requested().pop().expect("a spawn");

        env(&spec, "OPENCODE_SERVER_PASSWORD").expect("a server password")
    }

    /// Every request the server received so far.
    pub fn requests(&self) -> Vec<LoggedRequest> {
        self.spawner.requests()
    }

    /// The requests to `route` (`METHOD /path`, `*` for one segment) so far.
    pub fn requests_to(&self, route: &str) -> Vec<LoggedRequest> {
        self.requests()
            .into_iter()
            .filter(|request| request.is(route))
            .collect()
    }

    /// Waits until the spawned server was reaped; panics after [`WAIT`].
    pub async fn wait_reaped(&self) {
        let deadline = tokio::time::Instant::now() + WAIT;

        while self.spawner.exited() == 0 {
            assert!(tokio::time::Instant::now() < deadline, "the server was never reaped");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

/// The value of `name` in `spec`'s extra environment.
pub fn env(spec: &ExecSpec, name: &str) -> Option<String> {
    spec.env
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.to_string_lossy().into_owned())
}

/// The launch spec of `opencode serve` 2.x in `project`, with `env` added.
pub fn serve_spec(project: &Path, env: &[(&str, &str)]) -> ExecSpec {
    let mut spec = ExecSpec::new("opencode").args(SERVE_ARGS);
    spec.cwd = Some(project.to_path_buf());
    spec.env
        .extend(env.iter().map(|(name, value)| ((*name).into(), (*value).into())));
    spec
}

/// Starts scenario `name` with the default test options.
pub async fn start(name: &str) -> Chat {
    start_with(name, None, options()).await
}

/// Starts scenario `name`, resuming `resume`, with `options`.
pub async fn start_with(name: &str, resume: Option<&str>, options: SessionOptions) -> Chat {
    let launch = Launch {
        name,
        resume,
        approval: None,
        utility: false,
        options,
        env: &[],
    };

    start_through(launch, |spawner| spawner, Box::pin(std::future::pending())).await
}

/// Starts scenario `name` with an approval override, as a helper chat when `utility`.
pub async fn start_mode(name: &str, approval: Option<ApprovalMode>, utility: bool) -> Chat {
    let launch = Launch {
        name,
        resume: None,
        approval,
        utility,
        options: options(),
        env: &[],
    };

    start_through(launch, |spawner| spawner, Box::pin(std::future::pending())).await
}

/// Everything a test chat starts with.
pub struct Launch<'a> {
    pub name: &'a str,
    pub resume: Option<&'a str>,
    pub approval: Option<ApprovalMode>,
    pub utility: bool,
    pub options: SessionOptions,
    /// Extra launch environment (an inherited `OPENCODE_CONFIG_CONTENT`, say).
    pub env: &'a [(&'a str, &'a str)],
}

/// Starts `launch` through the spawner `through` makes of the fake server spawner (which the chat
/// keeps for inspection), given up when `abort` resolves.
pub async fn start_through(
    launch: Launch<'_>,
    through: impl FnOnce(Arc<FakeServerSpawner>) -> Arc<dyn ProcessSpawner>,
    abort: StartAbort<'_>,
) -> Chat {
    let Launch {
        name,
        resume,
        approval,
        utility,
        options,
        env,
    } = launch;
    let dir = project();
    let project = dunce::canonicalize(dir.path()).expect("canonical project");
    let log_dir = tempfile::tempdir().expect("temp log dir");
    let sink = CollectingSink::new();
    let spawner = FakeServerSpawner::new(name, log_dir.path().join("requests.jsonl"));
    let used = through(spawner.clone());

    let launch = SessionLaunch {
        key: "key-1".to_owned(),
        spec: serve_spec(&project, env),
        spawner: used.clone(),
        protocol: ChatProtocol::OpenCodeServerV2,
        project: project.clone(),
        sessions_dir: project.clone(),
        config_dir: project.clone(),
        resume,
        approval_override: approval,
        trust_once: false,
        utility,
        memory_session: false,
        sink: sink.clone(),
        options: &options,
        abort,
    };

    let session = AgentSession::start(launch).await;

    Chat {
        session,
        sink,
        spawner,
        project,
        _dir: dir,
        _log_dir: log_dir,
    }
}

/// The exit of a server ended at stop: SIGTERM on Unix. Windows has no signals and
/// TerminateProcess leaves exit code 1.
pub fn terminated() -> AgentEvent {
    if cfg!(unix) {
        AgentEvent::Exited {
            code: None,
            signal: Some(15),
        }
    } else {
        AgentEvent::Exited {
            code: Some(1),
            signal: None,
        }
    }
}

/// The `type` tag of an event.
pub fn kind(event: &AgentEvent) -> String {
    serde_json::to_value(event).expect("events serialize")["type"]
        .as_str()
        .expect("type tag")
        .to_owned()
}

/// How many events of type `name` there are.
pub fn count(events: &[AgentEvent], name: &str) -> usize {
    events.iter().filter(|event| kind(event) == name).count()
}

/// A plain prompt.
pub fn text_prompt(message: &str) -> AgentAction {
    AgentAction::Prompt {
        message: message.to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    }
}

/// Waits for the `Marker` title the scenarios end with; every event so far.
pub async fn until_marker(chat: &Chat) -> Vec<AgentEvent> {
    chat.wait(
        "marker",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Marker")),
    )
    .await;

    chat.events()
}
