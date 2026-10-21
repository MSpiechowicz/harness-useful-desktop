//! Startup of an [`OpenCodeV2Session`]: with an approval mode, first the probe server for the
//! mode's configuration (`probe.rs`) → spawn with a random server password (and the mode's
//! `OPENCODE_CONFIG_CONTENT`) → the listening line (`server listening on http://…`, within
//! [`STARTUP_TIMEOUT`](crate::agent::STARTUP_TIMEOUT)) → `GET /api/info` until the server is ready
//! (`503` while it starts, a refused password fails) → the event stream, opened before any call
//! that changes something → `POST /api/session` (a helper chat's with rules that deny every tool,
//! which the answer must show) or, to resume, `GET /api/session/{id}` (refused unless the session's
//! directory is the project or inside it) and every message of the resumed session, marked as
//! seen → `GET /api/agent` (OpenCode loads a directory's agents after the first request there, so
//! an empty list is asked again for at most [`AGENT_WAIT`]; with a mode until they have the mode's
//! rules, then the project's saved "Always allow" rules are read, see `probe.rs`), `/api/model`,
//! `/api/command` and `/api/skill` ([`Slash`]; and `/api/model/default` for the model a session
//! that names none uses) →
//! `ready`, `state` and `commandsUpdated`, then the events that arrived meanwhile. A helper chat
//! never resumes a session.
//!
//! The whole startup, the probe included, runs within one budget and abort race
//! ([`run_startup_until`]): the startup deadline, twice that with a mode (two cold starts); a
//! failed one kills the server and emits nothing. Server output before the listening line and
//! after it is passed on like stderr.

use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::oneshot;

use super::approvals::{Allowed, LaunchRules, denies_all};
use super::catalog::Catalog;
use super::commands::Slash;
use super::history::{all_messages, last_context};
use super::probe::{ModeLaunch, Probe, check_mode, mode_launch, with_content};
use super::{Chat, Live, OpenCodeV2Session, normalized, stream};
use crate::agent::engine::SessionLaunch;
use crate::agent::io::Shared;
use crate::agent::process::{AgentProcess, LineForwarder, MAX_STDERR_LINE, ProcessTraits, Stdout, run_startup_until};
use crate::agent::{EventSink, SessionOptions};
use crate::error::{BusinessError, Result};
use crate::harness::CommandCatalog;
use crate::harnesses::opencode::OpenCodeCatalog;
use crate::model::AgentEvent;
use crate::opencode_server::mapper::project_dir;
use crate::opencode_server::{EventStream, OpenCodeClient, ServerPassword};
use crate::opencode_server_v2::messages::SessionMessage;
use crate::opencode_server_v2::modes::deny_all;
use crate::opencode_server_v2::wire::{Agent, CreateBody, Envelope, Model, ModelRef, Session, listening_addr};
use crate::opencode_server_v2::{Mapper, RouteV2, SessionId};
use crate::pi_family::rpc::LineSplitter;
use crate::process::{ExecSpec, ProcessSpawner};

/// How often `GET /api/info` is asked again while the server starts (`503`).
const READY_POLL: Duration = Duration::from_millis(100);

/// How long an empty `GET /api/agent` is asked again: OpenCode 2.1.2 loads a directory's agents
/// after the first request there and lists them a few hundred milliseconds later.
pub(super) const AGENT_WAIT: Duration = Duration::from_secs(3);

/// How often `GET /api/agent` is asked again while it is empty or settling.
pub(super) const AGENT_POLL: Duration = Duration::from_millis(100);

/// The wait between two equal agent lists that count as settled: OpenCode 2.1.2 lists its own
/// agents first and adds the rules of the configuration and of its browser connection about
/// 200 ms later.
const AGENT_SETTLE: Duration = Duration::from_millis(300);

/// What a completed startup hands over.
struct Booted {
    client: OpenCodeClient,
    events: EventStream,
    session: SessionId,
    info: Session,
    /// The session's directory, canonical.
    directory: std::path::PathBuf,
    catalog: Catalog,
    slash: Slash,
    /// The chat's agent.
    agent: String,
    /// The session's model, or the one OpenCode uses while the session names none.
    model: Option<ModelRef>,
    /// A resumed session's messages, oldest first (none for a new one).
    messages: Vec<SessionMessage>,
}

