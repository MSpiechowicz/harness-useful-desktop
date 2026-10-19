//! Session actions of an [`OpenCodeSession`]: new chat, resume, fork, clone, rename, compact,
//! export, statistics, the last reply and `!command`s are not available yet. Moving a chat to
//! another project is not possible: OpenCode keeps a session in the directory it was created in.

use std::path::Path;

use super::OpenCodeSession;
use crate::error::{BusinessError, Result};
use crate::model::{AgentAction, AgentReply, BashOutput};

impl OpenCodeSession {
    /// Runs a session action.
    pub(super) async fn session_op(&self, _action: AgentAction) -> Result<AgentReply> {
        Err(BusinessError::Unsupported(
            "this action is not available in OpenCode chats yet".to_owned(),
        ))
    }

    /// Refuses: OpenCode cannot move a session to another project.
    pub async fn move_project(&self, _project: &Path, _sessions_dir: &Path) -> Result<()> {
        Err(BusinessError::Unsupported(
            "OpenCode cannot move a chat to another project".to_owned(),
        ))
    }

    /// Runs `command` in the agent's shell; not available yet.
    pub async fn bash(&self, _command: &str) -> Result<BashOutput> {
        self.process.ensure_running()?;

        Err(BusinessError::Unsupported(
            "!commands in OpenCode chats are not available yet".to_owned(),
        ))
    }
}
