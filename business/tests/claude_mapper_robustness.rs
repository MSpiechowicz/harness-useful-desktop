//! The Claude chat's event mapping with frames that are odd or hostile: numbers sent as floats,
//! session ids that are not ids, a withdrawn `/compact`, a result that names its prompts only in
//! the list, a context window that is no longer known, and replies and tool inputs that grow past
//! the streaming bound.

mod claude_frames;

use std::path::Path;

use claude_frames::{hints, requesting};
use harness_business::claude_stream::mapper::{
    ClaudeMapper, Hint, MAX_STREAMED_BYTES, Mapped, REPLY_CUT, SendKind, TOOL_INPUT_CUT, events,
};
use harness_business::claude_stream::stats::ClaudeStats;
use harness_business::claude_stream::wire::{Inbound, decode_value};
use harness_business::model::{AgentEvent, ChatBlock, PromptStatus};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

const SESSION: &str = "c1262da3-a66b-42b5-a9ee-32bb98f2045d";

fn mapper() -> ClaudeMapper {
    ClaudeMapper::new(Path::new("/work/project"))
}

/// Maps `frames` as the reader does: every line decoded first.
fn feed(mapper: &mut ClaudeMapper, frames: &[Value]) -> Vec<Mapped> {
    frames
        .iter()
        .flat_map(|frame| mapper.on_line(&frame.to_string()))
        .collect()
}

fn lifecycle(uuid: &str, state: &str) -> Value {
    json!({ "type": "command_lifecycle", "command_uuid": uuid, "state": state })
}

fn stream(event: Value) -> Value {
    json!({ "type": "stream_event", "api_message_id": "msg_1", "event": event })
}

fn result_frame(value: Value) -> harness_business::claude_stream::wire::ResultFrame {
    match decode_value(value).expect("a result") {
        Inbound::Result(result) => *result,
        other => panic!("not a result: {other:?}"),
    }
}

#[test]
fn a_result_and_a_retry_with_float_numbers_still_reach_the_chat() {
    let mut mapper = mapper();
    let out = feed(
        &mut mapper,
        &[
            requesting("u1"),
            json!({ "type": "system", "subtype": "api_retry", "attempt": 2.0, "max_retries": 10,
                "retry_delay_ms": 1532.7, "error_status": 529.0, "error": "overloaded" }),
            json!({ "type": "result", "subtype": "success", "is_error": false, "num_turns": 1.0,
                "duration_ms": 2318.4, "duration_api_ms": 1162.9, "total_cost_usd": 0.25,
                "usage": { "input_tokens": 2.0, "output_tokens": 8, "iterations": [] },
                "modelUsage": { "m": { "inputTokens": 2.0, "contextWindow": 1000000.0, "costUSD": 0.25 } },
                "user_message_uuid": "u1", "user_message_uuids": ["u1"] }),
        ],
    );
    let events = events(&out);

    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::FrameError { .. })),
        "{events:#?}"
    );
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::Retry { phase, attempt: Some(2), message: Some(message), .. }
            if phase == "start" && message == "overloaded (529)"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::PromptResult {
            status: PromptStatus::Completed,
            ..
        }
    )));

    let ended = hints(&out).into_iter().find_map(|hint| match hint {
        Hint::TurnEnded(end) => Some(end.result),
        _ => None,
    });
    let ended = ended.expect("the turn ended");
    assert_eq!((ended.duration_ms, ended.num_turns), (Some(2318), 1));
    assert_eq!(ended.model_usage["m"].context_window, Some(1_000_000));
}

#[test]
fn a_session_id_that_is_not_an_id_is_ignored() {
    let mut mapper = mapper();
    let out = feed(
        &mut mapper,
        &[
            json!({ "type": "system", "subtype": "session_state_changed", "state": "idle",
                "session_id": "../../../etc/passwd" }),
            json!({ "type": "system", "subtype": "session_state_changed", "state": "idle",
                "session_id": SESSION.to_uppercase() }),
            json!({ "type": "system", "subtype": "session_state_changed", "state": "idle", "session_id": SESSION }),
        ],
    );

    let named: Vec<Hint> = hints(&out)
        .into_iter()
        .filter(|hint| matches!(hint, Hint::SessionId(_)))
        .collect();
    assert_eq!(named, [Hint::SessionId(SESSION.to_owned())]);
    assert_eq!(mapper.session_id(), Some(SESSION));
}

