//! OpenCode history keeps a tool call left running or pending in a finished turn as a failed
//! result (the one the live chat gives it), so a reloaded chat does not show it as "Not run". The
//! newest reply may still be running (another process), so it stays open.

use crate::opencode_session_support;

use harness_business::claude_stream::mapper::UNFINISHED_TOOL;
use harness_business::harness::{CliSessions, OpenCodeGeneration};
use harness_business::harnesses::opencode::OpenCodeSessions;
use harness_business::model::{ChatBlock, ChatMessage, MessageRole};
use harness_business::opencode_server_v2::mapper::settled_history;
use harness_business::opencode_server_v2::messages::SessionMessage;
use opencode_session_support::{Fixture, Replay, ok, opencode};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const ID: &str = "ses_2b7f3c9a1ffeA1b2C3d4E5f6G7";

/// `(text, is_error)` of the result of call `call`; `None` when it has no result.
fn result_of(messages: &[ChatMessage], call: &str) -> Option<(String, bool)> {
    let mut found = messages
        .iter()
        .filter(|message| message.role == MessageRole::ToolResult && message.tool_call_id.as_deref() == Some(call));
    let message = found.next()?;
    assert!(found.next().is_none(), "one result for {call}");

    let text = match &message.blocks[..] {
        [ChatBlock::Text { text }] => text.clone(),
        other => panic!("not one text block: {other:?}"),
    };

    Some((text, message.is_error == Some(true)))
}

fn failed() -> Option<(String, bool)> {
    Some((UNFINISHED_TOOL.to_owned(), true))
}

fn v1_tool(call: &str, status: &str) -> Value {
    let state = match status {
        "completed" => {
            json!({ "status": "completed", "input": {}, "output": "done", "time": { "start": 1, "end": 2 } })
        }
        "pending" => json!({ "status": "pending", "input": {}, "raw": "" }),
        _ => json!({ "status": status, "input": { "filePath": "/home/demo/src/lib.rs" }, "time": { "start": 1 } }),
    };

    json!({ "type": "tool", "id": format!("prt_{call}"), "callID": call, "tool": "read", "state": state })
}

fn v1_user(id: &str) -> Value {
    json!({
        "info": { "role": "user", "id": id, "time": { "created": 1 } },
        "parts": [{ "type": "text", "id": format!("prt_{id}"), "text": "Read the file" }]
    })
}

fn v1_reply(id: &str, completed: bool, parts: &[Value]) -> Value {
    let time = if completed {
        json!({ "created": 1, "completed": 2 })
    } else {
        json!({ "created": 1 })
    };

    json!({ "info": { "role": "assistant", "id": id, "time": time }, "parts": parts })
}

fn v1_transcript(messages: Vec<Value>) -> Vec<ChatMessage> {
    let fixture = Fixture::new();
    let export = json!({
        "info": { "id": ID, "directory": fixture.project().to_string_lossy() },
        "messages": messages
    });
    let runner = Replay::answering([ok(export.to_string())]);

    OpenCodeSessions::new()
        .messages(&fixture.ctx(fixture.project(), &runner), &opencode(ID), None, 200)
        .unwrap()
        .messages
}

#[test]
fn v1_calls_left_open_in_a_finished_turn_fail() {
    let messages = v1_transcript(vec![
        v1_user("msg_1"),
        v1_reply(
            "msg_2",
            true,
            &[
                v1_tool("done", "completed"),
                v1_tool("ran", "running"),
                v1_tool("waited", "pending"),
            ],
        ),
        v1_user("msg_3"),
        // Crashed: never completed, but a later reply exists.
        v1_reply("msg_4", false, &[v1_tool("crashed", "running")]),
        v1_user("msg_5"),
        // The newest reply may still run: queued prompts after it do not close it.
        v1_reply(
            "msg_6",
            false,
            &[v1_tool("live", "running"), v1_tool("queued", "pending")],
        ),
        v1_user("msg_7"),
    ]);

    assert_eq!(result_of(&messages, "done"), Some(("done".to_owned(), false)));
    assert_eq!(result_of(&messages, "ran"), failed());
    assert_eq!(result_of(&messages, "waited"), failed());
    assert_eq!(result_of(&messages, "crashed"), failed());
    assert_eq!(result_of(&messages, "live"), None);
    assert_eq!(result_of(&messages, "queued"), None);
}

