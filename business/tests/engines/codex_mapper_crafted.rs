//! The Codex chat's event mapping over crafted frames, for what the recordings do not show: a
//! failed turn (an unusable model, a missing login), retries, reasoning summaries, MCP calls,
//! unknown methods and items, a turn whose start was missed, an unfinished command, the
//! engine's follow-up queue and a turn that never began.

use harness_business::codex_rpc::mapper::{
    CodexMapper, Hint, Mapped, SIGN_IN_ERROR, StartKind, UNFINISHED_TOOL, events,
};
use harness_business::codex_rpc::wire::decode_value;
use harness_business::codex_rpc::wire::results::ThreadStatus;
use harness_business::model::{AgentEvent, ChatBlock, NoticeLevel, PromptStatus};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

use crate::codex_recorded::{PROJECT, shape};

const THREAD: &str = "thread-1";
const TURN: &str = "turn-1";

/// What the fact-2 recording run reported for the configured model.
const UNSUPPORTED: &str = "The 'gpt-6-astra' model is not supported when using Codex with a ChatGPT account.";

fn mapper() -> CodexMapper {
    CodexMapper::new(std::path::Path::new(PROJECT))
}

fn map(mapper: &mut CodexMapper, frames: &[Value]) -> Vec<Mapped> {
    frames
        .iter()
        .flat_map(|frame| mapper.on_frame(&decode_value(frame.clone()).expect("a frame")))
        .collect()
}

fn note(method: &str, params: Value) -> Value {
    json!({ "method": method, "params": params, "emittedAtMs": 1 })
}

fn turn(method: &str, status: &str, error: Value) -> Value {
    note(
        method,
        json!({ "threadId": THREAD, "turn": { "id": TURN, "items": [], "status": status, "error": error } }),
    )
}

fn item(method: &str, item: Value) -> Value {
    note(method, json!({ "item": item, "threadId": THREAD, "turnId": TURN }))
}

fn delta(method: &str, item_id: &str, delta: &str, summary_index: u32) -> Value {
    note(
        method,
        json!({ "threadId": THREAD, "turnId": TURN, "itemId": item_id, "delta": delta, "summaryIndex": summary_index }),
    )
}

fn error(message: &str, kind: Value, will_retry: bool) -> Value {
    note(
        "error",
        json!({
            "error": { "message": message, "codexErrorInfo": kind, "additionalDetails": null },
            "willRetry": will_retry, "threadId": THREAD, "turnId": TURN,
        }),
    )
}

fn failed(error: &str, kind: Value) -> Value {
    turn(
        "turn/completed",
        "failed",
        json!({ "message": error, "codexErrorInfo": kind }),
    )
}

fn last_result(events: &[AgentEvent]) -> Option<(PromptStatus, Option<String>, bool)> {
    events.iter().rev().find_map(|event| match event {
        AgentEvent::PromptResult {
            status,
            error,
            retryable,
            ..
        } => Some((*status, error.clone(), *retryable)),
        _ => None,
    })
}

#[test]
fn an_unusable_model_fails_the_turn_with_its_error() {
    let mut mapper = mapper();
    mapper.sent(StartKind::Prompt);

    let prompt = json!({ "type": "userMessage", "id": "user-1", "clientId": "prompt-1", "content": [{ "type": "text", "text": "hi" }] });
    let warning = "Model metadata for `gpt-6-astra` not found. Defaulting to fallback metadata.";
    let out = map(
        &mut mapper,
        &[
            turn("turn/started", "inProgress", Value::Null),
            item("item/started", prompt.clone()),
            item("item/completed", prompt),
            note("warning", json!({ "threadId": THREAD, "message": warning })),
            error(UNSUPPORTED, json!("badRequest"), false),
            note(
                "thread/status/changed",
                json!({ "threadId": THREAD, "status": { "type": "systemError" } }),
            ),
            failed(UNSUPPORTED, json!("badRequest")),
        ],
    );
    let events = events(&out);

    assert_eq!(
        events,
        [
            AgentEvent::AgentStart,
            AgentEvent::TurnStart,
            AgentEvent::Notice {
                level: NoticeLevel::Warning,
                message: warning.to_owned(),
            },
            AgentEvent::TurnEnd,
            AgentEvent::AgentEnd,
            AgentEvent::PromptResult {
                request_id: Some("prompt-1".to_owned()),
                status: PromptStatus::Error,
                error: Some(UNSUPPORTED.to_owned()),
                retryable: false,
            },
            AgentEvent::Settled { aborted: Some(false) },
        ]
    );
    assert!(out.contains(&Mapped::Hint(Hint::ThreadStatus(ThreadStatus::SystemError))));
    assert!(!mapper.is_working());
}

