//! Startup of an [`OpenCodeSession`]: spawn with a random server password → the listening line
//! (within [`STARTUP_TIMEOUT`](crate::agent::STARTUP_TIMEOUT)) → `GET /global/health` → the event
//! stream, opened before any call that changes something → `POST /session` or, to resume,
//! `GET /session/{id}` (refused unless the session's directory is the project or inside it) →
//! `GET /agent`, `/config/providers`, `/command` → `ready`, `state` and `commandsUpdated`, then
//! the events that arrived meanwhile.
//!
//! The whole startup runs within the shared deadline and abort race ([`run_startup`]); a failed
//! one kills the server and emits nothing. Server output before the listening line and after it is
//! passed on like stderr.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::{Catalog, Chat, Live, OpenCodeSession, stream};
use crate::agent::engine::SessionLaunch;
use crate::agent::io::Shared;
use crate::agent::process::{AgentProcess, LineForwarder, MAX_STDERR_LINE, ProcessTraits, Stdout, run_startup};
use crate::error::{BusinessError, Result};
use crate::harness::CommandCatalog;
use crate::harnesses::opencode::OpenCodeCatalog;
use crate::model::{AgentEvent, ApprovalMode};
use crate::opencode_server::mapper::project_dir;
use crate::opencode_server::wire::{SessionInfo, listening_addr, primary_agents, provider_models, session_info};
use crate::opencode_server::{EventStream, Mapper, OpenCodeClient, Route, ServerPassword, SessionId};
use crate::pi_family::rpc::LineSplitter;

/// The agent every OpenCode install has, used when `/agent` lists none.
const FALLBACK_AGENT: &str = "build";

/// What a completed startup hands over.
struct Booted {
    client: OpenCodeClient,
    events: EventStream,
    session: SessionId,
    info: SessionInfo,
    /// The session's directory, canonical.
    directory: PathBuf,
    catalog: Catalog,
    commands: Vec<Value>,
}

impl OpenCodeSession {
    /// Spawns `opencode serve` and completes startup (see the module docs).
    pub async fn start(launch: SessionLaunch<'_>) -> Result<Self> {
        check_settings(launch.approval_override, launch.utility)?;

        let password = ServerPassword::random();
        let mut spec = launch.spec.clone();
        spec.env
            .push(("OPENCODE_SERVER_PASSWORD".into(), password.expose_secret().into()));
        spec.env.push(("OPENCODE_SERVER_USERNAME".into(), "opencode".into()));

        let traits = ProcessTraits {
            terminate_on_stop: true,
            secret: Some(password.expose_secret().to_owned()),
        };
        let (listening_tx, listening_rx) = oneshot::channel();

        let process = AgentProcess::spawn(
            launch.spawner,
            &spec,
            launch.key,
            launch.sink,
            launch.options,
            traits,
            move |stdout, shared| read_stdout(stdout, shared, listening_tx),
        )?;

        let limit = launch.options.startup_timeout;
        let steps = boot(listening_rx, password, &launch.project, launch.resume, limit);

        match run_startup(launch.options, launch.abort, steps).await {
            Ok(booted) => Ok(Self::open(process, &launch.project, booted)),
            Err(error) => Err(process.fail_start(error).await),
        }
    }

    /// Emits the first events and starts the event task.
    fn open(process: AgentProcess, project: &Path, booted: Booted) -> Self {
        let shared = process.shared().clone();
        let info = booted.info;

        let agent = info
            .agent
            .filter(|agent| booted.catalog.agents.iter().any(|known| &known.id == agent))
            .or_else(|| booted.catalog.agents.first().map(|agent| agent.id.clone()))
            .unwrap_or_else(|| FALLBACK_AGENT.to_owned());

        let commands = OpenCodeCatalog.merged(&booted.commands);
        let live = Live {
            mapper: Mapper::new(booted.session.as_str(), project, &booted.directory),
            session: booted.session,
            title: info.title,
            model: info.model,
            variant: info.variant,
            agent,
            commands: command_names(&booted.commands),
        };

        let chat = Arc::new(Chat {
            shared: shared.clone(),
            client: booted.client,
            directory: info.directory,
            catalog: booted.catalog,
            live: Mutex::new(live),
        });

        let state = chat.state(&chat.lock());
        shared.open(vec![
            AgentEvent::Ready { protocol_version: None },
            AgentEvent::State { state },
            AgentEvent::CommandsUpdated { commands },
        ]);

        let events = stream::spawn(chat.clone(), booted.events, process.exit_watch());

        Self { process, chat, events }
    }
}

