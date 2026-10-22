//! Startup of a [`CodexSession`]: spawn `codex app-server` → the stdout reader → `initialize`
//! (the app's name and version, the experimental API on, Codex's noise opted out of) → its result
//! within the startup limit ([`STARTUP_TIMEOUT`](crate::agent::STARTUP_TIMEOUT)), Codex's version
//! from its user agent → the `initialized` notification → `model/list` (a list Codex refuses
//! leaves the picker empty) → `thread/start` in the project, or `thread/resume` of the resumed
//! thread without its turns (a helper chat's thread is ephemeral: not saved, not listed) →
//! `ready`, `state`, `commandsUpdated` and the notice of an untrusted project, then the events that
//! arrived meanwhile.
//!
//! - No model is sent: the user's `config.toml` applies. The approval policy and sandbox are
//!   those of the chat's approval mode, none without an override (`approvals.rs`, Auto is refused
//!   before anything spawns).
//! - The whole startup runs within the startup deadline and gives up when every session is stopped
//!   ([`run_startup`]); a failed one kills the process and emits nothing. A process that exits
//!   first fails the start with its last stderr lines.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;

use super::approvals::{answered_mode, mode_policy, trust_notice};
use super::io::{read_stdout, refused};
use super::settings::commands;
use super::{Chat, CodexSession, Live};
use crate::agent::engine::SessionLaunch;
use crate::agent::process::{AgentProcess, ProcessTraits, run_startup};
use crate::codex_rpc::wire::answers::initialized;
use crate::codex_rpc::wire::outbound::{Initialize, Request, ThreadResume, ThreadStart};
use crate::codex_rpc::wire::results::{InitializeResult, Model, Page, ThreadSession};
use crate::error::{BusinessError, Result};
use crate::harnesses::codex::is_thread_id;
use crate::model::AgentEvent;

/// What a thread starts with.
enum Start {
    /// A new thread; ephemeral for a helper chat.
    New { ephemeral: bool },
    /// The saved thread with this id.
    Resume(String),
}

impl CodexSession {
    /// Spawns `codex app-server` and completes startup (see the module docs).
    pub async fn start(launch: SessionLaunch<'_>) -> Result<Self> {
        mode_policy(launch.approval_override)?;

        let start = match launch.resume {
            Some(id) if !is_thread_id(id) => {
                return Err(BusinessError::InvalidInput(
                    "a Codex thread id is a lowercase UUID".to_owned(),
                ));
            }
            Some(id) => Start::Resume(id.to_owned()),
            None => Start::New {
                ephemeral: launch.utility,
            },
        };

        let options = launch.options;
        let (chat_tx, chat_rx) = oneshot::channel();
        let process = AgentProcess::spawn(
            launch.spawner.as_ref(),
            &launch.spec,
            launch.key,
            launch.sink,
            options,
            ProcessTraits::default(),
            move |stdout, _shared| read_stdout(stdout, chat_rx),
        )?;

        let chat = Arc::new(Chat {
            shared: process.shared().clone(),
            attachments_dir: options.attachments_dir.clone(),
            approval_override: launch.approval_override,
            live: Mutex::new(Live::new(&launch.project, launch.utility)),
        });
        chat_tx.send(chat.clone()).ok();

        let cwd = launch.project.to_string_lossy().into_owned();
        let steps = handshake(&chat, &start, cwd, options.startup_timeout);

        if let Err(error) = run_startup(options, launch.abort, steps).await {
            return Err(process.fail_start(error).await);
        }

        let notice = trust_notice(&launch.config_dir, &launch.project, launch.trust_once, launch.utility);
        chat.open(notice);

        Ok(Self { process, chat })
    }
}

impl Chat {
    /// Emits the first events, `notice` last.
    fn open(&self, notice: Option<AgentEvent>) {
        let first = {
            let live = self.lock();

            vec![
                AgentEvent::Ready { protocol_version: None },
                AgentEvent::State {
                    state: self.state(&live),
                },
                AgentEvent::CommandsUpdated { commands: commands() },
            ]
            .into_iter()
            .chain(notice)
            .collect()
        };

        self.shared.open(first);
    }
}

/// The handshake, the models and the thread, each within `limit`.
async fn handshake(chat: &Chat, start: &Start, cwd: String, limit: Duration) -> Result<()> {
    let hello = Request::Initialize(Initialize::desktop(env!("CARGO_PKG_VERSION")));
    let init: InitializeResult = chat.request(&hello, limit).await.map_err(|error| match error {
        BusinessError::Timeout(_) => BusinessError::Timeout(format!(
            "Codex did not answer the handshake within {} s",
            limit.as_secs_f64()
        )),
        BusinessError::Process(_) => BusinessError::Process("the agent exited during startup".to_owned()),
        other => other,
    })?;

    chat.write(&initialized()).await?;

    let version = init.version().map(str::to_owned);
    tracing::debug!(
        version = version.as_deref().unwrap_or("unknown"),
        "Codex app server started"
    );

    let models = match chat.call(&Request::ModelList { include_hidden: false }, limit).await? {
        Ok(listed) => serde_json::from_value::<Page<Model>>(listed).map_or_else(|_| Vec::new(), |page| page.data),
        Err(error) => {
            tracing::debug!(error = %error.message, "Codex listed no models");
            Vec::new()
        }
    };

    let settings = chat.thread_settings(cwd);

    let request = match start {
        Start::New { ephemeral } => Request::ThreadStart(ThreadStart {
            settings,
            ephemeral: ephemeral.then_some(true),
            ..ThreadStart::default()
        }),
        Start::Resume(thread_id) => Request::ThreadResume(ThreadResume {
            thread_id: thread_id.clone(),
            settings,
            exclude_turns: true,
        }),
    };

    let session = thread(chat, &request, limit).await?;

    let mut live = chat.lock();
    live.codex_version = version;
    live.models = models;
    live.mapper.follow_thread(&session.thread.id);
    live.thread_id = Some(session.thread.id.clone()).filter(|id| !id.is_empty());
    live.title = session.thread.name.clone().filter(|name| !name.trim().is_empty());
    live.thread_model = Some(session.model).filter(|model| !model.is_empty());
    live.thread_effort = session.reasoning_effort;
    live.mode = chat
        .approval_override
        .or_else(|| answered_mode(session.approval_policy.as_ref()));

    if !session.model_provider.is_empty() {
        live.provider = session.model_provider;
    }

    Ok(())
}

/// Starts or resumes the thread; Codex's refusal fails the start with its reason.
async fn thread(chat: &Chat, request: &Request, limit: Duration) -> Result<ThreadSession> {
    let answer = chat.call(request, limit).await?;
    let session = answer.map_err(|error| refused(request.method(), &error))?;

    serde_json::from_value::<ThreadSession>(session)
        .ok()
        .filter(|session| !session.thread.id.is_empty())
        .ok_or_else(|| BusinessError::Protocol(format!("Codex sent an unreadable answer to {}", request.method())))
}
