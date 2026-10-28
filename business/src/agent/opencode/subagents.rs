//! Stopping a subagent and messaging it, for an [`OpenCodeSession`]. A subagent is a session of
//! its own: stopping it aborts that session (`POST /session/{id}/abort`, the chat's turn goes on),
//! a message to it is a prompt of that session (`prompt_async`, with the agent and model it
//! works with). Only subagents the chat's `task` calls started count.

use serde_json::json;

use super::{Live, OpenCodeSession};
use crate::error::{BusinessError, Result};
use crate::model::AgentReply;
use crate::opencode_server::mapper::SubagentSettings;
use crate::opencode_server::wire::{Outgoing, Settings, prompt_body};
use crate::opencode_server::{Route, SessionId};

impl OpenCodeSession {
    /// Aborts subagent `id`; nothing to do when it already finished.
    pub(super) async fn stop_subagent(&self, id: &str) -> Result<AgentReply> {
        let active = {
            let live = self.chat.lock();

            if own_session(&live, id) {
                return Err(unknown(id));
            }

            live.mapper.subagent_active(id)
        };

        match active {
            None => return Err(unknown(id)),
            Some(false) => return Ok(AgentReply::Ack),
            Some(true) => {}
        }

        let session = SessionId::parse(id)?;
        let empty = json!({});

        self.chat
            .client
            .send(Route::Abort { session: &session }, Some(&empty))
            .await?
            .error_for_status()?;

        Ok(AgentReply::Ack)
    }

    /// Sends `message` to subagent `id`. A subagent that finished takes no message: the webview
    /// then sends it to the orchestrator.
    pub(super) async fn message_subagent(&self, id: &str, message: &str) -> Result<AgentReply> {
        let (active, settings) = {
            let live = self.chat.lock();

            if own_session(&live, id) {
                return Err(unknown(id));
            }

            (live.mapper.subagent_active(id), live.mapper.subagent_settings(id))
        };

        match active {
            None => return Err(unknown(id)),
            Some(false) => return Err(BusinessError::Conflict("this subagent has finished".to_owned())),
            Some(true) => {}
        }

        // Only the agent the server reported counts: without it the message would run as the
        // server's default agent, whose permissions may differ.
        let SubagentSettings { agent, model } = settings.unwrap_or_default();
        let Some(agent) = agent else {
            return Err(BusinessError::Conflict(
                "this subagent has not reported its agent yet, try again in a moment".to_owned(),
            ));
        };

        let settings = Settings {
            agent: &agent,
            model: model.as_ref(),
            variant: None,
        };

        let body = prompt_body(&Outgoing::new(message, &[]), &settings);

        let session = SessionId::parse(id)?;

        self.chat
            .client
            .send(Route::PromptAsync { session: &session }, Some(&body))
            .await?
            .error_for_status()?;

        Ok(AgentReply::Ack)
    }
}

/// Whether `id` is the chat's own session, which no subagent action may touch.
fn own_session(live: &Live, id: &str) -> bool {
    live.session.as_str() == id
}

fn unknown(id: &str) -> BusinessError {
    BusinessError::NotFound(format!("subagent {id}"))
}
