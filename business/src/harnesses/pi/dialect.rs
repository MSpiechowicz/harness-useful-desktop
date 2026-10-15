//! The PI RPC dialect (PI docs `rpc.md`, `rpc-commands.md`, `json.md`).
//!
//! - Startup: PI sends no ready frame; the first `get_state` is the probe.
//! - Prompts: the acknowledgement's `disposition` is `started`, `queued` or `handled`. A handled
//!   prompt is complete at once; the others complete when `agent_settled` arrives (PI has no
//!   `prompt_result`), as `aborted`, `error` (the last assistant message failed) or `completed`.
//! - History: `get_messages` once; the session slices it.
//! - The command list is `get_commands`; `clone` exists.

use super::catalog::PiCatalog;
use crate::pi_family::rpc::prelude::*;

/// The PI dialect.
#[derive(Debug, Clone, Copy, Default)]
pub struct PiDialect;

fn bare(kind: &str, id: &str) -> Value {
    command_frame(kind, id, Vec::<(&str, Value)>::new())
}

impl RpcDialect for PiDialect {
    fn harness(&self) -> HarnessId {
        HarnessId::Pi
    }

    fn catalog(&self) -> &'static dyn CommandCatalog {
        &PiCatalog
    }

    fn handshake(&self) -> Handshake {
        Handshake::Probe
    }

    fn encode(&self, action: &AgentAction, id: &str, context: &EncodeContext<'_>) -> Result<Value> {
        match action {
            AgentAction::Clone => Ok(bare("clone", id)),
            AgentAction::RefreshCommands => Ok(bare("get_commands", id)),
            AgentAction::GetHistory { .. } => Ok(self.history_request(None, id)),
            AgentAction::RemoveQueued { .. } => Err(BusinessError::Unsupported(
                "PI cannot withdraw a queued message".to_owned(),
            )),
            _ => encode_shared(HarnessId::Pi, action, id, context)?
                .ok_or_else(|| BusinessError::Internal("action without a PI command".to_owned())),
        }
    }

    fn history_request(&self, _cursor: Option<&str>, id: &str) -> Value {
        bare("get_messages", id)
    }

    fn history_paged(&self) -> bool {
        false
    }

    /// `agent_settled` reports the waiting prompts first, then `settled` (OMP's order).
    fn decode(&self, frame: Value, state: &mut DecodeState) -> Vec<Inbound> {
        if frame_type(&frame) != "agent_settled" {
            return decode_common(&frame, state);
        }

        let aborted = frame.get("aborted").and_then(Value::as_bool).unwrap_or(false);
        let (prompts, error) = state.settle_prompts();

        let mut inbound: Vec<Inbound> = prompts
            .into_iter()
            .map(|id| Inbound::Event(prompt_result(id, aborted, error.clone())))
            .collect();
        inbound.extend(decode_common(&frame, state));

        inbound
    }

    fn on_prompt_ack(&self, request_id: &str, data: Option<&Value>, state: &mut DecodeState) -> Vec<AgentEvent> {
        let disposition = data.and_then(|data| data.get("disposition")).and_then(Value::as_str);

        if disposition == Some("handled") {
            return vec![AgentEvent::PromptResult {
                request_id: Some(request_id.to_owned()),
                status: PromptStatus::Handled,
                error: None,
                retryable: false,
            }];
        }

        state.track_prompt(request_id);
        Vec::new()
    }
}

fn prompt_result(request_id: String, aborted: bool, error: Option<String>) -> AgentEvent {
    let status = match (aborted, &error) {
        (true, _) => PromptStatus::Aborted,
        (false, Some(_)) => PromptStatus::Error,
        (false, None) => PromptStatus::Completed,
    };

    AgentEvent::PromptResult {
        request_id: Some(request_id),
        status,
        error: if status == PromptStatus::Error { error } else { None },
        retryable: false,
    }
}
