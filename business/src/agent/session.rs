//! One running agent process spoken to over RPC.
//!
//! Startup: spawn → handshake (OMP `ready` within 10 s; PI none) → the dialect's post-handshake
//! commands → `get_state` (PI's probe, also within 10 s) → the command list → `ready`, `state`
//! and `commandsUpdated` are emitted, then everything the agent sent meanwhile. A failed startup
//! kills the process and emits nothing.
//!
//! Stop: close stdin → wait up to the grace period (3 s) → kill the process group. `exited` is
//! always the last event and is emitted exactly once, also when the agent crashes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use super::SessionOptions;
use super::io::{Readers, Shared, WriterMsg, read_loop, stderr_loop, wait_loop, write_loop};
use super::sink::{EmitMsg, EventSink, spawn_emitter};
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply, BashOutput, MAX_ACTION_TEXT_BYTES, SessionState, UiResponse};
use crate::pi_family::normalize::{chat_message, model_option, string};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;
use crate::pi_family::rpc::shared::{items, parse_bash, parse_reply, slice_history};
use crate::pi_family::rpc::{
    DecodeState, EncodeContext, Handshake, RequestKind, RpcDialect, RpcResponse, await_response, encode_frame,
    timeout_for,
};
use crate::process::{ExecSpec, ExitInfo, ProcessSpawner};
use crate::security::redact_text;

/// Time limit for a `bash` command.
pub const BASH_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// How long a killed agent may take to be reaped.
const KILL_WAIT: Duration = Duration::from_secs(5);

/// How long `stop` waits for the last events to be delivered.
const EMITTER_WAIT: Duration = Duration::from_secs(2);

/// History pages read at most (256 messages each).
const MAX_HISTORY_PAGES: usize = 64;

/// Frames queued for stdin at most.
const WRITER_QUEUE: usize = 64;

/// Everything needed to start a session.
pub struct SessionLaunch<'a> {
    /// Desktop session key.
    pub key: String,
    /// The command to run.
    pub spec: ExecSpec,
    /// Starts the process.
    pub spawner: &'a dyn ProcessSpawner,
    /// The harness's dialect.
    pub dialect: &'static dyn RpcDialect,
    /// Canonical project directory.
    pub project: PathBuf,
    /// The project's session folder (for `switchSession`).
    pub sessions_dir: PathBuf,
    /// Receives the events.
    pub sink: Arc<dyn EventSink>,
    /// Timing knobs.
    pub options: &'a SessionOptions,
}

/// A running agent session (see the module docs).
pub struct AgentSession {
    shared: Arc<Shared>,
    sessions_dir: PathBuf,
    next_id: AtomicU64,
    exited: watch::Receiver<Option<ExitInfo>>,
    kill: watch::Sender<bool>,
    emitter: Mutex<Option<JoinHandle<()>>>,
    stop_grace: Duration,
    protocol_version: Option<u32>,
}

impl AgentSession {
    /// Spawns the agent and completes startup.
    pub async fn start(launch: SessionLaunch<'_>) -> Result<Self> {
        let process = launch.spawner.spawn(&launch.spec)?;
        let (events, emitter) = spawn_emitter(launch.key.clone(), launch.sink, launch.options.coalesce_window);
        let (writer, frames) = mpsc::channel(WRITER_QUEUE);
        let shared = Arc::new(Shared::new(launch.dialect, writer, events));

        let (ready_tx, ready_rx) = oneshot::channel();
        let (kill_tx, kill_rx) = watch::channel(false);
        let (exit_tx, exit_rx) = watch::channel(None);

        tokio::spawn(write_loop(process.stdin, frames));
        let readers = Readers {
            stdout: tokio::spawn(read_loop(
                process.stdout,
                shared.clone(),
                ready_tx,
                DecodeState::new(&launch.project),
            )),
            stderr: tokio::spawn(stderr_loop(process.stderr, shared.clone())),
        };
        tokio::spawn(wait_loop(process.child, kill_rx, exit_tx, readers, shared.clone()));

        let mut session = Self {
            shared,
            sessions_dir: launch.sessions_dir,
            next_id: AtomicU64::new(0),
            exited: exit_rx,
            kill: kill_tx,
            emitter: Mutex::new(Some(emitter)),
            stop_grace: launch.options.stop_grace,
            protocol_version: None,
        };

        match session.startup(ready_rx, launch.options.startup_timeout).await {
            Ok(()) => Ok(session),
            Err(error) => {
                session.kill_now().await;
                Err(with_stderr(error, &session.shared.stderr_tail()))
            }
        }
    }

