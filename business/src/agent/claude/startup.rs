//! Startup of a [`ClaudeSession`] and of every later process of the chat: spawn `claude` → the
//! stdout reader → the `initialize` control request (with the ask-all `PreToolUse` hook for an
//! Always ask override) → its reply within the startup limit
//! ([`STARTUP_TIMEOUT`](crate::agent::STARTUP_TIMEOUT)): the slash commands, the models and the
//! permission mode, never the account → `ready`, `state` and `commandsUpdated`, then the events
//! that arrived meanwhile.
//!
//! - **Session:** a new chat names its session (`--session-id <fresh uuid>`), so its id is known
//!   from the start; a resumed one is `--resume <id>`; a fork or clone `--resume <id>
//!   --fork-session --session-id <fresh uuid>`, a fork cut with `--resume-session-at <uuid>`. A
//!   frame that names another id still wins (a fresh `state` then). A helper chat saves nothing and
//!   learns its id from the frames.
//! - **Model:** the chat's pick (`--model=<value>`, not for `default`). A new session the chat has
//!   picked no model for starts on Harness Skills' default role when its models file names one
//!   (`--model=<model>` and `--effort=<effort>`, see `roles.rs`), its thinking level then that
//!   effort while the model offers it. A resumed, forked, cloned or helper session never reads it.
//! - **Messages:** a session that starts with saved messages (resumed, forked, cloned) counts them
//!   from its session file, best effort.
//! - The whole startup runs within the startup deadline and gives up when every session is stopped
//!   ([`run_startup`]); a failed one kills the process and emits nothing. A process that exits
//!   first fails the start with its last stderr lines.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{oneshot, watch};

use super::io::read_stdout;
use super::ops::blocking;
use super::relay::{Port, Relay};
use super::reports::{counts, saved_messages};
use super::roles::{Selection, default_selection};
use super::settings::{DEFAULT_MODEL, catalog, thinking_offered};
use super::{Chat, ClaudeSession, Live, Origin, Run};
use crate::agent::SessionOptions;
use crate::agent::engine::SessionLaunch;
use crate::agent::process::{AgentProcess, ProcessTraits, StartAbort, run_startup};
use crate::claude_stream::permissions::ask_all_hooks;
use crate::claude_stream::wire::control::InitializeReply;
use crate::claude_stream::wire::outbound::{HostRequest, new_message_uuid};
use crate::error::{BusinessError, Result};
use crate::model::{AgentEvent, ApprovalMode};
use crate::process::{ExecSpec, ProcessSpawner};

/// The flags that name the session or its model, each with its value (`--fork-session` has none).
const SESSION_FLAGS: [&str; 4] = ["--resume", "--session-id", "--resume-session-at", "--model"];

/// The model flag with its value in one argument (`--model=sonnet`), so no value reads as a flag.
const MODEL_FLAG: &str = "--model=";

/// The effort flag with its value in one argument (`--effort=high`).
const EFFORT_FLAG: &str = "--effort=";

/// The flag that copies the resumed session.
const FORK_FLAG: &str = "--fork-session";

/// How a process starts its session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Start {
    /// A new session with this id.
    New(String),
    /// A saved session.
    Resume(String),
    /// A copy of session `source` under the id `id`, cut after the message `at` when it is a fork.
    Fork {
        source: String,
        id: String,
        at: Option<String>,
    },
    /// A helper chat: nothing saved.
    Helper,
}

impl Start {
    /// A new session with a fresh id.
    pub(super) fn fresh() -> Self {
        Self::New(fresh_id())
    }

    /// The id the session has from the start.
    fn session_id(&self) -> Option<&str> {
        match self {
            Self::New(id) | Self::Resume(id) | Self::Fork { id, .. } => Some(id),
            Self::Helper => None,
        }
    }

    /// Its command-line flags.
    fn args(&self) -> Vec<String> {
        let pair = |flag: &str, value: &str| [flag.to_owned(), value.to_owned()];

        match self {
            Self::New(id) => pair("--session-id", id).to_vec(),
            Self::Resume(id) => pair("--resume", id).to_vec(),
            Self::Fork { source, id, at } => {
                let mut args = pair("--resume", source).to_vec();
                args.push(FORK_FLAG.to_owned());
                args.extend(pair("--session-id", id));

                if let Some(at) = at {
                    args.extend(pair("--resume-session-at", at));
                }

                args
            }
            Self::Helper => Vec::new(),
        }
    }

    /// The saved messages it starts with: a session and the message they end with.
    fn origin(&self) -> Option<Origin> {
        match self {
            Self::Resume(id) => Some(Origin {
                session: id.clone(),
                cut: None,
            }),
            Self::Fork { source, at, .. } => Some(Origin {
                session: source.clone(),
                cut: at.clone(),
            }),
            Self::New(_) | Self::Helper => None,
        }
    }
}

/// What starting a process of the chat takes.
pub(super) struct Launcher {
    key: String,
    spawner: Arc<dyn ProcessSpawner>,
    /// The launch's command line without the session flags.
    pub(super) base: ExecSpec,
    pub(super) project: PathBuf,
    pub(super) sessions_dir: PathBuf,
    /// Claude Code's config directory, where Harness Skills keeps its model roles.
    config_dir: PathBuf,
    ask_all: bool,
    pub(super) helper: bool,
    relay: Arc<Relay>,
    pub(super) options: SessionOptions,
}

