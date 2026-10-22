//! Starts the OpenCode chat engine directly ([`AgentSession::start`]) against the scripted
//! `opencode serve` (`fake_agent --dialect opencode`). Starts through the manager are in
//! `opencode_launch.rs`.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use harness_business::agent::{AgentSession, CollectingSink, SessionLaunch, SessionOptions, StartAbort};
use harness_business::harness::ChatProtocol;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ApprovalMode, ChatMessage, MessageRole};
use harness_business::process::{ExecSpec, ProcessSpawner};
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
    start_as(name, resume, None, false, options).await
}

/// Starts scenario `name` with an approval override, as a helper chat when `utility`.
pub async fn start_mode(name: &str, approval: Option<ApprovalMode>, utility: bool) -> Chat {
    start_as(name, None, approval, utility, options()).await
}

/// Starts scenario `name` with every launch setting.
pub async fn start_as(
    name: &str,
    resume: Option<&str>,
    approval: Option<ApprovalMode>,
    utility: bool,
    options: SessionOptions,
) -> Chat {
    start_launch(name, resume, approval, utility, options, &[]).await
}

/// Starts scenario `name` (not a helper chat) with `env` in the server's launch environment (an
/// inherited `OPENCODE_CONFIG_CONTENT`, say).
pub async fn start_env(
    name: &str,
    resume: Option<&str>,
    approval: Option<ApprovalMode>,
    options: SessionOptions,
    env: &[(&str, &str)],
) -> Chat {
    start_launch(name, resume, approval, false, options, env).await
}

async fn start_launch(
    name: &str,
    resume: Option<&str>,
    approval: Option<ApprovalMode>,
    utility: bool,
    options: SessionOptions,
    env: &[(&str, &str)],
) -> Chat {
    let launch = Launch {
        name,
        resume,
        approval,
        utility,
        options,
        env,
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
    pub env: &'a [(&'a str, &'a str)],
}

/// Starts `launch` through the spawner `through` makes of the scenario spawner (which the chat
/// keeps for inspection), given up when `abort` resolves.
pub async fn start_through(
    launch: Launch<'_>,
    through: impl FnOnce(Arc<ScenarioSpawner>) -> Arc<dyn ProcessSpawner>,
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
    let sink = CollectingSink::new();
    let spawner = ScenarioSpawner::new("opencode", name);
    let used = through(spawner.clone());

    let mut spec = ExecSpec::new("opencode").args(["serve", "--hostname", "127.0.0.1", "--port", "0", "--mdns=false"]);
    spec.cwd = Some(project.clone());
    spec.env
        .extend(env.iter().map(|(name, value)| ((*name).into(), (*value).into())));

    let launch = SessionLaunch {
        key: "key-1".to_owned(),
        spec,
        spawner: used.clone(),
        protocol: ChatProtocol::OpenCodeServer,
        project: project.clone(),
        sessions_dir: project.clone(),
        config_dir: project.clone(),
        resume,
        approval_override: approval,
        trust_once: false,
        utility,
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

/// A plain prompt.
pub fn text_prompt(message: &str) -> AgentAction {
    AgentAction::Prompt {
        message: message.to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    }
}

/// Sends `message` as a prompt that starts a turn; how many events were delivered when the
/// prompt-taken call ran, once it ran.
pub async fn prompt(chat: &Chat, message: &str) -> Arc<Mutex<Option<usize>>> {
    let taken_at = Arc::new(Mutex::new(None));
    let (sink, record) = (chat.sink.clone(), taken_at.clone());
    chat.session().on_prompt_taken(Box::new(move || {
        *record.lock().unwrap() = Some(sink.events().len());
    }));

    let reply = chat.session().send(text_prompt(message)).await.expect("prompt taken");
    assert_eq!(
        reply,
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("started".to_owned())
        }
    );

    taken_at
}

/// Waits for the `Marker` title the scenarios end with; the events of the scenario after startup.
pub async fn until_marker(chat: &Chat) -> Vec<AgentEvent> {
    chat.wait(
        "marker",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Marker")),
    )
    .await;

    chat.events().split_off(3)
}

/// The messages of `role` that ended.
pub fn message_end(events: &[AgentEvent], role: MessageRole) -> Vec<ChatMessage> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } if message.role == role => Some(message.clone()),
            _ => None,
        })
        .collect()
}

/// The texts of the ended user messages.
pub fn user_texts(events: &[AgentEvent]) -> Vec<String> {
    message_end(events, MessageRole::User)
        .into_iter()
        .filter_map(|message| match message.blocks.first() {
            Some(harness_business::model::ChatBlock::Text { text }) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// Every `queueUpdate` as `(steering, follow-ups, live steered)`.
pub fn queue_updates(events: &[AgentEvent]) -> Vec<(Vec<String>, Vec<String>, u32)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::QueueUpdate {
                steering,
                follow_up,
                live_steered,
            } => Some((steering.clone(), follow_up.clone(), *live_steered)),
            _ => None,
        })
        .collect()
}

/// How many events of type `name` there are.
pub fn count(events: &[AgentEvent], name: &str) -> usize {
    events.iter().filter(|event| kind(event) == name).count()
}

/// Waits for the dialog `id`; panics after [`WAIT`].
pub async fn dialog(chat: &Chat, id: &str) -> harness_business::model::UiRequest {
    let event = chat
        .wait(
            id,
            |event| matches!(event, AgentEvent::UiRequest { request } if request.id == id),
        )
        .await;

    match event {
        AgentEvent::UiRequest { request } => request,
        _ => unreachable!(),
    }
}

/// Answers dialog `id` with `outcome`.
pub async fn answer(chat: &Chat, id: &str, outcome: harness_business::model::UiOutcome) {
    let response = harness_business::model::UiResponse {
        id: id.to_owned(),
        outcome,
    };

    chat.session().respond(response).await.expect("answered");
}

/// Answers dialog `id` with the option or text `value`.
pub async fn choose(chat: &Chat, id: &str, value: &str) {
    let value = value.to_owned();
    answer(chat, id, harness_business::model::UiOutcome::Value { value }).await;
}
