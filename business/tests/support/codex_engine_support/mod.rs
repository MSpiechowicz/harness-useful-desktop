//! Starts the Codex chat engine the way the manager will: [`Codex::launch_spec`] and
//! [`Codex::chat_protocol`] into [`AgentSession::start`], against the scripted `codex app-server`
//! (`fake_agent --dialect codex`, see `src/bin/fake_agent/codex.rs` for the scenario format).
//! Scenarios replay the recordings in `tests/fixtures/codex/app_server`; images go to a
//! temporary attachments folder, and the Codex home is [`HOME`] in the project.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use harness_business::agent::{AgentSession, CollectingSink, SessionLaunch};
use harness_business::harness::{Harness, LaunchRequest};
use harness_business::harnesses::Codex;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ApprovalMode, UiOutcome, UiRequest, UiResponse};
use tempfile::TempDir;

use crate::agent_support::{ScenarioSpawner, WAIT, binary, options, project};

/// The recorded thread every scenario starts.
pub const THREAD: &str = "01a164a0-4c41-7613-9735-20de2a8016a8";

/// The Codex home of a chat, a folder in its project (holding nothing unless a test writes it).
pub const HOME: &str = "codex-home";

/// How a chat launches besides its scenario: its approval mode and whether it trusts the project
/// once.
#[derive(Debug, Clone, Copy, Default)]
pub struct Setup {
    pub approval_override: Option<ApprovalMode>,
    pub trust_once: bool,
}

/// A Codex chat and what it saw.
pub struct CodexChat {
    pub session: AgentSession,
    pub sink: Arc<CollectingSink>,
    pub spawner: Arc<ScenarioSpawner>,
    /// The canonical project directory.
    pub project: PathBuf,
    /// Where the chat saves images.
    pub attachments: TempDir,
    _dir: TempDir,
}

/// What [`launch`] gives back: the session or the start's error, with what it ran on.
pub struct Launched {
    pub result: harness_business::Result<AgentSession>,
    pub sink: Arc<CollectingSink>,
    pub spawner: Arc<ScenarioSpawner>,
    pub project: PathBuf,
    pub attachments: TempDir,
    pub dir: TempDir,
}

/// Launches scenario `name`, resuming `resume`, without panicking on a failed start.
pub async fn launch(name: &str, resume: Option<&str>) -> Launched {
    launch_with(name, resume, Setup::default(), |_| {}).await
}

/// [`launch`] with `setup`, after `prepare` wrote what the test needs into the (canonical)
/// project.
pub async fn launch_with(name: &str, resume: Option<&str>, setup: Setup, prepare: impl FnOnce(&Path)) -> Launched {
    let dir = project();
    let project = dunce::canonicalize(dir.path()).expect("canonical project");
    prepare(&project);

    let attachments = tempfile::tempdir().expect("attachments dir");
    let sink = CollectingSink::new();
    let spawner = ScenarioSpawner::new("codex", name);

    let binary = binary();
    let request = LaunchRequest {
        binary: &binary,
        project: &project,
        resume,
        approval_override: setup.approval_override,
        trust_once: setup.trust_once,
        utility: false,
    };
    let options = harness_business::agent::SessionOptions {
        attachments_dir: Some(attachments.path().to_path_buf()),
        ..options()
    };

    let result = match Codex.launch_spec(&request) {
        Ok(spec) => started(spec, &spawner, &project, resume, setup, &sink, &options).await,
        Err(error) => Err(error),
    };

    Launched {
        result,
        sink,
        spawner,
        project,
        attachments,
        dir,
    }
}