impl ClaudeSession {
    /// Spawns `claude` and completes startup (see the module docs).
    pub async fn start(launch: SessionLaunch<'_>) -> Result<Self> {
        // The launch spec refuses Yolo too: it needs Claude Code's bypass mode.
        if launch.approval_override == Some(ApprovalMode::Yolo) {
            return Err(BusinessError::Unsupported(
                "Claude Code chats have no Yolo mode: choose Always ask, Write or Auto".to_owned(),
            ));
        }

        let helper = launch.utility;
        let start = match launch.resume {
            _ if helper => Start::Helper,
            Some(id) => Start::Resume(id.to_owned()),
            None => Start::fresh(),
        };

        let launcher = Box::new(Launcher {
            key: launch.key,
            spawner: launch.spawner,
            base: base_spec(launch.spec),
            project: launch.project,
            sessions_dir: launch.sessions_dir,
            config_dir: launch.config_dir,
            ask_all: !helper && launch.approval_override == Some(ApprovalMode::AlwaysAsk),
            helper,
            relay: Relay::new(launch.sink),
            options: launch.options.clone(),
        });

        let (run, reply) = launcher.launch(&start, None, launch.abort).await?;
        run.open(reply);

        Ok(Self {
            current: Mutex::new(run),
            launcher,
            moving: tokio::sync::Mutex::new(()),
            stopped: watch::channel(false).0,
        })
    }
}

impl Launcher {
    /// Spawns `claude` for `start` (on `model`, the chat's pick, when there is one, else a new
    /// session on Harness Skills' default role) and completes the handshake; the process is not
    /// opened yet. A failed start kills it.
    pub(super) async fn launch(
        &self,
        start: &Start,
        model: Option<&str>,
        abort: StartAbort<'_>,
    ) -> Result<(Arc<Run>, InitializeReply)> {
        let role = match start {
            Start::New(_) if model.is_none() => self.default_role().await,
            _ => None,
        };

        let spec = command_line(&self.base, start, model, role.as_ref());
        let port = Port::new(&self.relay);
        let options = &self.options;
        let (chat_tx, chat_rx) = oneshot::channel();
        let process = AgentProcess::spawn(
            self.spawner.as_ref(),
            &spec,
            self.key.clone(),
            port.clone(),
            options,
            ProcessTraits::default(),
            move |stdout, _shared| read_stdout(stdout, chat_rx),
        )?;

        let mut live = Live::new(&self.project, start.session_id(), self.ask_all, self.helper);
        live.model = model.map(str::to_owned);

        if let Some(role) = role {
            live.launched_model = Some(role.model);
            live.thinking = role.effort;
        }

        if matches!(start, Start::Fork { .. }) {
            live.origin = start.origin();
        }

        let chat = Arc::new(Chat {
            shared: process.shared().clone(),
            live: Mutex::new(live),
        });
        chat_tx.send(chat.clone()).ok();

        let steps = initialize(&chat, self.ask_all, options.startup_timeout);

        let reply = match run_startup(options, abort, steps).await {
            Ok(reply) => reply,
            Err(error) => return Err(process.fail_start(error).await),
        };

        if let Some(origin) = start.origin() {
            let saved = saved_messages(&self.sessions_dir, &self.project, &origin.session, origin.cut).await;
            chat.lock().counts = counts(&saved.unwrap_or_default());
        }

        Ok((Arc::new(Run { process, chat, port }), reply))
    }

    /// Harness Skills' default role, read off the async workers; `None` on any failure.
    async fn default_role(&self) -> Option<Selection> {
        let dir = self.config_dir.clone();

        blocking(move || Ok(default_selection(&dir))).await.ok().flatten()
    }
}

impl Run {
    /// Keeps what `initialize` told and emits the first events.
    pub(super) fn open(&self, reply: InitializeReply) {
        let chat = &self.chat;

        let first = {
            let mut live = chat.lock();
            live.commands = reply.commands;
            live.models = reply.models;
            live.permission_mode = reply.current_permission_mode;

            // The effort the process started with shows only when its model offers it.
            if !thinking_offered(&live) {
                live.thinking = None;
            }

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
    }
}

/// A fresh session id (a UUID v4, as Claude Code's own).
pub(super) fn fresh_id() -> String {
    new_message_uuid()
}

/// `base` with the flags of `start`, `--model=<id>` for a picked model other than the default, else
/// `--model=<model>` and `--effort=<effort>` of `role`.
fn command_line(base: &ExecSpec, start: &Start, model: Option<&str>, role: Option<&Selection>) -> ExecSpec {
    let mut spec = base.clone();
    spec.args.extend(start.args().into_iter().map(OsString::from));

    let picked = model.filter(|model| *model != DEFAULT_MODEL);

    if let Some(model) = picked.or(role.map(|role| role.model.as_str())) {
        spec.args.push(OsString::from(format!("{MODEL_FLAG}{model}")));
    }

    if let Some(effort) = role.and_then(|role| role.effort.as_deref()) {
        spec.args.push(OsString::from(format!("{EFFORT_FLAG}{effort}")));
    }

    spec
}

/// `spec` without the session flags the harness's launch spec adds (see [`SESSION_FLAGS`]).
fn base_spec(mut spec: ExecSpec) -> ExecSpec {
    let mut args = Vec::with_capacity(spec.args.len());
    let mut skip_value = false;

    for arg in spec.args {
        if skip_value {
            skip_value = false;
            continue;
        }

        let name = arg.to_str().unwrap_or_default();

        if SESSION_FLAGS.contains(&name) {
            skip_value = true;
        } else if name != FORK_FLAG && !name.starts_with(MODEL_FLAG) {
            args.push(arg);
        }
    }

    spec.args = args;
    spec
}

/// `initialize` (with the ask-all hook when `ask_all`) and its reply within `limit`.
async fn initialize(chat: &Chat, ask_all: bool, limit: Duration) -> Result<InitializeReply> {
    let hooks = if ask_all { ask_all_hooks() } else { Vec::new() };
    let request = HostRequest::Initialize { hooks };

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
