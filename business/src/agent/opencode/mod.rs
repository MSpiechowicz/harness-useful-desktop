//! The OpenCode chat engine: `opencode serve` driven over loopback HTTP and its event stream.
//!
//! Not available yet: [`OpenCodeSession::start`] refuses every launch (and is not reached, since
//! the manager refuses harnesses without chat first), so no session of this engine ever exists.

use std::convert::Infallible;
use std::path::Path;

use super::engine::SessionLaunch;
use super::sink::PromptTaken;
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentEvent, AgentReply, BashOutput, UiResponse};
use crate::process::ExitInfo;

/// A running OpenCode session; cannot exist yet (see the module docs).
pub struct OpenCodeSession {
    never: Infallible,
}

impl OpenCodeSession {
    /// Refuses: chat with OpenCode is not available yet.
    pub async fn start(_launch: SessionLaunch<'_>) -> Result<Self> {
        Err(BusinessError::Unsupported(
            "chat with OpenCode is coming soon".to_owned(),
        ))
    }

    // The calls of an `AgentSession`, none reachable while no session can exist.

    /// OpenCode has no RPC handshake.
    pub fn protocol_version(&self) -> Option<u32> {
        match self.never {}
    }

    pub fn exit_info(&self) -> Option<ExitInfo> {
        match self.never {}
    }

    pub fn is_running(&self) -> bool {
        match self.never {}
    }

    pub async fn send(&self, _action: AgentAction) -> Result<AgentReply> {
        match self.never {}
    }

    pub async fn move_project(&self, _project: &Path, _sessions_dir: &Path) -> Result<()> {
        match self.never {}
    }

    pub async fn respond(&self, _response: UiResponse) -> Result<()> {
        match self.never {}
    }

    pub async fn bash(&self, _command: &str) -> Result<BashOutput> {
        match self.never {}
    }

    pub fn emit(&self, _event: AgentEvent) {
        match self.never {}
    }

    pub fn on_prompt_taken(&self, _taken: PromptTaken) {
        match self.never {}
    }

    pub async fn stop(&self) {
        match self.never {}
    }
}