#[test]
fn a_missing_login_asks_to_sign_in_and_dropped_streams_may_pass_again() {
    let mut mapper = mapper();
    let unauthorized = map(
        &mut mapper,
        &[
            turn("turn/started", "inProgress", Value::Null),
            failed("401 Unauthorized", json!("unauthorized")),
        ],
    );
    assert_eq!(
        last_result(&events(&unauthorized)),
        Some((PromptStatus::Error, Some(SIGN_IN_ERROR.to_owned()), false))
    );

    let mut mapper = self::mapper();
    let dropped = json!({ "responseStreamDisconnected": { "httpStatusCode": 502 } });
    let out = map(
        &mut mapper,
        &[
            turn("turn/started", "inProgress", Value::Null),
            error("stream disconnected\ndetails", dropped.clone(), false),
            turn("turn/completed", "failed", Value::Null),
        ],
    );
    assert_eq!(
        last_result(&events(&out)),
        Some((PromptStatus::Error, Some("stream disconnected".to_owned()), true))
    );
}

#[test]
fn retries_count_up_and_end_when_the_model_answers() {
    let mut mapper = mapper();
    let message = json!({ "type": "agentMessage", "id": "msg-1", "text": "", "phase": null });
    let out = map(
        &mut mapper,
        &[
            turn("turn/started", "inProgress", Value::Null),
            error("Reconnecting... 1/5", json!("serverOverloaded"), true),
            error("Reconnecting... 2/5", json!("serverOverloaded"), true),
            item("item/started", message),
        ],
    );

    let retries: Vec<(String, Option<u32>)> = events(&out)
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Retry { phase, attempt, .. } => Some((phase, attempt)),
            _ => None,
        })
        .collect();
    assert_eq!(
        retries,
        [
            ("start".to_owned(), Some(1)),
            ("start".to_owned(), Some(2)),
            ("end".to_owned(), Some(2))
        ]
    );
}

#[test]
fn reasoning_with_a_summary_streams_as_thinking_and_empty_reasoning_shows_nothing() {
    let mut mapper = mapper();
    let reasoning = |summary: Value| json!({ "type": "reasoning", "id": "rs-1", "summary": summary, "content": [] });
    let out = map(
        &mut mapper,
        &[
            turn("turn/started", "inProgress", Value::Null),
            item("item/started", reasoning(json!([]))),
            delta("item/reasoning/summaryPartAdded", "rs-1", "", 0),
            delta("item/reasoning/summaryTextDelta", "rs-1", "Plan", 0),
            delta("item/reasoning/summaryPartAdded", "rs-1", "", 1),
            delta("item/reasoning/summaryTextDelta", "rs-1", "Check", 1),
            item("item/completed", reasoning(json!(["Plan", "Check"]))),
            item(
                "item/completed",
                json!({ "type": "reasoning", "id": "rs-2", "summary": [], "content": [] }),
            ),
        ],
    );
    let events = events(&out);

    let thinking: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ThinkingDelta { delta, .. } => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(thinking, ["Plan", "\n\n", "Check"]);

    let ended: Vec<Vec<ChatBlock>> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } => Some(message.blocks.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        ended,
        [vec![ChatBlock::Thinking {
            text: "Plan\n\nCheck".to_owned(),
            redacted: false,
        }]]
    );
}

#[test]
fn an_mcp_call_shows_its_text_content_or_its_error() {
    let mut mapper = mapper();
    let call = |status: &str, result: Value, error: Value| {
        json!({
            "type": "mcpToolCall", "id": "mcp-1", "server": "docs", "tool": "search", "status": status,
            "arguments": { "query": "serde" }, "result": result, "error": error, "durationMs": 3,
        })
    };
    let content = json!({ "content": [{ "type": "text", "text": "found" }, { "type": "image", "data": "…" }] });
    let out = map(
        &mut mapper,
        &[
            turn("turn/started", "inProgress", Value::Null),
            item("item/started", call("inProgress", Value::Null, Value::Null)),
            delta("item/mcpToolCall/progress", "mcp-1", "searching", 0),
            item("item/completed", call("completed", content, Value::Null)),
        ],
    );
    let events = events(&out);

    assert!(events.contains(&AgentEvent::ToolStart {
        tool_call_id: "mcp-1".to_owned(),
        name: "mcp__docs__search".to_owned(),
        args: json!({ "query": "serde" }),
        intent: None,
    }));
    let end = events.iter().find_map(|event| match event {
        AgentEvent::ToolEnd { is_error, result, .. } => Some((*is_error, result["content"][0]["text"].clone())),
        _ => None,
    });
    assert_eq!(end, Some((false, json!("found"))));
    assert_eq!(shape(&events)[2..], ["toolStart", "toolUpdate", "toolEnd"]);
}

