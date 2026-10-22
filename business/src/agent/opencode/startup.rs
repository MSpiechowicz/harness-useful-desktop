//! Startup of an [`OpenCodeSession`]: with an approval mode, first the probe server for the
//! mode's configuration (`probe.rs`) → spawn with a random server password (and the mode's
//! `OPENCODE_CONFIG_CONTENT`) → the listening line (within
//! [`STARTUP_TIMEOUT`](crate::agent::STARTUP_TIMEOUT)) → `GET /global/health` → the event stream,
//! opened before any call that changes something → `POST /session` or, to resume,
//! `GET /session/{id}` (refused unless the session's directory is the project or inside it) →
//! `GET /agent` (with a mode: checked against the probe's, see
//! [`check_applied`]), `/config/providers`, `/command` and `/config` (read only: they give the
//! model a new session shows, see [`server_default_model`]) → a helper chat's deny-all rules (`approvals.rs`) → `ready`,
//! `state` and `commandsUpdated`, then the events that arrived meanwhile. A helper chat never
//! resumes a session.
//!
//! The whole startup, the probe included, runs within one budget and abort race
//! ([`run_startup_until`]): the startup deadline, twice that with a mode (two cold starts); a failed one kills the server and emits nothing. Server output before
//! the listening line and after it is passed on like stderr.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{Notify, oneshot};

use super::approvals::{LaunchRules, deny_helper_tools};
use super::probe::{ModeLaunch, Probe, mode_launch, refused, with_content};
use super::{Catalog, Chat, Context, Live, OpenCodeSession, TurnSettings, new_mapper, stream};
use crate::agent::engine::SessionLaunch;
use crate::agent::io::Shared;
use crate::agent::process::{AgentProcess, LineForwarder, MAX_STDERR_LINE, ProcessTraits, Stdout, run_startup_until};
use crate::agent::{EventSink, SessionOptions};
use crate::error::{BusinessError, Result};
use crate::harness::CommandCatalog;
use crate::harnesses::opencode::OpenCodeCatalog;
use crate::model::AgentEvent;
use crate::opencode_server::mapper::project_dir;
use crate::opencode_server::mode_config::check_applied;
use crate::opencode_server::wire::{
    ModelRef, SessionInfo, listening_addr, primary_agents, provider_models, server_default_model, session_info,
};
use crate::opencode_server::{EventStream, OpenCodeClient, Route, ServerPassword, SessionId};
use crate::pi_family::rpc::LineSplitter;
use crate::process::{ExecSpec, ProcessSpawner};

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
    /// The model OpenCode uses while the chat names none (`wire::server_default_model`).
    default_model: Option<ModelRef>,
    /// The chat's agent.
    agent: String,
    /// The session was resumed (it may hold messages).
    resumed: bool,
}

impl OpenCodeSession {
    /// Spawns `opencode serve` and completes startup (see the module docs).
    pub async fn start(launch: SessionLaunch<'_>) -> Result<Self> {
        if launch.utility && launch.resume.is_some() {
            return Err(BusinessError::InvalidInput(
                "a helper chat cannot resume a session".to_owned(),
            ));
        }

        let rules = LaunchRules {
            approval: launch.approval_override,
            utility: launch.utility,
        };

        // With a mode the server starts twice (the probe first): the budget holds both.
        let options = launch.options;
        let mut abort = launch.abort;
        let starts = if rules.mode().is_some() { 2 } else { 1 };
        let budget = options.startup_deadline * starts;
        let until = tokio::time::Instant::now() + budget;
        let mut spec = launch.spec.clone();

        let moded = match rules.mode() {
            Some(mode) => {
                let probe = Probe {
                    spawner: launch.spawner.as_ref(),
                    spec: &spec,
                    key: &launch.key,
                    project: &launch.project,
                    resume: launch.resume,
                    options,
                };
                let moded = run_startup_until(budget, until, &mut abort, mode_launch(probe, mode)).await?;
                with_content(&mut spec, &moded.content);
                Some(moded)
            }
            None => None,
        };

        let Spawned {
            process,
            password,
            listening,
        } = spawn_server(launch.spawner.as_ref(), &spec, launch.key, launch.sink, options).map_err(
            |error| match &moded {
                Some(_) => BusinessError::Unsupported(format!(
                    "OpenCode could not be started with the approval mode's configuration ({error}). Start the chat \
                 without an approval mode"
                )),
                None => error,
            },
        )?;

        let limit = options.startup_timeout;
        let steps = boot(
            listening,
            password,
            &launch.project,
            launch.resume,
            rules,
            moded.as_ref(),
            limit,
        );

        match run_startup_until(budget, until, &mut abort, steps).await {
            Ok(booted) => Ok(Self::open(process, &launch.project, rules, options, booted)),
            Err(error) => Err(process.fail_start(error).await),
        }
    }