impl OpenCodeV2Session {
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
                    spawner: launch.spawner,
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
        } = spawn_server(launch.spawner, &spec, launch.key, launch.sink, options).map_err(|error| match &moded {
            Some(_) => BusinessError::Unsupported(format!(
                "OpenCode could not be started with the approval mode's configuration ({error}). Start the chat \
                 without an approval mode"
            )),
            None => error,
        })?;

        let steps = boot(
            listening,
            password,
            &launch.project,
            launch.resume,
            rules,
            moded.as_ref(),
            options.startup_timeout,
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

        let mut mapper = Mapper::new(booted.session.as_str(), project, &booted.directory);
        mapper.seen(&booted.messages);
        mapper.set_mode_applied(rules.mode().is_some());

        let commands = OpenCodeCatalog.merged(&booted.slash.entries);
        let live = Live {
            session: booted.session,
            mapper,
            title: info.title,
            model: booted.model.map(normalized),
            agent: booted.agent,
            commands: booted.slash.commands,
            skills: booted.slash.skills,
            context: last_context(&booted.messages),
            picks: Vec::new(),
            mark: info.time.updated,
            shell: None,
            allowed: Allowed::default(),
        };

        let chat = Arc::new(Chat {
            shared: shared.clone(),
            client: booted.client,
            directory: info.location.directory,
            project: project.to_path_buf(),
            root: booted.directory,
            export_dir: options.export_dir.clone(),
            catalog: booted.catalog,
            rules,
            live: Mutex::new(live),
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
    ready(&client).await?;
    let events = client.stream(RouteV2::Event).await?;

    let project_text = project
        .to_str()
        .ok_or_else(|| BusinessError::InvalidInput("the project path is not valid UTF-8".to_owned()))?;
    let info = match resume {
        Some(id) => resumed(&client, id, project_text).await?,
        None => created(&client, project_text, rules.utility).await?,
    };

    let directory = project_dir(&info.location.directory, project).ok_or_else(|| match resume {
        Some(id) => BusinessError::NotFound(format!("session {id} in this project")),
        None => BusinessError::Protocol("OpenCode created the session outside the project".to_owned()),
    })?;
    let session = SessionId::parse(&info.id)
        .map_err(|_| BusinessError::Protocol("OpenCode sent an invalid session id".to_owned()))?;

    let scope = info.location.directory.as_str();
    let messages = match resume {
        Some(_) => all_messages(&client, &session, scope).await?,
        None => Vec::new(),
    };

    let listed = match moded {
        Some(moded) => check_mode(&client, moded, scope, &info.project_id).await?,
        None => agent_list(&client, scope, false).await?,
    };

    let agents: Vec<Agent> = serde_json::from_value(listed)
        .map_err(|error| BusinessError::Protocol(format!("OpenCode sent an unreadable agent list: {error}")))?;
    let models: Vec<Model> = get_data(&client, RouteV2::ModelList { directory: scope }).await?;
    let slash = Slash::read(&client, scope).await?;

    let catalog = Catalog::new(models, &agents);
    let agent = catalog.chat_agent(info.agent.as_deref());
    let model = match &info.model {
        Some(model) => Some(model.clone()),
        None => default_model(&client, scope).await,
    };

    Ok(Booted {
        client,
        events,
        session,
        info,
        directory,
        catalog,
        slash,
        agent,
        model,
        messages: messages.iter().map(SessionMessage::parse).collect(),
    })
}

/// `GET /api/agent` in `directory` (its `data`): asked again while it is empty, for at most
/// [`AGENT_WAIT`], and when `settle`, until two reads [`AGENT_SETTLE`] apart agree.
pub(super) async fn agent_list(client: &OpenCodeClient, directory: &str, settle: bool) -> Result<Value> {
    let until = tokio::time::Instant::now() + AGENT_WAIT;
    let mut listed: Value = get_data(client, RouteV2::AgentList { directory }).await?;

    while is_empty_list(&listed) && tokio::time::Instant::now() < until {
        tokio::time::sleep(AGENT_POLL).await;
        listed = get_data(client, RouteV2::AgentList { directory }).await?;
    }

    while settle && !is_empty_list(&listed) && tokio::time::Instant::now() < until {
        tokio::time::sleep(AGENT_SETTLE).await;
        let again: Value = get_data(client, RouteV2::AgentList { directory }).await?;

        if again == listed {
            break;
        }

        listed = again;
    }

    Ok(listed)
}

fn is_empty_list(listed: &Value) -> bool {
    listed.as_array().is_none_or(Vec::is_empty)
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

    let traits = ProcessTraits {
        terminate_on_stop: true,
        secret: Some(password.expose_secret().to_owned()),
    };
    let (listening_tx, listening) = oneshot::channel();

    // `spawner` is the app's tracked spawner, so a crashed app's server is killed on the next start.
    let process = AgentProcess::spawn(spawner, &spec, key, sink, options, traits, move |stdout, shared| {
        read_stdout(stdout, shared, listening_tx)
    })?;

    Ok(Spawned {
        process,
        password,
        listening,
    })
}

/// Waits for the server's address on `listening` (within `limit`).
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

    Ok(OpenCodeClient::new(addr, password)?)
}

/// Asks `GET /api/info` until the server is ready: `503` while it starts; a refused password or
/// any other failure ends the startup.
pub(super) async fn ready(client: &OpenCodeClient) -> Result<()> {
    loop {
        let response = client.send(RouteV2::Info, None).await?;

        match response.status() {
            503 => tokio::time::sleep(READY_POLL).await,
            401 => {
                return Err(BusinessError::Protocol(
                    "OpenCode refused the chat's server password (HTTP 401)".to_owned(),
                ));
            }
            _ => {
                response.error_for_status()?;
                return Ok(());
            }
        }
    }
}

/// `POST /api/session` in `directory`; for a helper chat with rules that deny every tool, which
/// the created session must show.
async fn created(client: &OpenCodeClient, directory: &str, utility: bool) -> Result<Session> {
    let body = CreateBody {
        permissions: utility.then(deny_all),
        ..CreateBody::default()
    };
    let body =
        serde_json::to_value(body).map_err(|error| BusinessError::Internal(format!("the session request: {error}")))?;
    let response = client
        .send(RouteV2::SessionCreate { directory }, Some(&body))
        .await?
        .error_for_status()?;

    let session = response.json::<Envelope<Session>>()?.data;

    if utility && !denies_all(&session.permissions) {
        return Err(BusinessError::Protocol(
            "OpenCode did not keep the helper chat's rules that deny every tool".to_owned(),
        ));
    }

    Ok(session)
}

/// `GET /api/session/{id}`; `NotFound` when the server does not know it.
pub(super) async fn resumed(client: &OpenCodeClient, id: &str, directory: &str) -> Result<Session> {
    let session = SessionId::parse(id)?;
    let route = RouteV2::SessionGet {
        session: &session,
        directory,
    };
    let response = client.send(route, None).await?;

    if response.status() == 404 {
        return Err(BusinessError::NotFound(format!("session {id}")));
    }

    Ok(response.error_for_status()?.json::<Envelope<Session>>()?.data)
}

/// `GET /api/model/default`: the model OpenCode uses for a session that names none (a new session
/// gets none from `POST /api/session`); `None` when it has none or does not say.
async fn default_model(client: &OpenCodeClient, directory: &str) -> Option<ModelRef> {
    let model: Option<Model> = get_data(client, RouteV2::ModelDefault { directory }).await.ok()?;

    model.map(|model| model.reference())
}

/// The `data` of an enveloped `GET` that must succeed.
pub(super) async fn get_data<T: DeserializeOwned>(client: &OpenCodeClient, route: RouteV2<'_>) -> Result<T> {
    let response = client.send(route, None).await?.error_for_status()?;

    Ok(response.json::<Envelope<T>>()?.data)
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
