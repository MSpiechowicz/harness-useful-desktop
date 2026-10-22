//! Starts the Claude Code chat engine directly ([`AgentSession::start`] with
//! [`ChatProtocol::ClaudeStream`]) against the scripted `claude` (`fake_agent --dialect claude`,
//! see `src/bin/fake_agent/claude.rs` for the scenario format). Scenarios replay the recordings in
//! `tests/fixtures/claude/stream`. The project folder is also the session folder; [`Setup`] puts
//! saved sessions there and plays one scenario per process of the chat.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use harness_business::agent::{AgentSession, CollectingSink, SessionLaunch, SessionOptions};
use harness_business::harness::ChatProtocol;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ApprovalMode, SessionState};
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
    Setup {
        resume,
        options,
        ..Setup::new(&[name])
    }
    .start()
    .await
}

/// Starts scenario `name` with the approval override `approval`.
pub async fn start_in(name: &str, approval: Option<ApprovalMode>) -> Chat {
    Setup {
        approval,
        ..Setup::new(&[name])
    }
    .start()
    .await
}

/// A start of the chat engine.
pub struct Setup<'a> {
    /// The scenario of each process in turn.
    pub scenarios: &'a [&'a str],
    pub resume: Option<&'a str>,
    pub approval: Option<ApprovalMode>,
    pub utility: bool,
    /// Session files of `tests/fixtures/sessions/claude` copied into the session folder.
    pub sessions: &'a [&'a str],
    pub options: SessionOptions,
}

impl<'a> Setup<'a> {
    pub fn new(scenarios: &'a [&'a str]) -> Self {
        Self {
            scenarios,
            resume: None,
            approval: None,
            utility: false,
            sessions: &[],
            options: options(),
        }
    }

    pub async fn start(self) -> Chat {
        let dir = project();
        let project = dunce::canonicalize(dir.path()).expect("canonical project");
        let sink = CollectingSink::new();
        let spawner = ScenarioSpawner::sequence("claude", self.scenarios);

        for id in self.sessions {
            let file = format!("{id}.jsonl");
            std::fs::copy(saved_session(id), project.join(file)).expect("copy the session file");
        }

        let launch = SessionLaunch {
            key: "key-1".to_owned(),
            spec: launch_spec(&project),
            spawner: spawner.clone(),
            protocol: ChatProtocol::ClaudeStream,
            project: project.clone(),
            sessions_dir: project.clone(),
            resume: self.resume,
            approval_override: self.approval,
            utility: self.utility,
            sink: sink.clone(),
            options: &self.options,
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
}

/// The synthetic session file `id` (`tests/fixtures/sessions/claude`).
pub fn saved_session(id: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sessions/claude")
        .join(format!("{id}.jsonl"))
}

/// The arguments of spawn `index` (from 0).
pub fn argv(chat: &Chat, index: usize) -> Vec<String> {
    let specs = chat.spawner.requested();
    let spec = specs.get(index).expect("that many spawns");

    spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

/// The value after `flag` in `args`.
pub fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let at = args.iter().position(|arg| arg == name)?;

    args.get(at + 1).map(String::as_str)
}

/// Waits for the `Marker` title the scenarios set; every event so far.
pub async fn marker(chat: &Chat) -> Vec<AgentEvent> {
    chat.wait(
        "marker",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Marker")),
    )
    .await;

    chat.events()
}

/// Stops the chat; the scripted CLI exits 0 only when every expected frame matched.
pub async fn stop_cleanly(chat: &Chat) {
    let events = chat.stop().await;

    assert_eq!(
        events.last(),
        Some(&AgentEvent::Exited {
            code: Some(0),
            signal: None
        }),
        "the scripted CLI saw what it expected"
    );
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

/// The chat's saved messages (`GetHistory`, one page).
pub async fn history(chat: &Chat) -> Vec<harness_business::model::ChatMessage> {
    let reply = chat
        .send(AgentAction::GetHistory {
            cursor: None,
            limit: 200,
        })
        .await;

    match reply {
        AgentReply::History { messages, .. } => messages,
        other => panic!("not a history: {other:?}"),
    }
}
