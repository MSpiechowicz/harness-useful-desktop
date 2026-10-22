//! The Claude chat's event mapping for frames the recordings do not hold: subagents, retries,
//! startup failures, withdrawn follow-ups, replies without partial messages, unfinished tool
//! calls, dialogs, API errors and automatic or failed compactions.

use crate::claude_frames;

use std::path::Path;

use claude_frames::{hints, requesting};
use harness_business::claude_stream::mapper::{Ask, ClaudeMapper, Hint, Mapped, SendKind, UNFINISHED_TOOL, events};
use harness_business::claude_stream::wire::decode_value;
use harness_business::model::{AgentEvent, ChatBlock, PromptStatus, SubagentStatus};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn mapper() -> ClaudeMapper {
    ClaudeMapper::new(Path::new("/work/project"))
}

fn feed(mapper: &mut ClaudeMapper, frames: &[Value]) -> Vec<Mapped> {
    frames
        .iter()
        .flat_map(|frame| mapper.on_frame(&decode_value(frame.clone()).unwrap()))
        .collect()
}

fn kinds(mapped: &[Mapped]) -> Vec<String> {
    events(mapped)
        .iter()
        .map(|event| {
            serde_json::to_value(event).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

fn text_reply(id: &str, text: &str) -> Value {
    json!({ "type": "assistant", "uuid": format!("entry-{id}"), "parent_tool_use_id": null,
        "message": { "id": id, "model": "claude-opus-5-5", "content": [{ "type": "text", "text": text }] } })
}

fn result(uuid: &str, extra: Value) -> Value {
    let mut frame = json!({ "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
        "result": "", "user_message_uuid": uuid, "user_message_uuids": [uuid] });
    frame
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    frame
}

#[test]
fn frames_of_a_subagent_are_not_shown() {
    let mut mapper = mapper();
    let parent = json!("toolu_task");
    let out = feed(
        &mut mapper,
        &[
            json!({ "type": "stream_event", "parent_tool_use_id": parent,
                "event": { "type": "message_start", "message": { "id": "msg_sub" } } }),
            json!({ "type": "assistant", "parent_tool_use_id": parent,
                "message": { "id": "msg_sub", "content": [{ "type": "text", "text": "inside" }] } }),
            json!({ "type": "user", "parent_tool_use_id": parent,
                "message": { "content": [{ "type": "tool_result", "tool_use_id": "toolu_x", "content": "x" }] } }),
        ],
    );

    assert!(events(&out).is_empty());
    assert!(!mapper.is_working());
}

#[test]
fn task_subagents_show_as_subagents_and_other_tasks_do_not() {
    let mut mapper = mapper();
    let out = feed(
        &mut mapper,
        &[
            json!({ "type": "system", "subtype": "task_started", "task_id": "t1", "tool_use_id": "toolu_task",
                "description": "Find the parser", "subagent_type": "Explore", "task_type": "local_agent",
                "prompt": "Look for the parser" }),
            json!({ "type": "system", "subtype": "task_progress", "task_id": "t1", "description": "Find the parser",
                "usage": { "total_tokens": 900, "tool_uses": 3, "duration_ms": 4000 }, "last_tool_name": "Grep" }),
            json!({ "type": "system", "subtype": "task_notification", "task_id": "t1", "status": "completed",
                "summary": "Found it", "output_file": "" }),
            json!({ "type": "system", "subtype": "task_started", "task_id": "b1", "task_type": "local_bash",
                "description": "sleep" }),
        ],
    );

    let subagents: Vec<_> = events(&out)
        .into_iter()
        .map(|event| match event {
            AgentEvent::Subagent { subagent } => *subagent,
            other => panic!("{other:?}"),
        })
        .collect();

    assert_eq!(subagents.len(), 3);
    assert_eq!(subagents[0].agent, "Explore");
    assert_eq!(subagents[0].parent_tool_call_id.as_deref(), Some("toolu_task"));
    assert_eq!(subagents[0].task.as_deref(), Some("Look for the parser"));
    assert_eq!(subagents[1].current_tool.as_deref(), Some("Grep"));
    assert_eq!((subagents[1].tool_count, subagents[1].tokens), (Some(3), Some(900)));
    assert_eq!(subagents[2].status, SubagentStatus::Completed);
    assert_eq!(subagents[2].intent.as_deref(), Some("Found it"));
}

#[test]
fn a_retried_model_call_shows_a_retry_until_the_next_step() {
    let mut mapper = mapper();
    let out = feed(
        &mut mapper,
        &[
            requesting("u1"),
            json!({ "type": "system", "subtype": "api_retry", "attempt": 1, "max_retries": 10,
                "retry_delay_ms": 500, "error_status": 529, "error": "overloaded" }),
            json!({ "type": "stream_event", "event": { "type": "message_start", "message": { "id": "msg_1" } } }),
        ],
    );

    let retries: Vec<AgentEvent> = events(&out)
        .into_iter()
        .filter(|event| matches!(event, AgentEvent::Retry { .. }))
        .collect();
    assert_eq!(
        retries,
        [
            AgentEvent::Retry {
                phase: "start".to_owned(),
                attempt: Some(1),
                max_attempts: Some(10),
                message: Some("overloaded (529)".to_owned()),
            },
            AgentEvent::Retry {
                phase: "end".to_owned(),
                attempt: Some(1),
                max_attempts: None,
                message: None,
            },
        ]
    );
}

#[test]
fn a_failure_before_any_turn_fails_the_prompt_without_a_turn() {
    let mut mapper = mapper();
    let out = feed(
        &mut mapper,
        &[
            json!({ "type": "result", "subtype": "error_during_execution", "is_error": true, "num_turns": 0,
            "errors": ["No message found with message.uuid of: 1234"] }),
        ],
    );

    assert_eq!(kinds(&out), ["promptResult"]);
    assert!(matches!(
        &events(&out)[0],
        AgentEvent::PromptResult { status: PromptStatus::Error, error: Some(error), .. }
            if error == "No message found with message.uuid of: 1234"
    ));
}

#[test]
fn a_stop_before_the_turn_starts_aborts_the_prompt() {
    let mut mapper = mapper();
    mapper.abort_requested();
    mapper.sent("u1", SendKind::Prompt);
    mapper.abort_requested();

    let out = feed(
        &mut mapper,
        &[result(
            "u1",
            json!({ "subtype": "error_during_execution", "is_error": true, "num_turns": 0,
                "errors": ["[ede_diagnostic] result_type=user"] }),
        )],
    );

    assert_eq!(
        events(&out),
        [AgentEvent::PromptResult {
            request_id: Some("u1".to_owned()),
            status: PromptStatus::Aborted,
            error: None,
            retryable: false,
        }]
    );
    assert!(!mapper.is_working());
}

#[test]
fn a_turn_ending_while_a_follow_up_waits_settles_once_it_is_withdrawn() {
    let mut mapper = mapper();
    mapper.sent("u1", SendKind::Prompt);
    mapper.sent("u2", SendKind::FollowUp);

    let out = feed(
        &mut mapper,
        &[requesting("u1"), text_reply("msg_1", "done"), result("u1", json!({}))],
    );
    assert_eq!(kinds(&out).last().map(String::as_str), Some("promptResult"));
    assert!(mapper.is_working());

    let withdrawn = mapper.withdrawn("u2");
    assert_eq!(events(&withdrawn), [AgentEvent::Settled { aborted: Some(false) }]);
    assert!(!mapper.is_working());
}

#[test]
fn a_reply_without_partial_messages_shows_from_its_assistant_frames() {
    let mut mapper = mapper();
    let out = feed(
        &mut mapper,
        &[requesting("u1"), text_reply("msg_1", "whole"), result("u1", json!({}))],
    );

    assert_eq!(
        kinds(&out),
        [
            "agentStart",
            "turnStart",
            "messageStart",
            "textDelta",
            "messageEnd",
            "turnEnd",
            "agentEnd",
            "promptResult",
            "settled"
        ]
    );

    let ended = events(&out).into_iter().find_map(|event| match event {
        AgentEvent::MessageEnd { message, .. } => Some(message),
        _ => None,
    });
    let ended = ended.unwrap();
    assert_eq!(
        ended.blocks,
        [ChatBlock::Text {
            text: "whole".to_owned()
        }]
    );
    assert_eq!(ended.id.as_deref(), Some("entry-msg_1"));
}

#[test]
fn a_tool_call_still_running_when_the_turn_ends_fails_as_unfinished() {
    let mut mapper = mapper();
    let out = feed(
        &mut mapper,
        &[
            requesting("u1"),
            json!({ "type": "assistant", "message": { "id": "msg_1", "content": [
                { "type": "tool_use", "id": "toolu_1", "name": "Write", "input": { "file_path": "/work/project/a.txt" } }
            ] } }),
            result(
                "u1",
                json!({ "subtype": "error_during_execution", "is_error": true, "terminal_reason": "aborted_tools" }),
            ),
        ],
    );

    let end = events(&out).into_iter().find_map(|event| match event {
        AgentEvent::ToolEnd {
            is_error,
            result,
            touched_paths,
            ..
        } => Some((is_error, result["content"][0]["text"].clone(), touched_paths)),
        _ => None,
    });
    assert_eq!(end, Some((true, json!(UNFINISHED_TOOL), Vec::new())));
    assert!(events(&out).contains(&AgentEvent::Settled { aborted: Some(true) }));
}

#[test]
fn dialogs_elicitations_unknown_requests_and_cancels_are_hints() {
    let mut mapper = mapper();
    let out = feed(
        &mut mapper,
        &[
            json!({ "type": "control_request", "request_id": "d1", "request": { "subtype": "request_user_dialog",
                "dialog_kind": "plan_review", "payload": { "plan": "x" } } }),
            json!({ "type": "control_request", "request_id": "e1", "request": { "subtype": "elicitation",
                "mcp_server_name": "docs", "message": "Pick a space", "mode": "form" } }),
            json!({ "type": "control_request", "request_id": "m1", "request": { "subtype": "mcp_message" } }),
            json!({ "type": "control_cancel_request", "request_id": "d1" }),
        ],
    );

    let hints = hints(&out);
    assert!(
        matches!(&hints[0], Hint::Ask(Ask::Dialog { request_id, dialog }) if request_id == "d1" && dialog.dialog_kind == "plan_review")
    );
    assert!(
        matches!(&hints[1], Hint::Ask(Ask::Elicitation { elicitation, .. }) if elicitation.mcp_server_name == "docs")
    );
    assert_eq!(
        hints[2],
        Hint::Unsupported {
            request_id: "m1".to_owned(),
            subtype: "mcp_message".to_owned()
        }
    );
    assert_eq!(hints[3], Hint::Withdrawn("d1".to_owned()));
    assert!(events(&out).is_empty());
}

#[test]
fn an_api_error_fails_the_turn_with_its_message_and_may_be_retried() {
    let mut mapper = mapper();
    let text = "API Error: Repeated 529 Overloaded errors";
    let out = feed(
        &mut mapper,
        &[
            requesting("u1"),
            json!({ "type": "assistant", "is_api_error_message": true, "error": "overloaded",
                "message": { "id": "e1", "model": "<synthetic>", "content": [{ "type": "text", "text": text }] } }),
            result("u1", json!({ "is_error": true, "result": text })),
        ],
    );

    assert!(!kinds(&out).contains(&"messageStart".to_owned()));
    assert!(events(&out).contains(&AgentEvent::PromptResult {
        request_id: Some("u1".to_owned()),
        status: PromptStatus::Error,
        error: Some(text.to_owned()),
        retryable: true,
    }));
}

#[test]
fn an_automatic_compaction_during_a_turn_and_a_failed_one_show_their_end() {
    let mut mapper = mapper();
    let compaction = |phase: &str, reason: Option<&str>, error: Option<&str>| AgentEvent::Compaction {
        phase: phase.to_owned(),
        reason: reason.map(str::to_owned),
        error: error.map(str::to_owned),
    };

    let auto = feed(
        &mut mapper,
        &[
            requesting("u1"),
            json!({ "type": "system", "subtype": "status", "status": "compacting" }),
            json!({ "type": "system", "subtype": "compact_boundary",
                "compact_metadata": { "trigger": "auto", "pre_tokens": 900000, "post_tokens": 20000 } }),
        ],
    );
    assert_eq!(
        events(&auto)[1..],
        [compaction("start", None, None), compaction("end", Some("auto"), None)]
    );

    let mut mapper = self::mapper();
    mapper.sent("c1", SendKind::Compact);
    let failed = feed(
        &mut mapper,
        &[
            json!({ "type": "command_lifecycle", "command_uuid": "c1", "state": "started" }),
            json!({ "type": "system", "subtype": "status", "status": "compacting" }),
            json!({ "type": "system", "subtype": "status", "status": null, "compact_result": "failed",
                "compact_error": "Not enough messages to compact." }),
        ],
    );
    assert_eq!(
        events(&failed),
        [
            compaction("start", Some("manual"), None),
            compaction("end", Some("manual"), Some("Not enough messages to compact.")),
        ]
    );
}

#[test]
fn notices_resets_and_broken_lines() {
    let mut mapper = mapper();
    let out = feed(
        &mut mapper,
        &[
            json!({ "type": "system", "subtype": "local_command_output", "content": "\u{1b}[1mUsage\u{1b}[0m: 12%" }),
            json!({ "type": "conversation_reset", "new_conversation_id": "n1", "trigger": "clear" }),
            json!({ "type": "keep_alive" }),
        ],
    );

    assert!(matches!(&events(&out)[..], [AgentEvent::Notice { message, .. }] if message == "Usage: 12%"));
    assert!(matches!(&hints(&out)[..], [Hint::ConversationReset(reset)] if reset.trigger.as_deref() == Some("clear")));

    let broken = mapper.on_line("{oops");
    assert!(matches!(
        &events(&broken)[..],
        [AgentEvent::FrameError {
            original_type: None,
            ..
        }]
    ));
}
