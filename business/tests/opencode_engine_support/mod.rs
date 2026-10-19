//! Starts the OpenCode chat engine directly ([`AgentSession::start`]) against the scripted
//! `opencode serve` (`fake_agent --dialect opencode`): the manager does not offer OpenCode chat
//! yet.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use harness_business::agent::{AgentSession, CollectingSink, SessionLaunch, SessionOptions};
use harness_business::harness::ChatProtocol;
use harness_business::model::AgentEvent;
use harness_business::process::ExecSpec;
use tempfile::TempDir;

use crate::agent_support::{ScenarioSpawner, WAIT, options, project, wait_event};

/// The session of the recorded run (`fixtures/opencode/server`).
pub const SESSION: &str = "ses_eaa561636ffezINnnn1ghd5370";

/// A started (or refused) chat and everything to inspect it.
pub struct Chat {
    pub session: harness_business::Result<AgentSession>,
    pub sink: Arc<CollectingSink>,
    pub spawner: Arc<ScenarioSpawner>,
    /// The canonical project directory.
    pub project: PathBuf,
    _dir: TempDir,
}

impl Chat {
    /// The running session; panics when startup failed.
    pub fn session(&self) -> &AgentSession {
        self.session.as_ref().expect("the OpenCode chat starts")
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

/// Starts scenario `name` with the default test options.
pub async fn start(name: &str) -> Chat {
    start_with(name, None, options()).await
}

/// Starts scenario `name`, resuming `resume`, with `options`.
pub async fn start_with(name: &str, resume: Option<&str>, options: SessionOptions) -> Chat {
    let dir = project();
    let project = dunce::canonicalize(dir.path()).expect("canonical project");
    let sink = CollectingSink::new();
    let spawner = ScenarioSpawner::new("opencode", name);

    let mut spec = ExecSpec::new("opencode").args(["serve", "--hostname", "127.0.0.1", "--port", "0", "--mdns=false"]);
    spec.cwd = Some(project.clone());

    let launch = SessionLaunch {
        key: "key-1".to_owned(),
        spec,
        spawner: spawner.as_ref(),
        protocol: ChatProtocol::OpenCodeServer,
        project: project.clone(),
        sessions_dir: project.clone(),
        resume,
        approval_override: None,
        utility: false,
        sink: sink.clone(),
        options: &options,
        abort: Box::pin(std::future::pending()),
    };

    let session = AgentSession::start(launch).await;

    Chat {
        session,
        sink,
        spawner,
        project,
        _dir: dir,
    }
}

/// The `type` tag of an event.
pub fn kind(event: &AgentEvent) -> String {
    serde_json::to_value(event).expect("events serialize")["type"]
        .as_str()
        .expect("type tag")
        .to_owned()
}