    /// Emits the first events and starts the event task.
    fn open(
        process: AgentProcess,
        project: &Path,
        rules: LaunchRules,
        options: &SessionOptions,
        booted: Booted,
    ) -> Self {
        let shared = process.shared().clone();
        let info = booted.info;

        let commands = OpenCodeCatalog.merged(&booted.commands);
        let live = Live {
            mapper: new_mapper(&booted.session, project, &booted.directory, rules),
            session: booted.session,
            title: info.title,
            model: info.model.or(booted.default_model),
            model_chosen: false,
            variant: info.variant,
            agent: booted.agent,
            turn: TurnSettings::default(),
            commands: command_names(&booted.commands),
            // A resumed session's newest reply is read when the state is first asked for.
            context: Context {
                mark: None,
                loaded: !booted.resumed,
            },
        };

        let chat = Arc::new(Chat {
            shared: shared.clone(),
            client: booted.client,
            directory: info.directory,
            project: project.to_path_buf(),
            root: booted.directory,
            export_dir: options.export_dir.clone(),
            catalog: booted.catalog,
            rules,
            live: Mutex::new(live),
            event_taken: Notify::new(),
        });

        let state = chat.state(&chat.lock());
        shared.open(vec![
            AgentEvent::Ready { protocol_version: None },
            AgentEvent::State { state },
            AgentEvent::CommandsUpdated { commands },
        ]);

        let watch = stream::Watch {
            exited: process.exit_watch(),
            terminator: process.terminator(),
            silence: options.event_silence,
        };
        let events = stream::spawn(chat.clone(), booted.events, watch);

        Self { process, chat, events }
    }
}

/// The startup steps after spawn (see the module docs).
async fn boot(
    listening: oneshot::Receiver<SocketAddr>,
    password: ServerPassword,
    project: &Path,
    resume: Option<&str>,
    rules: LaunchRules,
    moded: Option<&ModeLaunch>,
    limit: Duration,
) -> Result<Booted> {
    let client = listening_client(listening, password, limit).await?;
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

    if let Some(moded) = moded {
        check_applied(&moded.agents, &agents, &moded.config).map_err(|why| refused(&why))?;
    }

    let providers = get_json(&client, Route::ConfigProviders { directory: scope }).await?;
    let commands = get_json(&client, Route::Commands { directory: scope }).await?;
    let config = get_json(&client, Route::Config { directory: scope }).await?;

    let catalog = Catalog {
        models: provider_models(&providers),
        agents: primary_agents(&agents),
    };
    let agent = chat_agent(info.agent.as_deref(), &catalog);

    if rules.utility {
        deny_helper_tools(&client, &session).await?;
    }

    Ok(Booted {
        client,
        events,
        session,
        info,
        directory,
        commands: commands.as_array().cloned().unwrap_or_default(),
        default_model: server_default_model(&config, &providers),
        agent,
        catalog,
        resumed: resume.is_some(),
    })
}

/// A spawned `opencode serve`, its random password and where its address arrives.
pub(super) struct Spawned {
    pub process: AgentProcess,
    pub password: ServerPassword,
    pub listening: oneshot::Receiver<SocketAddr>,
}

/// Spawns `spec` as session `key` with a random server password (masked in its output and
/// events), its events going to `sink`; it is stopped with SIGTERM, as it ignores stdin EOF.
pub(super) fn spawn_server(
    spawner: &dyn ProcessSpawner,
    spec: &ExecSpec,
    key: String,
    sink: Arc<dyn EventSink>,
    options: &SessionOptions,
) -> Result<Spawned> {
    let password = ServerPassword::random();
    let mut spec = spec.clone();
    spec.env
        .push(("OPENCODE_SERVER_PASSWORD".into(), password.expose_secret().into()));
    spec.env.push(("OPENCODE_SERVER_USERNAME".into(), "opencode".into()));

    let traits = ProcessTraits {
        terminate_on_stop: true,
        secret: Some(password.expose_secret().to_owned()),
    };
    let (listening_tx, listening) = oneshot::channel();

    let process = AgentProcess::spawn(spawner, &spec, key, sink, options, traits, move |stdout, shared| {
        read_stdout(stdout, shared, listening_tx)
    })?;

    Ok(Spawned {
        process,
        password,
        listening,
    })
}

/// The agent the session last used when the chat can switch to it, else the default one.
fn chat_agent(last: Option<&str>, catalog: &Catalog) -> String {
    let known = |agent: &&str| catalog.agents.iter().any(|option| option.id == *agent);

    last.filter(known)
        .map(str::to_owned)
        .or_else(|| catalog.agents.first().map(|agent| agent.id.clone()))
        .unwrap_or_else(|| FALLBACK_AGENT.to_owned())
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
pub(super) async fn resumed(client: &OpenCodeClient, id: &str) -> Result<SessionInfo> {
    let session = SessionId::parse(id)?;
    let response = client.send(Route::SessionGet { session: &session }, None).await?;

    if response.status() == 404 {
        return Err(BusinessError::NotFound(format!("session {id}")));
    }

    read_session(&response.error_for_status()?.json()?)
}

/// The session in a reply that must hold one.
pub(super) fn read_session(body: &Value) -> Result<SessionInfo> {
    session_info(body).ok_or_else(|| BusinessError::Protocol("OpenCode sent a session without an id".to_owned()))
}

/// Waits for the server's address on `listening` (within `limit`), then checks its health.
pub(super) async fn listening_client(
    listening: oneshot::Receiver<SocketAddr>,
    password: ServerPassword,
    limit: Duration,
) -> Result<OpenCodeClient> {
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

    Ok(client)
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
pub(super) async fn read_stdout(stdout: Stdout, shared: Arc<Shared>, listening: oneshot::Sender<SocketAddr>) {
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
