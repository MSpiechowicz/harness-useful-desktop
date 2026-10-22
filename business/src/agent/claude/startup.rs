//! Startup of a [`ClaudeSession`]: spawn `claude` → the stdout reader → the `initialize` control
//! request (no app hooks yet) → its reply within the startup limit
//! ([`STARTUP_TIMEOUT`](crate::agent::STARTUP_TIMEOUT)): the slash commands, the models and the
//! permission mode, never the account → `ready`, `state` and `commandsUpdated`, then the events
//! that arrived meanwhile.
//!
//! The whole startup runs within the startup deadline and gives up when every session is stopped
//! ([`run_startup`]); a failed one kills the process and emits nothing. A process that exits
//! first fails the start with its last stderr lines. A resumed session keeps its id, a new one
//! learns it from the first frame that names it (a fresh `state` then). Helper chats are not
//! available yet.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::oneshot;

use super::io::read_stdout;
use super::settings::catalog;
use super::{Chat, ClaudeSession, Live};
use crate::agent::engine::SessionLaunch;
use crate::agent::process::{AgentProcess, ProcessTraits, run_startup};
use crate::claude_stream::wire::control::InitializeReply;
use crate::claude_stream::wire::outbound::HostRequest;
use crate::error::{BusinessError, Result};
use crate::model::AgentEvent;

impl ClaudeSession {
    /// Spawns `claude` and completes startup (see the module docs).
    pub async fn start(launch: SessionLaunch<'_>) -> Result<Self> {
        if launch.utility {
            return Err(BusinessError::Unsupported(
                "helper chats with Claude Code are not available yet".to_owned(),
            ));
        }

        let options = launch.options;
        let (chat_tx, chat_rx) = oneshot::channel();
        let process = AgentProcess::spawn(
            launch.spawner,
            &launch.spec,
            launch.key,
            launch.sink,
            options,
            ProcessTraits::default(),
            move |stdout, _shared| read_stdout(stdout, chat_rx),
        )?;

        let chat = Arc::new(Chat {
            shared: process.shared().clone(),
            live: Mutex::new(Live::new(&launch.project, launch.resume)),
        });
        chat_tx.send(chat.clone()).ok();

        let steps = initialize(&chat, options.startup_timeout);

        match run_startup(options, launch.abort, steps).await {
            Ok(reply) => Ok(Self::open(process, chat, reply)),
            Err(error) => Err(process.fail_start(error).await),
        }
    }

    /// Keeps what `initialize` told and emits the first events.
    fn open(process: AgentProcess, chat: Arc<Chat>, reply: InitializeReply) -> Self {
        let first = {
            let mut live = chat.lock();
            live.commands = reply.commands;
            live.models = reply.models;
            live.permission_mode = reply.current_permission_mode;

            vec![
                AgentEvent::Ready { protocol_version: None },
                AgentEvent::State {
                    state: chat.state(&live),
                },
                AgentEvent::CommandsUpdated {
                    commands: catalog(&live),
                },
            ]
        };

        chat.shared.open(first);

        Self { process, chat }
    }
}

/// `initialize` and its reply within `limit`.
async fn initialize(chat: &Chat, limit: Duration) -> Result<InitializeReply> {
    let request = HostRequest::Initialize { hooks: Vec::new() };

    let reply = chat.control(&request, limit).await.map_err(|error| match error {
        BusinessError::Timeout(_) => BusinessError::Timeout(format!(
            "Claude Code did not answer the handshake within {} s",
            limit.as_secs_f64()
        )),
        BusinessError::Process(_) => BusinessError::Process("the agent exited during startup".to_owned()),
        other => other,
    })?;

    serde_json::from_value(reply)
        .map_err(|_| BusinessError::Protocol("Claude Code sent an unreadable handshake reply".to_owned()))
}
