//! The chat engines behind one session type: [`AgentSession`] picks the engine for the harness's
//! [`ChatProtocol`] at start and forwards every call to it.
//!
//! - RPC ([`RpcSession`]): OMP and PI in `--mode rpc`.
//! - OpenCode server ([`OpenCodeSession`]): `opencode serve` (the manager does not offer it yet).
//! - OpenCode 2 server ([`OpenCodeV2Session`]): `opencode serve` 2.x and its `/api` routes.
//! - Claude Code stream-json ([`ClaudeSession`]): `claude` in stream-json mode with its control
//!   protocol.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::SessionOptions;
use super::claude::ClaudeSession;
use super::opencode::OpenCodeSession;
use super::opencode_v2::OpenCodeV2Session;
use super::process::StartAbort;
use super::session::RpcSession;
use super::sink::{EventSink, PromptTaken};
use crate::error::Result;
use crate::harness::ChatProtocol;
use crate::model::{AgentAction, AgentEvent, AgentReply, ApprovalMode, BashOutput, UiResponse};
use crate::process::{ExecSpec, ExitInfo, ProcessSpawner};

/// Everything needed to start a session.
pub struct SessionLaunch<'a> {
    /// Desktop session key.
    pub key: String,
    /// The command to run.
    pub spec: ExecSpec,
    /// Starts the process.
    pub spawner: &'a dyn ProcessSpawner,
    /// How the harness talks to the app (picks the engine).
    pub protocol: ChatProtocol,
    /// Canonical project directory.
    pub project: PathBuf,
    /// The project's session folder (for `switchSession`).
    pub sessions_dir: PathBuf,
    /// The resumed session id. The RPC command line already has it, an engine without one applies
    /// it after spawn (as the next three).
    pub resume: Option<&'a str>,
    /// The per-session approval mode, where the harness supports one.
    pub approval_override: Option<ApprovalMode>,
    /// A short-lived helper agent without a saved session.
    pub utility: bool,
    /// Receives the events.
    pub sink: Arc<dyn EventSink>,
    /// Timing knobs.
    pub options: &'a SessionOptions,
    /// Gives the startup up when it resolves first.
    pub abort: StartAbort<'a>,
}

/// A running agent session, whatever its engine (see the module docs).
pub enum AgentSession {
    /// The PI-family RPC engine.
    Rpc(RpcSession),
    /// The `opencode serve` engine.
    OpenCode(OpenCodeSession),
    /// The `opencode serve` 2.x engine.
    OpenCodeV2(OpenCodeV2Session),
    /// The Claude Code stream-json engine.
    Claude(ClaudeSession),
}

impl AgentSession {
    /// Spawns the agent with the engine of `launch.protocol` and completes startup.
    pub async fn start(launch: SessionLaunch<'_>) -> Result<Self> {
        match launch.protocol {
            ChatProtocol::Rpc(dialect) => RpcSession::start(launch, dialect).await.map(Self::Rpc),
            ChatProtocol::OpenCodeServer => OpenCodeSession::start(launch).await.map(Self::OpenCode),
            ChatProtocol::OpenCodeServerV2 => OpenCodeV2Session::start(launch).await.map(Self::OpenCodeV2),
            ChatProtocol::ClaudeStream => ClaudeSession::start(launch).await.map(Self::Claude),
        }
    }

    /// RPC protocol version from the handshake (OMP).
    pub fn protocol_version(&self) -> Option<u32> {
        match self {
            Self::Rpc(session) => session.protocol_version(),
            Self::OpenCode(session) => session.protocol_version(),
            Self::OpenCodeV2(session) => session.protocol_version(),
            Self::Claude(session) => session.protocol_version(),
        }
    }

    /// How the process ended, once it has.
    pub fn exit_info(&self) -> Option<ExitInfo> {
        match self {
            Self::Rpc(session) => session.exit_info(),
            Self::OpenCode(session) => session.exit_info(),
            Self::OpenCodeV2(session) => session.exit_info(),
            Self::Claude(session) => session.exit_info(),
        }
    }

    /// True until the process exited.
    pub fn is_running(&self) -> bool {
        match self {
            Self::Rpc(session) => session.is_running(),
            Self::OpenCode(session) => session.is_running(),
            Self::OpenCodeV2(session) => session.is_running(),
            Self::Claude(session) => session.is_running(),
        }
    }

    /// Runs `action` (validated first).
    pub async fn send(&self, action: AgentAction) -> Result<AgentReply> {
        match self {
            Self::Rpc(session) => session.send(action).await,
            Self::OpenCode(session) => session.send(action).await,
            Self::OpenCodeV2(session) => session.send(action).await,
            Self::Claude(session) => session.send(action).await,
        }
    }

    /// Moves the running session to `project` (whose session folder is `sessions_dir`).
    pub async fn move_project(&self, project: &Path, sessions_dir: &Path) -> Result<()> {
        match self {
            Self::Rpc(session) => session.move_project(project, sessions_dir).await,
            Self::OpenCode(session) => session.move_project(project, sessions_dir).await,
            Self::OpenCodeV2(session) => session.move_project(project, sessions_dir).await,
            Self::Claude(session) => session.move_project(project, sessions_dir).await,
        }
    }

    /// Answers an open dialog; fails for ids that are not open.
    pub async fn respond(&self, response: UiResponse) -> Result<()> {
        match self {
            Self::Rpc(session) => session.respond(response).await,
            Self::OpenCode(session) => session.respond(response).await,
            Self::OpenCodeV2(session) => session.respond(response).await,
            Self::Claude(session) => session.respond(response).await,
        }
    }

    /// Runs `command` in the agent's shell. The caller must have the user's explicit confirmation.
    pub async fn bash(&self, command: &str) -> Result<BashOutput> {
        match self {
            Self::Rpc(session) => session.bash(command).await,
            Self::OpenCode(session) => session.bash(command).await,
            Self::OpenCodeV2(session) => session.bash(command).await,
            Self::Claude(session) => session.bash(command).await,
        }
    }

    /// Puts `event` into the session's stream, after the events already in it.
    pub fn emit(&self, event: AgentEvent) {
        match self {
            Self::Rpc(session) => session.emit(event),
            Self::OpenCode(session) => session.emit(event),
            Self::OpenCodeV2(session) => session.emit(event),
            Self::Claude(session) => session.emit(event),
        }
    }

    /// Calls `taken` in the session's event order when the agent takes the next prompt.
    pub fn on_prompt_taken(&self, taken: PromptTaken) {
        match self {
            Self::Rpc(session) => session.on_prompt_taken(taken),
            Self::OpenCode(session) => session.on_prompt_taken(taken),
            Self::OpenCodeV2(session) => session.on_prompt_taken(taken),
            Self::Claude(session) => session.on_prompt_taken(taken),
        }
    }

    /// Stops the agent and waits until `exited` was delivered.
    pub async fn stop(&self) {
        match self {
            Self::Rpc(session) => session.stop().await,
            Self::OpenCode(session) => session.stop().await,
            Self::OpenCodeV2(session) => session.stop().await,
            Self::Claude(session) => session.stop().await,
        }
    }
}
