//! The RPC chat engine: one running agent process spoken to over the PI-family RPC protocol.
//!
//! Startup: spawn, then the steps of `startup.rs`, all within a deadline. A failed startup kills
//! the process and emits nothing. The process lifecycle (tasks, stop, exit) is
//! [`AgentProcess`]'s.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::oneshot;

use super::engine::SessionLaunch;
use super::io::{Shared, WriterMsg, read_loop};
use super::process::{AgentProcess, ProcessTraits};
use super::sink::PromptTaken;
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply, BashOutput, MAX_ACTION_TEXT_BYTES, SessionState, UiResponse};
use crate::pi_family::normalize::{chat_message, hidden_message, model_option, string};
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;
use crate::pi_family::rpc::shared::{items, parse_bash, parse_reply, slice_history};
use crate::pi_family::rpc::{
    DecodeState, EncodeContext, RequestKind, RpcDialect, RpcResponse, await_response, encode_frame, timeout_for,
};
use crate::process::ExitInfo;
use crate::security::redact_text;

/// Time limit for a `bash` command.
pub const BASH_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// History pages read at most (256 messages each).
const MAX_HISTORY_PAGES: usize = 64;

/// How often a move is checked for (the agent answers the prompt before the file is in place).
const MOVE_CHECKS: usize = 10;

/// Pause between two move checks.
const MOVE_CHECK_INTERVAL: Duration = Duration::from_millis(200);

/// A running RPC agent session (see the module docs).
pub struct RpcSession {
    process: AgentProcess,
    pub(super) shared: Arc<Shared>,
    pub(super) dialect: &'static dyn RpcDialect,
    /// The project's session folder; follows the session when it moves to another project.
    sessions_dir: Mutex<PathBuf>,
    next_id: AtomicU64,
    pub(super) protocol_version: Option<u32>,
}

impl RpcSession {
    /// Spawns the agent and completes startup. The resume, approval override and utility settings
    /// of `launch` are already in its command line.
    pub async fn start(launch: SessionLaunch<'_>, dialect: &'static dyn RpcDialect) -> Result<Self> {
        let (ready_tx, ready_rx) = oneshot::channel();
        let decode = DecodeState::new(&launch.project);

        let process = AgentProcess::spawn(
            launch.spawner,
            &launch.spec,
            launch.key,
            launch.sink,
            launch.options,
            ProcessTraits::default(),
            move |stdout, shared| read_loop(stdout, shared, dialect, ready_tx, decode),
        )?;

        let mut session = Self {
            shared: process.shared().clone(),
            process,
            dialect,
            sessions_dir: Mutex::new(launch.sessions_dir),
            next_id: AtomicU64::new(0),
            protocol_version: None,
        };

        let started = session.startup(ready_rx, launch.options, launch.abort).await;

        match started {
            Ok(()) => Ok(session),
            Err(error) => Err(session.process.fail_start(error).await),
        }
    }

    /// RPC protocol version from the handshake (OMP).
    pub fn protocol_version(&self) -> Option<u32> {
        self.protocol_version
    }

    /// How the process ended, once it has.
    pub fn exit_info(&self) -> Option<ExitInfo> {
        self.process.exit_info()
    }

    /// True until the process exited.
    pub fn is_running(&self) -> bool {
        self.process.is_running()
    }

    pub(super) fn next_id(&self) -> String {
        format!("req-{}", self.next_id.fetch_add(1, Ordering::Relaxed) + 1)
    }

