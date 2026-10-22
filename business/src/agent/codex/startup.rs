//! Startup of a [`CodexSession`]: spawn `codex app-server` → the stdout reader → `initialize`
//! (the app's name and version, the experimental API on, Codex's noise opted out of) → its result
//! within the startup limit ([`STARTUP_TIMEOUT`](crate::agent::STARTUP_TIMEOUT)), Codex's version
//! from its user agent → the `initialized` notification → `model/list` (a list Codex refuses
//! leaves the picker empty) → `thread/start` in the project, or `thread/resume` of the resumed
//! thread without its turns (a helper chat's thread is ephemeral: not saved, not listed, see
//! `naming.rs`) → `ready`, `state` and `commandsUpdated`, then the events that arrived meanwhile.
//! The skills and a resumed thread's saved messages are read after that, in the background.
//!
//! - No model, approval policy or sandbox is sent: the user's `config.toml` applies (a helper
//!   chat's thread never asks and only reads, see `naming.rs`). An approval override is refused
//!   until the chat maps the approval modes.
//! - The whole startup runs within the startup deadline and gives up when every session is stopped
//!   ([`run_startup`]); a failed one kills the process and emits nothing. A process that exits
//!   first fails the start with its last stderr lines.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;

use super::io::{read_stdout, refused};
use super::naming::helper_thread;
use super::settings::commands;
use super::{Chat, CodexSession, Live};
use crate::agent::engine::SessionLaunch;
use crate::agent::process::{AgentProcess, ProcessTraits, run_startup};
use crate::codex_rpc::wire::answers::initialized;
use crate::codex_rpc::wire::outbound::{Initialize, Request, ThreadResume, ThreadSettings, ThreadStart};
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
        if launch.approval_override.is_some() {
            return Err(BusinessError::Unsupported(
                "Codex chats have no approval modes yet".to_owned(),
            ));
        }

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
            project: launch.project.clone(),
            attachments_dir: options.attachments_dir.clone(),
            export_dir: options.export_dir.clone(),
            helper: launch.utility,
            moving: tokio::sync::Mutex::new(()),
            live: Mutex::new(Live::new(&launch.project, launch.utility)),
        });
        chat_tx.send(chat.clone()).ok();

        let cwd = launch.project.to_string_lossy().into_owned();
        let steps = handshake(&chat, &start, cwd, options.startup_timeout);

        if let Err(error) = run_startup(options, launch.abort, steps).await {
            return Err(process.fail_start(error).await);
        }

        chat.open();
        chat.opened(matches!(start, Start::Resume(_)));

        Ok(Self { process, chat })
    }
}

impl Chat {
    /// Emits the first events.
    fn open(&self) {
        let first = {
            let live = self.lock();

            vec![
                AgentEvent::Ready { protocol_version: None },
                AgentEvent::State {
                    state: self.state(&live),
                },
                AgentEvent::CommandsUpdated { commands: commands() },
            ]
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

    let settings = ThreadSettings {
        cwd: Some(cwd),
        ..ThreadSettings::default()
    };

    let request = match start {
        Start::New { ephemeral: true } => Request::ThreadStart(helper_thread(settings)),
        Start::New { ephemeral: false } => Request::ThreadStart(ThreadStart {
            settings,
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
    live.follow(session);

    Ok(())
}

/// Starts, resumes or forks a thread; Codex's refusal fails with its reason.
pub(super) async fn thread(chat: &Chat, request: &Request, limit: Duration) -> Result<ThreadSession> {
    let answer = chat.call(request, limit).await?;
    let session = answer.map_err(|error| refused(request.method(), &error))?;

    serde_json::from_value::<ThreadSession>(session)
        .ok()
        .filter(|session| !session.thread.id.is_empty())
        .ok_or_else(|| BusinessError::Protocol(format!("Codex sent an unreadable answer to {}", request.method())))
}
