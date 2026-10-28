//! Crafted events of OpenCode 1 subagent sessions: the chat's `task` calls and what their child sessions
//! say.

use serde_json::{Value, json};

use crate::opencode_mapper_support::{SESSION, part};

pub fn created(id: &str, parent: &str) -> Value {
    json!({ "type": "session.created", "properties": { "sessionID": id,
        "info": { "id": id, "parentID": parent, "directory": "/work/project" } } })
}

/// The chat's `task` call `call` in state `state` (merged over its input and `running` status).
pub fn task(call: &str, description: &str, agent: &str, state: Value) -> Value {
    let mut full = json!({ "status": "running", "time": { "start": 10 },
        "input": { "description": description, "prompt": format!("Brief of {description}"), "subagent_type": agent } });

    for (key, value) in state.as_object().cloned().unwrap_or_default() {
        full[key] = value;
    }

    part(
        SESSION,
        json!({ "id": format!("prt_{call}"), "messageID": "msg_parent", "type": "tool", "tool": "task",
            "callID": call, "state": full }),
    )
}

pub fn linked(session: &str) -> Value {
    json!({ "metadata": { "sessionId": session } })
}

/// A reply of subagent `session`.
pub fn reply(session: &str, id: &str, tokens: Value, cost: f64) -> Value {
    json!({ "type": "message.updated", "properties": { "sessionID": session, "info": {
        "id": id, "role": "assistant", "agent": "general", "providerID": "mock", "modelID": "m1",
        "cost": cost, "tokens": tokens, "time": { "created": 1 } } } })
}

pub fn tool(session: &str, call: &str, name: &str, state: Value) -> Value {
    part(
        session,
        json!({ "id": format!("prt_{call}"), "messageID": "msg_c1", "type": "tool", "tool": name,
            "callID": call, "state": state }),
    )
}

pub fn prose(session: &str, id: &str, message: &str, text: &str, ended: bool) -> Value {
    let time = if ended {
        json!({ "start": 13, "end": 14 })
    } else {
        json!({ "start": 13 })
    };

    part(
        session,
        json!({ "id": id, "messageID": message, "type": "text", "text": text, "time": time }),
    )
}

pub fn child_status(session: &str, kind: &str) -> Value {
    json!({ "type": "session.status", "properties": { "sessionID": session, "status": { "type": kind } } })
}

pub fn aborted(session: &str) -> Value {
    json!({ "type": "session.error", "properties": { "sessionID": session,
        "error": { "name": "MessageAbortedError", "data": { "message": "Aborted" } } } })
}