/// Refuses the launch settings this engine cannot apply yet: an approval mode or a utility chat
/// needs per-session permission rules, and running without them would ask less than requested.
fn check_settings(approval_override: Option<ApprovalMode>, utility: bool) -> Result<()> {
    if approval_override.is_none() && !utility {
        return Ok(());
    }

    Err(BusinessError::Unsupported(
        "approval modes and helper chats are not available for OpenCode yet".to_owned(),
    ))
}

/// The startup steps after spawn (see the module docs).
async fn boot(
    listening: oneshot::Receiver<SocketAddr>,
    password: ServerPassword,
    project: &Path,
    resume: Option<&str>,
    limit: Duration,
) -> Result<Booted> {
    let addr = tokio::time::timeout(limit, listening)
        .await
        .map_err(|_| {
            BusinessError::Timeout(format!(
                "OpenCode did not report its server address within {} s",
                limit.as_secs_f64()
            ))
        })?
        .map_err(|_| BusinessError::Process("the agent exited during startup".to_owned()))?;

    let client = OpenCodeClient::new(addr, password)?;
    client.send(Route::Health, None).await?.error_for_status()?;

    let events = client.events().await?;

    let project_text = project
        .to_str()
        .ok_or_else(|| BusinessError::InvalidInput("the project path is not valid UTF-8".to_owned()))?;
    let info = match resume {
        Some(id) => resumed(&client, id).await?,
        None => created(&client, project_text).await?,
    };

    let directory = project_dir(&info.directory, project).ok_or_else(|| match resume {
        Some(id) => BusinessError::NotFound(format!("session {id} in this project")),
        None => BusinessError::Protocol("OpenCode created the session outside the project".to_owned()),
    })?;
    let session = SessionId::parse(&info.id)
        .map_err(|_| BusinessError::Protocol("OpenCode sent an invalid session id".to_owned()))?;

    let scope = info.directory.as_str();
    let agents = get_json(&client, Route::Agents { directory: scope }).await?;
    let providers = get_json(&client, Route::ConfigProviders { directory: scope }).await?;
    let commands = get_json(&client, Route::Commands { directory: scope }).await?;

    Ok(Booted {
        client,
        events,
        session,
        info,
        directory,
        catalog: Catalog {
            models: provider_models(&providers),
            agents: primary_agents(&agents),
        },
        commands: commands.as_array().cloned().unwrap_or_default(),
    })
}

/// `POST /session?directory=`.
async fn created(client: &OpenCodeClient, directory: &str) -> Result<SessionInfo> {
    let response = client
        .send(Route::SessionCreate { directory }, Some(&json!({})))
        .await?
        .error_for_status()?;

    read_session(&response.json()?)
}

/// `GET /session/{id}`; `NotFound` when the server does not know it.
async fn resumed(client: &OpenCodeClient, id: &str) -> Result<SessionInfo> {
    let session = SessionId::parse(id)?;
    let response = client.send(Route::SessionGet { session: &session }, None).await?;

    if response.status() == 404 {
        return Err(BusinessError::NotFound(format!("session {id}")));
    }

    read_session(&response.error_for_status()?.json()?)
}

fn read_session(body: &Value) -> Result<SessionInfo> {
    session_info(body).ok_or_else(|| BusinessError::Protocol("OpenCode sent a session without an id".to_owned()))
}

/// The JSON body of a `GET` that must succeed.
pub(super) async fn get_json(client: &OpenCodeClient, route: Route<'_>) -> Result<Value> {
    Ok(client.send(route, None).await?.error_for_status()?.json()?)
}

/// The names in a `GET /command` list.
pub(super) fn command_names(commands: &[Value]) -> Vec<String> {
    commands
        .iter()
        .filter_map(|command| command.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// Reads stdout: the listening line goes to startup, every other line is passed on like stderr.
async fn read_stdout(stdout: Stdout, shared: Arc<Shared>, listening: oneshot::Sender<SocketAddr>) {
    let mut lines = LineSplitter::new(stdout, MAX_STDERR_LINE);
    let mut forwarder = LineForwarder::default();
    let mut listening = Some(listening);

    while let Some(line) = lines.next_line().await {
        if let Some(sender) = listening.take() {
            match listening_addr(&String::from_utf8_lossy(&line.bytes)) {
                Some(addr) => {
                    sender.send(addr).ok();
                    continue;
                }
                None => listening = Some(sender),
            }
        }

        forwarder.pass(&shared, &line);
    }

    forwarder.finish(&shared);
}
