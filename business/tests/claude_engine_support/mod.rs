//! Starts the Claude Code chat engine directly ([`AgentSession::start`] with
//! [`ChatProtocol::ClaudeStream`]: the harness keeps chat off, so the manager would refuse it)
//! against the scripted `claude` (`fake_agent --dialect claude`, see
//! `src/bin/fake_agent/claude.rs` for the scenario format). Scenarios replay the recordings in
//! `tests/fixtures/claude/stream`.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use harness_business::agent::{AgentSession, CollectingSink, SessionLaunch, SessionOptions};
use harness_business::harness::ChatProtocol;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, SessionState};
use harness_business::process::ExecSpec;
use tempfile::TempDir;

use crate::agent_support::{ScenarioSpawner, options, project, wait_event};

/// The session of basic.jsonl (the first frame of `claude_startup.jsonl` names it).
pub const BASIC_SESSION: &str = "c1262da3-a66b-42b5-a9ee-32bb98f2045d";

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
        self.session.as_ref().expect("the Claude Code chat starts")
    }

    pub fn events(&self) -> Vec<AgentEvent> {
        self.sink.events()
    }

    /// Waits for an event matching `predicate`; panics after the test wait.
    pub async fn wait(&self, what: &str, predicate: impl Fn(&AgentEvent) -> bool) -> AgentEvent {
        wait_event(&self.sink, what, predicate).await
    }

    /// Runs `action`; panics when it fails.
    pub async fn send(&self, action: AgentAction) -> AgentReply {
        self.session()
            .send(action)
            .await
            .unwrap_or_else(|error| panic!("the action fails: {error}"))
    }

    /// Waits until the chat settled `count` times.
    pub async fn settled(&self, count: usize) -> Vec<AgentEvent> {
        let deadline = tokio::time::Instant::now() + crate::agent_support::WAIT;

        loop {
            let events = self.events();
            if events
                .iter()
                .filter(|event| matches!(event, AgentEvent::Settled { .. }))
                .count()
                >= count
            {
                return events;
            }

            assert!(
                tokio::time::Instant::now() < deadline,
                "never settled {count} times: {events:#?}"
            );
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    /// The newest `state` event.
    pub fn last_state(&self) -> SessionState {
        self.events()
            .into_iter()
            .rev()
            .find_map(|event| match event {
                AgentEvent::State { state } => Some(state),
                _ => None,
            })
            .expect("a state event")
    }

    /// Stops the chat and returns every event.
    pub async fn stop(&self) -> Vec<AgentEvent> {
        self.session().stop().await;
        self.events()
    }
}

/// The command line the harness gives a new chat (the spawner replaces the program).
pub fn launch_spec(project: &std::path::Path) -> ExecSpec {
    let mut spec = ExecSpec::new("claude").args([
        "--output-format",
        "stream-json",
        "--verbose",
        "--input-format",
        "stream-json",
        "--permission-prompt-tool",
        "stdio",
        "--include-partial-messages",
    ]);
    spec.cwd = Some(project.to_path_buf());
    spec
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
    let spawner = ScenarioSpawner::new("claude", name);

    let launch = SessionLaunch {
        key: "key-1".to_owned(),
        spec: launch_spec(&project),
        spawner: spawner.as_ref(),
        protocol: ChatProtocol::ClaudeStream,
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

/// The tags of `events`.
pub fn kinds(events: &[AgentEvent]) -> Vec<String> {
    events.iter().map(kind).collect()
}

/// A plain prompt.
pub fn prompt(message: &str) -> AgentAction {
    AgentAction::Prompt {
        message: message.to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    }
}
