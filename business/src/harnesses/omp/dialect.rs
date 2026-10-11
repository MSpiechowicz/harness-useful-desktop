//! The OMP RPC dialect (`modes/rpc/wire/rpc-wire.schema.json`, protocol v1).
//!
//! - Startup: wait for `{"type":"ready"}`, then `set_event_filter {events: null, messageUpdates:
//!   "delta"}` so `message_update` frames drop their accumulated snapshots.
//! - Prompts: acknowledged on admission; completion is a `prompt_result` with the request id. An
//!   acknowledgement with `agentInvoked: false` means OMP handled the prompt locally and no
//!   `prompt_result` follows, so it completes the prompt as `handled`.
//! - History: `get_messages_page` (oldest first, at most 256 per page); the session pages through
//!   it and serves the newest page first.
//! - `clone` does not exist in OMP.

use serde_json::json;

use super::catalog::OmpCatalog;
use super::subagents::{decode_subagent, subscription_frame};
use crate::pi_family::rpc::prelude::*;

/// Largest page OMP serves.
const HISTORY_PAGE: u32 = 256;

/// The OMP dialect.
#[derive(Debug, Clone, Copy, Default)]
pub struct OmpDialect;

impl RpcDialect for OmpDialect {
    fn harness(&self) -> HarnessId {
        HarnessId::Omp
    }

    fn catalog(&self) -> &'static dyn CommandCatalog {
        &OmpCatalog
    }

    fn handshake(&self) -> Handshake {
        Handshake::ReadyFrame
    }

    fn after_handshake(&self) -> Vec<Value> {
        vec![json!({ "type": "set_event_filter", "events": null, "messageUpdates": "delta" })]
    }

    fn move_prompt(&self, project: &str) -> Option<String> {
        // `/move <path>` takes the rest of the line as the path, spaces included.
        Some(format!("/move {project}"))
    }

    fn optional_after_handshake(&self) -> Vec<Value> {
        vec![subscription_frame()]
    }

    fn encode(&self, action: &AgentAction, id: &str, context: &EncodeContext<'_>) -> Result<Value> {
        match action {
            AgentAction::Clone => Err(BusinessError::Unsupported(
                "OMP cannot clone a session; fork it instead".to_owned(),
            )),
            AgentAction::RefreshCommands => {
                Ok(command_frame("get_available_commands", id, Vec::<(&str, Value)>::new()))
            }
            AgentAction::GetHistory { cursor, .. } => Ok(self.history_request(cursor.as_deref(), id)),
            _ => encode_shared(HarnessId::Omp, action, id, context)?
                .ok_or_else(|| BusinessError::Internal("action without an OMP command".to_owned())),
        }
    }

    fn history_request(&self, cursor: Option<&str>, id: &str) -> Value {
        command_frame(
            "get_messages_page",
            id,
            [
                ("cursor", cursor.map_or(Value::Null, Value::from)),
                ("limit", Value::from(HISTORY_PAGE)),
            ],
        )
    }

    fn history_paged(&self) -> bool {
        true
    }

    fn decode(&self, frame: Value, state: &mut DecodeState) -> Vec<Inbound> {
        if frame_type(&frame) == "ready" {
            let version = frame
                .get("protocolVersion")
                .and_then(Value::as_u64)
                .and_then(|version| u32::try_from(version).ok());

            return vec![Inbound::Ready(version)];
        }

        if let Some(subagent) = decode_subagent(&frame) {
            return vec![Inbound::Event(AgentEvent::Subagent {
                subagent: Box::new(subagent),
            })];
        }

        decode_common(&frame, state)
    }

    fn on_prompt_ack(&self, request_id: &str, data: Option<&Value>, _state: &mut DecodeState) -> Vec<AgentEvent> {
        let invoked = data.and_then(|data| data.get("agentInvoked")).and_then(Value::as_bool);

        if invoked != Some(false) {
            return Vec::new();
        }

        vec![AgentEvent::PromptResult {
            request_id: Some(request_id.to_owned()),
            status: PromptStatus::Handled,
            error: None,
            retryable: false,
        }]
    }
}