#[test]
fn a_withdrawn_or_cancelled_compact_never_labels_a_later_compaction_manual() {
    let compaction = |phase: &str, reason: Option<&str>| AgentEvent::Compaction {
        phase: phase.to_owned(),
        reason: reason.map(str::to_owned),
        error: None,
    };
    let auto_compaction = [
        requesting("u1"),
        json!({ "type": "system", "subtype": "status", "status": "compacting" }),
        json!({ "type": "system", "subtype": "compact_boundary",
            "compact_metadata": { "trigger": "auto", "pre_tokens": 900000, "post_tokens": 20000 } }),
    ];

    let mut withdrawn = mapper();
    withdrawn.sent("c1", SendKind::Compact);
    withdrawn.withdrawn("c1");
    withdrawn.sent("u1", SendKind::Prompt);
    let out = feed(&mut withdrawn, &auto_compaction);
    assert_eq!(
        events(&out)[1..],
        [compaction("start", None), compaction("end", Some("auto"))]
    );

    let mut cancelled = mapper();
    cancelled.sent("c1", SendKind::Compact);
    feed(
        &mut cancelled,
        &[lifecycle("c1", "queued"), lifecycle("c1", "cancelled")],
    );
    cancelled.sent("u1", SendKind::Prompt);
    let out = feed(&mut cancelled, &auto_compaction);
    assert_eq!(
        events(&out)[1..],
        [compaction("start", None), compaction("end", Some("auto"))]
    );
}

#[test]
fn a_result_that_lists_its_prompts_only_names_the_first() {
    let mut mapper = mapper();
    let out = feed(
        &mut mapper,
        &[
            requesting("u1"),
            json!({ "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
                "user_message_uuids": ["u1", "u2"] }),
        ],
    );

    let request = events(&out).into_iter().find_map(|event| match event {
        AgentEvent::PromptResult { request_id, .. } => Some(request_id),
        _ => None,
    });
    assert_eq!(request, Some(Some("u1".to_owned())));
}

#[test]
fn the_context_meter_hides_once_the_window_of_the_model_is_unknown() {
    let turn = |models: Value| {
        result_frame(json!({ "type": "result", "subtype": "success", "num_turns": 1,
            "usage": { "input_tokens": 1, "iterations": [{ "input_tokens": 100 }] },
            "modelUsage": models }))
    };

    let mut stats = ClaudeStats::default();
    stats.on_result(&turn(json!({ "m1": { "contextWindow": 1000 } })), Some("m1"));
    assert_eq!(stats.context().map(|context| context.tokens), Some(100));

    // The session moved to m2, whose window the result does not name.
    stats.on_result(
        &turn(json!({ "m1": { "contextWindow": 1000 }, "m2": { "inputTokens": 5 } })),
        Some("m2"),
    );
    assert_eq!(stats.context(), None, "no stale meter of m1");
}

#[test]
fn a_reply_stops_growing_at_the_bound_with_a_note() {
    let mut mapper = mapper();
    let chunk = "x".repeat(1024 * 1024);
    let mut frames = vec![
        requesting("u1"),
        stream(json!({ "type": "message_start", "message": { "id": "msg_1" } })),
        stream(json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } })),
    ];
    for _ in 0..10 {
        frames.push(stream(
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": chunk } }),
        ));
    }
    frames.push(stream(json!({ "type": "message_stop" })));

    let out = feed(&mut mapper, &frames);
    let events = events(&out);

    let streamed: String = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta { delta, .. } => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(streamed.len(), MAX_STREAMED_BYTES + REPLY_CUT.len());
    assert!(streamed.ends_with(REPLY_CUT));

    let ended = events.iter().find_map(|event| match event {
        AgentEvent::MessageEnd { message, .. } => Some(message.blocks.clone()),
        _ => None,
    });
    assert!(matches!(
        &ended.expect("the reply ended")[..],
        [ChatBlock::Text { text }] if text == &streamed
    ));
}

#[test]
fn a_tool_input_that_streams_past_the_bound_shows_a_note() {
    let mut mapper = mapper();
    let chunk = "y".repeat(1024 * 1024);
    let mut frames = vec![
        requesting("u1"),
        stream(json!({ "type": "message_start", "message": { "id": "msg_1" } })),
        stream(json!({ "type": "content_block_start", "index": 0,
            "content_block": { "type": "tool_use", "id": "toolu_1", "name": "Write", "input": {} } })),
        stream(json!({ "type": "content_block_delta", "index": 0,
            "delta": { "type": "input_json_delta", "partial_json": "{\"content\":\"" } })),
    ];
    for _ in 0..10 {
        frames.push(stream(
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "input_json_delta", "partial_json": chunk } }),
        ));
    }
    frames.push(stream(json!({ "type": "content_block_delta", "index": 0,
        "delta": { "type": "input_json_delta", "partial_json": "\"}" } })));
    frames.push(stream(json!({ "type": "content_block_stop", "index": 0 })));

    let out = feed(&mut mapper, &frames);
    let args = events(&out).into_iter().find_map(|event| match event {
        AgentEvent::ToolStart { args, .. } => Some(args),
        _ => None,
    });
    assert_eq!(args, Some(json!({ "note": TOOL_INPUT_CUT })));
}