    /// Sends `frame` (whose id is `id`) and waits for its successful response.
    pub(super) async fn request(
        &self,
        frame: &Value,
        id: &str,
        kind: RequestKind,
        limit: Duration,
    ) -> Result<RpcResponse> {
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
    pub(super) async fn call(&self, action: &AgentAction, limit: Duration) -> Result<AgentReply> {
        let id = self.next_id();
        let sessions_dir = self.sessions_dir();
        let context = EncodeContext {
            sessions_dir: &sessions_dir,
        };
        let frame = self.dialect.encode(action, &id, &context)?;

        let kind = match action {
            AgentAction::Prompt { .. } => RequestKind::Prompt,
            _ => RequestKind::Other,
        };

        let response = self.request(&frame, &id, kind, limit).await?;
        let reply = parse_reply(action, response.data.as_ref(), self.dialect.catalog())?;

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
        self.process.ensure_running()?;

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

    fn sessions_dir(&self) -> PathBuf {
        self.sessions_dir.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Moves the running session, conversation file and working directory, to `project` (whose
    /// session folder is `sessions_dir`) with the harness's move command (OMP `/move`). The move is
    /// confirmed by the session file now living in `sessions_dir`; otherwise it fails and the
    /// session stays where it was (OMP rolls a failed move back itself).
    pub async fn move_project(&self, project: &Path, sessions_dir: &Path) -> Result<()> {
        self.process.ensure_running()?;

        let target = project
            .to_str()
            .ok_or_else(|| BusinessError::InvalidInput("the project path is not valid UTF-8".to_owned()))?;
        let prompt = self.dialect.move_prompt(target).ok_or_else(|| {
            BusinessError::Unsupported("this harness cannot move a session to another project".to_owned())
        })?;

        if self.state().await?.is_streaming {
            return Err(BusinessError::Unsupported(
                "the agent is working; move the chat once it is done".to_owned(),
            ));
        }

        self.call(
            &AgentAction::Prompt {
                message: prompt,
                images: Vec::new(),
                streaming_behavior: None,
            },
            DEFAULT_TIMEOUT,
        )
        .await?;

        for _ in 0..MOVE_CHECKS {
            if self
                .session_file()
                .await?
                .is_some_and(|file| file.starts_with(sessions_dir))
            {
                *self.sessions_dir.lock().unwrap_or_else(PoisonError::into_inner) = sessions_dir.to_path_buf();
                self.shared.change_project(project.to_path_buf());

                return Ok(());
            }

            tokio::time::sleep(MOVE_CHECK_INTERVAL).await;
        }

        Err(BusinessError::Process(
            "the agent did not move the session; it stays in its current project".to_owned(),
        ))
    }

    /// The session file the agent writes to (`get_state`'s `sessionFile`), kept out of [`SessionState`].
    async fn session_file(&self) -> Result<Option<PathBuf>> {
        let id = self.next_id();
        let sessions_dir = self.sessions_dir();
        let context = EncodeContext {
            sessions_dir: &sessions_dir,
        };
        let frame = self.dialect.encode(&AgentAction::GetState, &id, &context)?;
        let response = self.request(&frame, &id, RequestKind::Other, DEFAULT_TIMEOUT).await?;

        Ok(response
            .data
            .as_ref()
            .and_then(|data| data.get("sessionFile"))
            .and_then(Value::as_str)
            .map(PathBuf::from))
    }

    async fn state(&self) -> Result<SessionState> {
        match self.call(&AgentAction::GetState, DEFAULT_TIMEOUT).await? {
            AgentReply::State { state } => Ok(state),
            _ => Err(BusinessError::Protocol("get_state returned no state".to_owned())),
        }
    }

    /// Reads the whole history (paging when the dialect pages) and serves the requested page.
    async fn history(&self, cursor: Option<&str>, limit: u32) -> Result<AgentReply> {
        let dialect = self.dialect;
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

        let shown = raw.iter().filter(|message| !hidden_message(message)).map(chat_message);

        slice_history(shown.collect(), cursor, limit)
    }

    /// Answers an open dialog; fails for ids that are not open (answered, cancelled or unknown).
    pub async fn respond(&self, response: UiResponse) -> Result<()> {
        self.process.ensure_running()?;

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

        let frame = self.dialect.encode_ui_response(&response);
        self.shared
            .writer
            .send(WriterMsg::Frame(encode_frame(&frame)))
            .await
            .map_err(|_| BusinessError::Process("the agent no longer reads input".to_owned()))
    }

    /// Runs `command` through the agent's `bash` RPC. The caller must have obtained the user's
    /// explicit confirmation.
    pub async fn bash(&self, command: &str) -> Result<BashOutput> {
        self.process.ensure_running()?;
        check_command(command)?;

        let id = self.next_id();
        let frame = self.dialect.encode_bash(command, &id);
        let response = self.request(&frame, &id, RequestKind::Other, BASH_TIMEOUT).await?;

        Ok(parse_bash(response.data.as_ref()))
    }

    /// Puts `event` into the session's stream, after the events already in it.
    pub fn emit(&self, event: AgentEvent) {
        self.shared.emit(event);
    }

    /// Calls `taken` in the session's event order when the agent takes the next prompt.
    pub fn on_prompt_taken(&self, taken: PromptTaken) {
        self.shared.on_prompt_taken(taken);
    }

    /// Stops the agent (see `AgentProcess::stop`) and waits until `exited` was delivered.
    pub async fn stop(&self) {
        self.process.stop().await;
    }
}

/// Checks a `!command` (any engine): not blank, at most [`MAX_ACTION_TEXT_BYTES`] bytes.
pub(super) fn check_command(command: &str) -> Result<()> {
    if command.trim().is_empty() || command.len() > MAX_ACTION_TEXT_BYTES {
        return Err(BusinessError::InvalidInput(format!(
            "command must be 1 to {MAX_ACTION_TEXT_BYTES} bytes"
        )));
    }

    Ok(())
}