#[test]
fn unknown_methods_and_items_are_ignored() {
    let mut mapper = mapper();
    let out = map(
        &mut mapper,
        &[
            turn("turn/started", "inProgress", Value::Null),
            note(
                "thread/realtime/started",
                json!({ "threadId": THREAD, "anything": [1, 2] }),
            ),
            item(
                "item/started",
                json!({ "type": "plan", "id": "plan-1", "text": "1. do it" }),
            ),
            item(
                "item/completed",
                json!({ "type": "collabAgentToolCall", "id": "agent-1", "tool": "spawnAgent" }),
            ),
            note(
                "turn/diff/updated",
                json!({ "threadId": THREAD, "turnId": TURN, "diff": "…" }),
            ),
            turn("turn/completed", "completed", Value::Null),
        ],
    );

    assert_eq!(
        shape(&events(&out)),
        [
            "agentStart",
            "turnStart",
            "turnEnd",
            "agentEnd",
            "promptResult",
            "settled"
        ]
    );
    assert_eq!(
        events(&mapper.on_line("not a frame")),
        [AgentEvent::FrameError {
            original_type: None,
            error: "The line is not valid JSON.".to_owned(),
        }]
    );
}

#[test]
fn a_missed_turn_start_begins_the_turn_and_late_frames_of_an_ended_turn_are_dropped() {
    let mut mapper = mapper();
    let message = json!({ "type": "agentMessage", "id": "msg-1", "text": "", "phase": "final_answer" });
    let command = json!({
        "type": "commandExecution", "id": "exec-1", "command": "sleep 9", "cwd": PROJECT, "status": "inProgress",
        "commandActions": [], "aggregatedOutput": null, "exitCode": null,
    });
    let out = map(
        &mut mapper,
        &[
            delta("item/agentMessage/delta", "msg-1", "Hel", 0),
            item("item/started", command),
            turn("turn/completed", "completed", Value::Null),
            delta("item/agentMessage/delta", "msg-1", "lo", 0),
            item("item/completed", message),
        ],
    );
    let events = events(&out);

    assert_eq!(
        shape(&events),
        [
            "agentStart",
            "turnStart",
            "messageStart",
            "textDelta",
            "toolStart",
            "toolEnd",
            "messageEnd",
            "turnEnd",
            "agentEnd",
            "promptResult",
            "settled"
        ]
    );
    let unfinished = events.iter().any(|event| {
        matches!(event, AgentEvent::ToolEnd { is_error: true, result, .. } if result["content"][0]["text"] == UNFINISHED_TOOL)
    });
    assert!(unfinished);
}

#[test]
fn waiting_follow_ups_hold_the_settle_unless_the_turn_was_aborted() {
    let mut mapper = mapper();
    let started = turn("turn/started", "inProgress", Value::Null);

    mapper.follow_ups_waiting(true);
    let out = map(
        &mut mapper,
        &[started.clone(), turn("turn/completed", "completed", Value::Null)],
    );
    assert_eq!(shape(&events(&out)).last().map(String::as_str), Some("promptResult"));

    assert_eq!(
        events(&mapper.follow_ups_waiting(false)),
        [AgentEvent::Settled { aborted: Some(false) }]
    );

    let mut mapper = self::mapper();
    mapper.follow_ups_waiting(true);
    let out = map(
        &mut mapper,
        &[started, turn("turn/completed", "interrupted", Value::Null)],
    );
    assert_eq!(events(&out).last(), Some(&AgentEvent::Settled { aborted: Some(true) }));
}

#[test]
fn a_turn_that_never_began_leaves_the_chat_idle() {
    let mut mapper = mapper();

    mapper.sent(StartKind::Prompt);
    assert!(mapper.is_working());
    mapper.abort_requested();

    assert_eq!(mapper.withdrawn(), []);
    assert!(!mapper.is_working());

    let out = map(
        &mut mapper,
        &[
            turn("turn/started", "inProgress", Value::Null),
            failed("boom", json!("other")),
        ],
    );
    assert_eq!(
        last_result(&events(&out)),
        Some((PromptStatus::Error, Some("boom".to_owned()), false)),
        "the stop asked for meanwhile is forgotten"
    );
}
