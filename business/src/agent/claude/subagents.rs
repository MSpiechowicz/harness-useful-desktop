//! Stopping a subagent of a [`ClaudeSession`]: `stop_task` for its task. Claude Code has no way to
//! write to a running subagent, so a message to one goes through the orchestrator (the webview
//! steers the chat) and `messageSubagent` stays unsupported.
//!
//! [`ClaudeSession`]: super::ClaudeSession

use super::ClaudeSession;
use crate::claude_stream::wire::outbound::HostRequest;
use crate::error::{BusinessError, Result};
use crate::model::AgentReply;
use crate::pi_family::rpc::pending::DEFAULT_TIMEOUT;

impl ClaudeSession {
    /// Stops subagent `id`. One that already finished is left alone, one the chat never saw is
    /// not found.
    pub(super) async fn stop_subagent(&self, id: &str) -> Result<AgentReply> {
        let run = self.run();
        let active = run.chat.lock().mapper.subagent_active(id);

        match active {
            None => Err(BusinessError::NotFound(format!("subagent {id}"))),
            Some(false) => Ok(AgentReply::Ack),
            Some(true) => {
                let request = HostRequest::StopTask { task_id: id.to_owned() };
                run.chat.control(&request, DEFAULT_TIMEOUT).await?;

                Ok(AgentReply::Ack)
            }
        }
    }
}