/// [`AgentSession::start`] on `spec` with the harness's protocol.
async fn started(
    spec: harness_business::process::ExecSpec,
    spawner: &Arc<ScenarioSpawner>,
    project: &Path,
    resume: Option<&str>,
    setup: Setup,
    sink: &Arc<CollectingSink>,
    options: &harness_business::agent::SessionOptions,
) -> harness_business::Result<AgentSession> {
    AgentSession::start(SessionLaunch {
        key: "codex-1".to_owned(),
        spec,
        spawner: spawner.clone(),
        protocol: Codex.chat_protocol(),
        project: project.to_path_buf(),
        sessions_dir: project.to_path_buf(),
        config_dir: project.join(HOME),
        resume,
        approval_override: setup.approval_override,
        trust_once: setup.trust_once,
        utility: false,
        sink: sink.clone(),
        options,
        abort: Box::pin(std::future::pending()),
    })
    .await
}

/// Starts scenario `name` as a new chat; panics when the start fails.
pub async fn start(name: &str) -> CodexChat {
    start_with(name, Setup::default(), |_| {}).await
}

/// [`start`] with `setup` and `prepare` (see [`launch_with`]).
pub async fn start_with(name: &str, setup: Setup, prepare: impl FnOnce(&Path)) -> CodexChat {
    let launched = launch_with(name, None, setup, prepare).await;

    CodexChat {
        session: launched.result.expect("the Codex chat starts"),
        sink: launched.sink,
        spawner: launched.spawner,
        project: launched.project,
        attachments: launched.attachments,
        _dir: launched.dir,
    }
}

impl CodexChat {
    pub fn events(&self) -> Vec<AgentEvent> {
        self.sink.events()
    }

    /// Waits for an event `matches` takes; panics after the test wait.
    pub async fn wait(&self, what: &str, matches: impl Fn(&AgentEvent) -> bool) -> AgentEvent {
        crate::agent_support::wait_event(&self.sink, what, matches).await
    }

    /// Runs `action`, which must succeed.
    pub async fn act(&self, action: AgentAction) -> AgentReply {
        match self.session.send(action).await {
            Ok(reply) => reply,
            Err(error) => panic!("the Codex chat refused the action: {error}"),
        }
    }

    /// Waits for the next dialog not seen in `seen` dialogs before it.
    pub async fn dialog(&self, seen: usize) -> UiRequest {
        let deadline = tokio::time::Instant::now() + WAIT;

        loop {
            let shown: Vec<UiRequest> = self
                .events()
                .into_iter()
                .filter_map(|event| match event {
                    AgentEvent::UiRequest { request } => Some(request),
                    _ => None,
                })
                .collect();

            if let Some(request) = shown.into_iter().nth(seen) {
                return request;
            }

            assert!(tokio::time::Instant::now() < deadline, "no dialog {seen}");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    /// Picks `value` in dialog `id`.
    pub async fn pick(&self, id: &str, value: &str) -> harness_business::Result<()> {
        let outcome = UiOutcome::Value {
            value: value.to_owned(),
        };

        self.session
            .respond(UiResponse {
                id: id.to_owned(),
                outcome,
            })
            .await
    }

    /// Waits until the chat settled `times` times; every event so far.
    pub async fn settled(&self, times: usize) -> Vec<AgentEvent> {
        let count = |events: &[AgentEvent]| {
            events
                .iter()
                .filter(|event| matches!(event, AgentEvent::Settled { .. }))
                .count()
        };

        self.wait("settled", |_| count(&self.events()) >= times).await;
        self.events()
    }

    /// Stops the chat: the scripted Codex exits 0 only when every expected frame matched.
    pub async fn finish(&self) -> Vec<AgentEvent> {
        self.session.stop().await;

        let events = self.events();
        let exit = AgentEvent::Exited {
            code: Some(0),
            signal: None,
        };
        assert_eq!(events.last(), Some(&exit), "the scripted Codex saw what it expected");

        events
    }
}

/// A prompt without images or a streaming behaviour.
pub fn prompt(text: &str) -> AgentAction {
    AgentAction::Prompt {
        message: text.to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    }
}

/// The streamed text of every finished assistant message.
pub fn replies(events: &[AgentEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } => Some(message),
            _ => None,
        })
        .flat_map(|message| &message.blocks)
        .filter_map(|block| match block {
            harness_business::model::ChatBlock::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}
