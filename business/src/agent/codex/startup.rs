//! Startup of a [`CodexSession`]: spawn `codex app-server` → the stdout reader → `initialize`
//! (the app's name and version, the experimental API on, Codex's noise opted out of) → its result
//! within the startup limit ([`STARTUP_TIMEOUT`](crate::agent::STARTUP_TIMEOUT)), Codex's version
//! from its user agent → the `initialized` notification → `model/list` (a list Codex refuses or
//! does not send in time leaves the picker empty until it asks again) → for a helper chat or one
//! with a memory identity, the MCP servers Codex runs the project with (`config/read`,
//! `memory.rs`) → `thread/start` in the project, or `thread/read` (the thread must be of the
//! project or a folder inside it, else `notFound`) and `thread/resume` of the resumed thread
//! without its turns (a helper chat's thread is ephemeral: not saved, not listed, see
//! `naming.rs`) → `ready`, `state`, `commandsUpdated`, the notice of an untrusted project and the
//! one of a memory identity no server gets, then the events that arrived meanwhile. The skills
//! and a resumed thread's saved messages are read after that, in the background.
//!
//! - No model is sent: the user's `config.toml` applies. The approval policy and sandbox are
//!   those of the chat's approval mode, none without an override (`approvals.rs`, Auto is refused
//!   before anything spawns). A helper chat's thread only reads, without web search or MCP
//!   servers, and its asks are declined (`naming.rs`).
//! - The whole startup runs within the startup deadline and gives up when every session is stopped
//!   ([`run_startup`]); a failed one kills the process and emits nothing. A process that exits
//!   first fails the start with its last stderr lines.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;

use super::approvals::{mode_policy, trust_notice};
use super::io::{read_stdout, refused};
use super::memory::{McpServers, memory_config, memory_notice};
use super::naming::helper_thread;
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
            project: launch.project.clone(),
            attachments_dir: options.attachments_dir.clone(),
            export_dir: options.export_dir.clone(),
            helper: launch.utility,
            memory_session: launch.memory_session && !launch.utility,
            prompt_ack_timeout: options.prompt_ack_timeout,
            moving: tokio::sync::Mutex::new(()),
            approval_override: launch.approval_override,
            live: Mutex::new(Live::new(&launch.project, launch.utility)),
        });
        chat_tx.send(chat.clone()).ok();

        let cwd = launch.project.to_string_lossy().into_owned();
        let steps = handshake(&chat, &start, cwd, options.startup_timeout);

        let memory_notice = match run_startup(options, launch.abort, steps).await {
            Ok(notice) => notice,
            Err(error) => return Err(process.fail_start(error).await),
        };

        let notice = trust_notice(&launch.config_dir, &launch.project, launch.trust_once, launch.utility);
        chat.open(notice.into_iter().chain(memory_notice));
        chat.opened(matches!(start, Start::Resume(_)));

        Ok(Self { process, chat })
    }
}

impl Chat {
    /// Emits the first events, `notices` last.
    fn open(&self, notices: impl IntoIterator<Item = AgentEvent>) {
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
            .chain(notices)
            .collect()
        };

        self.shared.open(first);
    }
}

/// The handshake, the models, the MCP servers and the thread, each within `limit`; the notice of
/// a memory identity no server gets (`memory.rs`).
async fn handshake(chat: &Chat, start: &Start, cwd: String, limit: Duration) -> Result<Option<AgentEvent>> {
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

    let models = match chat.call(&Request::ModelList { include_hidden: false }, limit).await {
        Ok(Ok(listed)) => serde_json::from_value::<Page<Model>>(listed).map_or_else(|_| Vec::new(), |page| page.data),
        Ok(Err(error)) => {
            tracing::debug!(error = %error.message, "Codex listed no models");
            Vec::new()
        }
        Err(error) => {
            tracing::debug!(%error, "Codex did not list its models");
            Vec::new()
        }
    };

    // The helper's thread switches off every MCP server Codex would start for it, so a read that
    // fails fails the helper.
    let helper_servers = match chat.helper {
        true => chat.mcp_servers(limit).await.map_err(|error| {
            BusinessError::Protocol(format!(
                "Codex did not list the MCP servers the helper chat switches off: {error}"
            ))
        })?,
        false => McpServers::new(),
    };
    let memory_notice = chat.memory_session_servers(limit).await;

    // A thread of another folder is never resumed here (as a switch checks it too).
    if let Start::Resume(thread_id) = start {
        chat.check_owned(thread_id).await?;
    }

    let settings = chat.thread_settings(cwd);

    let request = match start {
        Start::New { ephemeral: true } => Request::ThreadStart(helper_thread(settings, &helper_servers)),
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
    if let Some(warning) = live.follow(session, chat.approval_override, &chat.project) {
        chat.shared.emit(warning);
    }

    Ok(memory_notice)
}

impl Chat {
    /// For a chat with a memory identity: the `env_vars` its threads get and whether the identity
    /// reaches a server (`memory.rs`), and the notice when it cannot. A read that fails leaves the
    /// identity with no server.
    async fn memory_session_servers(&self, limit: Duration) -> Option<AgentEvent> {
        if !self.memory_session {
            return None;
        }

        let servers = match self.mcp_servers(limit).await {
            Ok(servers) => servers,
            Err(error) => {
                tracing::warn!(%error, "Codex did not list its MCP servers, the chat's memory guards do not apply");
                return None;
            }
        };

        let memory = memory_config(&servers);
        let notice = memory_notice(&memory);

        let mut live = self.lock();
        live.memory_config = memory.config;
        live.memory_passed_on = memory.passed_on;
        live.memory_missed = memory.missed;

        notice
    }
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
