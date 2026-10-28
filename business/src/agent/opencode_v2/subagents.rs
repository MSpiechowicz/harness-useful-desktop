//! Stopping a subagent and messaging it, for an [`OpenCodeV2Session`]. A subagent is a session of
//! its own: stopping it interrupts that session (`POST /api/session/{id}/interrupt`, the chat's
//! turn goes on), a message to it is a prompt of that session that joins what it is doing
//! (`delivery: steer`). Only subagents the chat's `subagent` calls started count.

use super::OpenCodeV2Session;
use crate::error::{BusinessError, Result};
use crate::model::AgentReply;
use crate::opencode_server_v2::wire::{Delivery, PromptBody};
use crate::opencode_server_v2::{RouteV2, SessionId};

impl OpenCodeV2Session {
    /// Interrupts subagent `id`; nothing to do when it already finished.
    pub(super) async fn stop_subagent(&self, id: &str) -> Result<AgentReply> {
        let directory = self.live_subagent(id, false)?;
        let Some(directory) = directory else {
            return Ok(AgentReply::Ack);
        };

        let session = SessionId::parse(id)?;
        let route = RouteV2::Interrupt {
            session: &session,
            directory: &directory,
        };

        self.chat.client.send(route, None).await?.error_for_status()?;

        Ok(AgentReply::Ack)
    }

    /// Sends `message` to subagent `id`. A subagent that finished takes no message: the webview
    /// then sends it to the orchestrator.
    pub(super) async fn message_subagent(&self, id: &str, message: &str) -> Result<AgentReply> {
        let directory = self.live_subagent(id, true)?.unwrap_or_default();
        let session = SessionId::parse(id)?;
        let route = RouteV2::Prompt {
            session: &session,
            directory: &directory,
        };
        let body = PromptBody::text(message.to_owned()).with_delivery(Delivery::Steer);
        let body = serde_json::to_value(&body).unwrap_or_default();

        self.chat.client.send(route, Some(&body)).await?.error_for_status()?;

        Ok(AgentReply::Ack)
    }

    /// The directory of subagent `id` when it works. `None` when it finished: an error for a
    /// message (`finished_is_error`), nothing to do for a stop. An unknown id is `NotFound`.
    fn live_subagent(&self, id: &str, finished_is_error: bool) -> Result<Option<String>> {
        let live = self.chat.lock();

        // The chat's own session is never a subagent: stopping it would end the chat's turn.
        if live.session.as_str() == id {
            return Err(BusinessError::NotFound(format!("subagent {id}")));
        }

        match live.mapper.subagent_active(id) {
            None => Err(BusinessError::NotFound(format!("subagent {id}"))),
            Some(false) if finished_is_error => Err(BusinessError::Conflict("this subagent has finished".to_owned())),
            Some(false) => Ok(None),
            Some(true) => {
                let directory = live.mapper.subagent_directory(id).unwrap_or(&self.chat.directory);

                Ok(Some(directory.to_owned()))
            }
        }
    }
}