#[test]
fn v1_a_newest_reply_that_completed_is_finished() {
    let messages = v1_transcript(vec![
        v1_user("msg_1"),
        v1_reply("msg_2", true, &[v1_tool("last", "running")]),
    ]);

    assert_eq!(result_of(&messages, "last"), failed());
}

fn v2_tool(call: &str, status: &str) -> Value {
    let state = match status {
        "completed" => json!({ "status": "completed", "input": {}, "content": [{ "type": "text", "text": "done" }] }),
        "streaming" => json!({ "status": "streaming", "input": "{\"pa" }),
        _ => json!({ "status": "running", "input": { "filePath": "/home/demo/src/lib.rs" } }),
    };

    json!({ "type": "tool", "id": call, "name": "read", "state": state, "time": { "created": 1 } })
}

fn v2_user(id: &str) -> Value {
    json!({ "type": "user", "id": id, "time": { "created": 1 }, "text": "Read the file" })
}

fn v2_reply(id: &str, completed: bool, content: &[Value]) -> Value {
    let time = if completed {
        json!({ "created": 1, "completed": 2 })
    } else {
        json!({ "created": 1 })
    };

    json!({ "type": "assistant", "id": id, "time": time, "content": content })
}

fn v2_idle(id: &str) -> Value {
    json!({ "type": "idle", "id": id, "time": { "created": 3 }, "outcome": "succeeded" })
}

fn v2_transcript(messages: &[Value]) -> Vec<ChatMessage> {
    let fixture = Fixture::new();
    let export = json!({
        "info": { "id": ID, "location": { "directory": fixture.project().to_string_lossy() } },
        "messages": messages
    });
    let runner = Replay::answering([ok(export.to_string())]);

    OpenCodeSessions::with_generation(OpenCodeGeneration::V2)
        .messages(&fixture.ctx(fixture.project(), &runner), &opencode(ID), None, 200)
        .unwrap()
        .messages
}

fn v2_session() -> Vec<Value> {
    vec![
        v2_user("msg_1"),
        v2_reply(
            "msg_2",
            true,
            &[
                v2_tool("done", "completed"),
                v2_tool("ran", "running"),
                v2_tool("streamed", "streaming"),
            ],
        ),
        v2_user("msg_3"),
        // Crashed: never completed, but a later reply exists.
        v2_reply("msg_4", false, &[v2_tool("crashed", "running")]),
        v2_user("msg_5"),
        // The newest reply may still run: queued prompts after it do not close it.
        v2_reply("msg_6", false, &[v2_tool("live", "running")]),
        v2_user("msg_7"),
    ]
}

#[test]
fn v2_calls_left_open_in_a_finished_turn_fail() {
    let messages = v2_transcript(&v2_session());

    assert_eq!(result_of(&messages, "done"), Some(("done".to_owned(), false)));
    assert_eq!(result_of(&messages, "ran"), failed());
    assert_eq!(result_of(&messages, "streamed"), failed());
    assert_eq!(result_of(&messages, "crashed"), failed());
    assert_eq!(result_of(&messages, "live"), None);
}

#[test]
fn v2_an_idle_marker_ends_the_turn_and_a_finished_session_settles_everything() {
    let mut session = v2_session();
    session.insert(6, v2_idle("msg_idle"));
    assert_eq!(result_of(&v2_transcript(&session), "live"), failed());

    let parsed: Vec<SessionMessage> = v2_session().iter().map(SessionMessage::parse).collect();

    let busy = settled_history(&parsed, true);
    assert_eq!(
        result_of(&busy, "live"),
        None,
        "a busy chat keeps its newest reply open"
    );
    assert_eq!(result_of(&busy, "ran"), failed());

    let idle = settled_history(&parsed, false);
    assert_eq!(
        result_of(&idle, "live"),
        failed(),
        "an idle chat has no turn left running"
    );
}
