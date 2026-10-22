//! What the app writes besides its requests: the `initialized` notification, and the answer to
//! each request of Codex under the request's id (approval decisions, granted permissions, the
//! answers to questions and elicitations, and errors for requests the app refuses).

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::RequestId;
use super::requests::ServerRequest;

/// The JSON-RPC code of a request the app does not handle.
pub const METHOD_NOT_FOUND: i64 = -32601;

/// The notification that ends the handshake, after `initialize`'s result.
pub fn initialized() -> Value {
    json!({ "method": "initialized" })
}

/// An approval decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Once.
    Accept,
    /// For the rest of the session.
    AcceptForSession,
    /// No; the turn goes on without it.
    Decline,
    /// No, and the turn stops.
    Cancel,
}

impl Decision {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::AcceptForSession => "acceptForSession",
            Self::Decline => "decline",
            Self::Cancel => "cancel",
        }
    }
}

/// The answer to `item/commandExecution/requestApproval` `id`.
pub fn command_decision(id: &RequestId, decision: Decision) -> Value {
    result(id, json!({ "decision": decision.wire() }))
}

/// Accepts the command of approval `id` and adds `amendment` (a command prefix) to the user's
/// rules, so commands starting with it no longer ask.
pub fn command_amendment(id: &RequestId, amendment: &[String]) -> Value {
    let decision = json!({ "acceptWithExecpolicyAmendment": { "execpolicy_amendment": amendment } });

    result(id, json!({ "decision": decision }))
}

/// The answer to `item/fileChange/requestApproval` `id`.
pub fn file_change_decision(id: &RequestId, decision: Decision) -> Value {
    command_decision(id, decision)
}

/// How long granted permissions hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantScope {
    Turn,
    Session,
}

/// Grants what `item/permissions/requestApproval` `id` asked for (`requested`, its `permissions`).
pub fn permissions_grant(id: &RequestId, requested: &Value, scope: GrantScope) -> Value {
    let granted: Map<String, Value> = requested
        .as_object()
        .map(|profile| {
            profile
                .iter()
                .filter(|(_, value)| !value.is_null())
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();

    let scope = match scope {
        GrantScope::Turn => "turn",
        GrantScope::Session => "session",
    };

    result(id, json!({ "permissions": granted, "scope": scope }))
}

/// Grants nothing of what `item/permissions/requestApproval` `id` asked for.
pub fn permissions_deny(id: &RequestId) -> Value {
    result(id, json!({ "permissions": {}, "scope": "turn" }))
}

/// The answers to `item/tool/requestUserInput` `id`: per question id, the chosen labels (or the
/// typed text).
pub fn user_input_answers(id: &RequestId, answers: &BTreeMap<String, Vec<String>>) -> Value {
    let answers: Map<String, Value> = answers
        .iter()
        .map(|(question, chosen)| (question.clone(), json!({ "answers": chosen })))
        .collect();

    result(id, json!({ "answers": answers }))
}

/// What the user did with an elicitation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElicitationAction {
    Accept,
    Decline,
    Cancel,
}

/// The answer to `mcpServer/elicitation/request` `id`: `content` holds the form's values when
/// accepted.
pub fn elicitation_answer(id: &RequestId, action: ElicitationAction, content: Option<Value>) -> Value {
    let action = match action {
        ElicitationAction::Accept => "accept",
        ElicitationAction::Decline => "decline",
        ElicitationAction::Cancel => "cancel",
    };

    result(id, json!({ "action": action, "content": content, "_meta": null }))
}

/// Refuses `request` (a dynamic tool call, a token refresh, a method the app does not handle).
pub fn refusal(request: &ServerRequest) -> Value {
    let message = format!("the desktop app does not handle {} requests", request.method);

    error(&request.id, METHOD_NOT_FOUND, &message)
}

/// An error answer to request `id`.
pub fn error(id: &RequestId, code: i64, message: &str) -> Value {
    json!({ "id": id, "error": { "code": code, "message": message } })
}

fn result(id: &RequestId, result: Value) -> Value {
    json!({ "id": id, "result": result })
}