    async fn startup(&mut self, ready: oneshot::Receiver<Option<u32>>, limit: Duration) -> Result<()> {
        let dialect = self.shared.dialect;

        if dialect.handshake() == Handshake::ReadyFrame {
            self.protocol_version = tokio::time::timeout(limit, ready)
                .await
                .map_err(|_| {
                    BusinessError::Timeout(format!("the agent sent no ready frame within {} s", limit.as_secs()))
                })?
                .map_err(|_| BusinessError::Process("the agent exited during startup".to_owned()))?;
        }

        for mut frame in dialect.after_handshake() {
            let id = self.next_id();
            frame["id"] = Value::from(id.as_str());
            self.request(&frame, &id, RequestKind::Other, DEFAULT_TIMEOUT).await?;
        }

        let state = match self.call(&AgentAction::GetState, limit).await? {
            AgentReply::State { state } => state,
            _ => return Err(BusinessError::Protocol("get_state returned no state".to_owned())),
        };

        let commands = match self.call(&AgentAction::RefreshCommands, DEFAULT_TIMEOUT).await {
            Ok(AgentReply::Commands { commands }) => commands,
            _ => dialect.catalog().merged(&[]),
        };

        let first = vec![
            AgentEvent::Ready {
                protocol_version: self.protocol_version,
            },
            AgentEvent::State { state },
            AgentEvent::CommandsUpdated { commands },
        ];
        self.shared.events.send(EmitMsg::Open(first)).ok();

        Ok(())
    }

    /// RPC protocol version from the handshake (OMP).
    pub fn protocol_version(&self) -> Option<u32> {
        self.protocol_version
    }

    /// How the process ended, once it has.
    pub fn exit_info(&self) -> Option<ExitInfo> {
        *self.exited.borrow()
    }

    /// True until the process exited.
    pub fn is_running(&self) -> bool {
        self.exit_info().is_none()
    }

    fn ensure_running(&self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }

        Err(BusinessError::Process("the agent session has exited".to_owned()))
    }

    fn next_id(&self) -> String {
        format!("req-{}", self.next_id.fetch_add(1, Ordering::Relaxed) + 1)
    }

    /// Sends `frame` (whose id is `id`) and waits for its successful response.
    async fn request(&self, frame: &Value, id: &str, kind: RequestKind, limit: Duration) -> Result<RpcResponse> {
        let label = frame
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("command")
            .to_owned();
        let receiver = self.shared.pending.register(id, kind)?;

        if self
            .shared
            .writer
            .send(WriterMsg::Frame(encode_frame(frame)))
            .await
            .is_err()
        {
            self.shared.pending.forget(id);
            return Err(BusinessError::Process("the agent no longer reads input".to_owned()));
        }

        let shared = &self.shared;
        let response = await_response(receiver, limit, &label, || shared.has_open_dialog()).await;

        if response.is_err() {
            self.shared.pending.forget(id);
        }

        let response = response?;
        if !response.success {
            let error = redact_text(response.error.as_deref().unwrap_or("no reason given"));
            return Err(BusinessError::Protocol(format!("{label} failed: {error}")));
        }

        Ok(response)
    }

    /// Encodes `action`, sends it and parses the reply (no side effects).
    async fn call(&self, action: &AgentAction, limit: Duration) -> Result<AgentReply> {
        let id = self.next_id();
        let context = EncodeContext {
            sessions_dir: &self.sessions_dir,
        };
        let frame = self.shared.dialect.encode(action, &id, &context)?;

        let kind = match action {
            AgentAction::Prompt { .. } => RequestKind::Prompt,
            _ => RequestKind::Other,
        };

        let response = self.request(&frame, &id, kind, limit).await?;
        let reply = parse_reply(action, response.data.as_ref(), self.shared.dialect.catalog())?;

        if let AgentAction::SetModel { .. } = action
            && let Some(model) = response.data.as_ref().and_then(model_option)
        {
            self.shared.emit(AgentEvent::ModelChanged { model: Some(model) });
        }

        Ok(reply)
    }

    /// Runs `action` (validated first). Session-changing actions also emit a fresh `state`;
    /// `refreshCommands` also emits `commandsUpdated`.
    pub async fn send(&self, action: AgentAction) -> Result<AgentReply> {
        action.validate()?;
        self.ensure_running()?;

        if let AgentAction::GetHistory { cursor, limit } = &action {
            return self.history(cursor.as_deref(), *limit).await;
        }

        let reply = self.call(&action, timeout_for(&action)).await?;

        match (&action, &reply) {
            (AgentAction::RefreshCommands, AgentReply::Commands { commands }) => {
                self.shared.emit(AgentEvent::CommandsUpdated {
                    commands: commands.clone(),
                });
            }
            (
                AgentAction::NewSession
                | AgentAction::SwitchSession { .. }
                | AgentAction::Fork { .. }
                | AgentAction::Clone,
                _,
            ) => {
                if let Ok(state) = self.state().await {
                    self.shared.emit(AgentEvent::State { state });
                }
            }
            _ => {}
        }

        Ok(reply)
    }

    async fn state(&self) -> Result<SessionState> {
        match self.call(&AgentAction::GetState, DEFAULT_TIMEOUT).await? {
            AgentReply::State { state } => Ok(state),
            _ => Err(BusinessError::Protocol("get_state returned no state".to_owned())),
        }
    }

    /// Reads the whole history (paging when the dialect pages) and serves the requested page.
    async fn history(&self, cursor: Option<&str>, limit: u32) -> Result<AgentReply> {
        let dialect = self.shared.dialect;
        let mut raw: Vec<Value> = Vec::new();
        let mut page: Option<String> = None;

        for _ in 0..MAX_HISTORY_PAGES {
            let id = self.next_id();
            let frame = dialect.history_request(page.as_deref(), &id);
            let response = self.request(&frame, &id, RequestKind::Other, DEFAULT_TIMEOUT).await?;
            let data = response.data.unwrap_or(Value::Null);

            raw.extend(items(&data, "messages"));
            page = string(&data, "nextCursor").filter(|_| dialect.history_paged());

            if page.is_none() {
                break;
            }
        }

        slice_history(raw.iter().map(chat_message).collect(), cursor, limit)
    }

    /// Answers an open dialog; fails for ids that are not open (answered, cancelled or unknown).
    pub async fn respond(&self, response: UiResponse) -> Result<()> {
        self.ensure_running()?;

        if let crate::model::UiOutcome::Value { value } = &response.outcome
            && value.len() > MAX_ACTION_TEXT_BYTES
        {
            return Err(BusinessError::InvalidInput(format!(
                "value exceeds {MAX_ACTION_TEXT_BYTES} bytes"
            )));
        }

        if !self.shared.close_dialog(&response.id) {
            return Err(BusinessError::NotFound(format!("open dialog {}", response.id)));
        }

        let frame = self.shared.dialect.encode_ui_response(&response);
        self.shared
            .writer
            .send(WriterMsg::Frame(encode_frame(&frame)))
            .await
            .map_err(|_| BusinessError::Process("the agent no longer reads input".to_owned()))
    }

    /// Runs `command` through the agent's `bash` RPC. The caller must have obtained the user's
    /// explicit confirmation.
    pub async fn bash(&self, command: &str) -> Result<BashOutput> {
        self.ensure_running()?;

        if command.trim().is_empty() || command.len() > MAX_ACTION_TEXT_BYTES {
            return Err(BusinessError::InvalidInput(format!(
                "command must be 1 to {MAX_ACTION_TEXT_BYTES} bytes"
            )));
        }

        let id = self.next_id();
        let frame = self.shared.dialect.encode_bash(command, &id);
        let response = self.request(&frame, &id, RequestKind::Other, BASH_TIMEOUT).await?;

        Ok(parse_bash(response.data.as_ref()))
    }

    /// Stops the agent (see the module docs) and waits until `exited` was delivered.
    pub async fn stop(&self) {
        self.shared.writer.try_send(WriterMsg::Close).ok();

        if !self.wait_exit(self.stop_grace).await {
            self.kill_now().await;
        }

        let emitter = self.emitter.lock().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(emitter) = emitter {
            tokio::time::timeout(EMITTER_WAIT, emitter).await.ok();
        }
    }

    async fn kill_now(&self) {
        self.kill.send_replace(true);
        self.wait_exit(KILL_WAIT).await;
    }

    async fn wait_exit(&self, limit: Duration) -> bool {
        let mut exited = self.exited.clone();
        let done = exited.wait_for(Option::is_some);

        matches!(tokio::time::timeout(limit, done).await, Ok(Ok(_)))
    }
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        // Safety net: a session dropped without `stop` must not leave its agent running.
        self.kill.send_replace(true);
    }
}

/// Adds the agent's last stderr lines to a startup error.
fn with_stderr(error: BusinessError, tail: &str) -> BusinessError {
    if tail.is_empty() {
        return error;
    }

    match error {
        BusinessError::Timeout(message) => BusinessError::Timeout(format!("{message} (stderr: {tail})")),
        BusinessError::Process(message) => BusinessError::Process(format!("{message} (stderr: {tail})")),
        BusinessError::Protocol(message) => BusinessError::Protocol(format!("{message} (stderr: {tail})")),
        other => other,
    }
}
