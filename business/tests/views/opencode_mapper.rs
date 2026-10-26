//! The OpenCode chat engine's event mapping (`Mapper`): messages, tool calls, sessions and errors,
//! against the recorded `opencode serve` run (`fixtures/opencode/server`) and slices of it.

use crate::opencode_mapper_support;

use harness_business::model::{AgentEvent, ChatBlock, MessageRole, NoticeLevel, PromptStatus};
use harness_business::opencode_server::wire::envelope;
use opencode_mapper_support::{SESSION, assistant, delta, events, feed, kind, mapper, part, recorded_events, status};
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn the_recorded_run_maps_to_settled_turns_of_its_own_session() {
    let mut mapper = mapper();
    let mut out = Vec::new();

    for data in recorded_events() {
        let envelope = envelope(&data).expect("every recorded event is an event object");
        out.extend(events(mapper.map(&envelope)));
    }

    let count = |name: &str| out.iter().filter(|event| kind(event) == name).count();

    // Hello, tool, edit, question, slow with a steer, abort, image, shell, compaction, bad model.
    assert_eq!(count("agentStart"), 10);
    assert_eq!(count("agentEnd"), 10);
    assert_eq!(count("settled"), 10);
    assert_eq!(count("promptResult"), 0, "no prompt was sent through this mapper");
    assert_eq!(count("notice"), 0, "the repeated model error is not shown again");
    assert_eq!(count("frameError"), 0);

    let aborted: Vec<_> = out
        .iter()
        .filter(|event| matches!(event, AgentEvent::Settled { aborted: Some(true) }))
        .collect();
    assert_eq!(aborted.len(), 1);

    let user_texts: Vec<String> = out
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } if message.role == MessageRole::User => {
                match message.blocks.first() {
                    Some(ChatBlock::Text { text }) => Some(text.clone()),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        user_texts,
        [
            "Say hello",
            "MAKE_TOOL now",
            "MAKE_EDIT now",
            "MAKE_QUESTION now",
            "SLOW answer",
            "steered message",
            "SLOW again",
            "Look",
            "x"
        ]
    );

    let tools: Vec<(String, bool)> = out
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolEnd { name, is_error, .. } => Some((name.clone(), *is_error)),
            _ => None,
        })
        .collect();
    let expected = [("bash", false), ("write", true), ("question", false), ("bash", false)];
    assert_eq!(tools, expected.map(|(name, error)| (name.to_owned(), error)).to_vec());

    let forks = ["msg_155aa031b002fyBhX1Yb6XFc7z", "msg_155aa0327002XTNdA98H1Pmb7R"];
    let shown = serde_json::to_string(&out).expect("events serialize");
    assert!(
        forks.iter().all(|id| !shown.contains(id)),
        "forked sessions are not this chat"
    );
}

#[test]
fn streamed_text_matches_the_final_text_of_each_reply() {
    let mut mapper = mapper();
    let mut out = Vec::new();

    for data in recorded_events() {
        out.extend(events(mapper.map(&envelope(&data).expect("event"))));
    }

    for event in &out {
        let AgentEvent::MessageEnd { message_id, message } = event else {
            continue;
        };

        if message.role != MessageRole::Assistant || message.stop_reason.as_deref() == Some("aborted") {
            continue;
        }

        let streamed: String = out
            .iter()
            .filter_map(|event| match event {
                AgentEvent::TextDelta {
                    message_id: id, delta, ..
                } if id == message_id => Some(delta.as_str()),
                _ => None,
            })
            .collect();
        let finished: String = message
            .blocks
            .iter()
            .filter_map(|block| match block {
                ChatBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(streamed, finished, "message {message_id}");
    }
}

#[test]
fn a_final_part_sends_only_the_text_not_streamed_yet() {
    let mut mapper = mapper();
    let message = "msg_1";
    let text = json!({ "id": "prt_t", "messageID": message, "type": "text", "text": "" });

    let out = events(feed(
        &mut mapper,
        &[
            status("busy"),
            assistant(message, false),
            part(SESSION, text),
            delta(message, "prt_t", "Hello "),
            part(
                SESSION,
                json!({ "id": "prt_t", "messageID": message, "type": "text", "text": "Hello world" }),
            ),
            part(
                SESSION,
                json!({ "id": "prt_t", "messageID": message, "type": "text", "text": "Rewritten" }),
            ),
            assistant(message, true),
        ],
    ));

    let deltas: Vec<(u32, &str)> = out
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta {
                content_index, delta, ..
            } => Some((*content_index, delta.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(deltas, [(0, "Hello "), (0, "world")]);

    let Some(AgentEvent::MessageEnd { message, .. }) = out.last() else {
        panic!("the reply ends last: {out:#?}");
    };
    assert_eq!(
        message.blocks,
        [ChatBlock::Text {
            text: "Rewritten".to_owned()
        }]
    );
}

#[test]
fn subagent_sessions_at_any_depth_stay_out_of_the_chat() {
    let mut mapper = mapper();
    let created = |id: &str, parent: &str| {
        json!({ "type": "session.created", "properties": { "sessionID": id,
            "info": { "id": id, "parentID": parent, "directory": "/work/project" } } })
    };
    let read = |session: &str, call: &str| {
        part(
            session,
            json!({ "id": format!("prt_{call}"), "messageID": "msg_c", "type": "tool", "tool": "read",
                "callID": call, "state": { "status": "completed", "input": { "filePath": "a.rs" },
                "output": "text", "title": "a.rs" } }),
        )
    };

    let out = events(feed(
        &mut mapper,
        &[
            created("ses_child", SESSION),
            created("ses_grandchild", "ses_child"),
            read("ses_child", "call_child"),
            read("ses_grandchild", "call_deep"),
            status("busy"),
            json!({ "type": "session.status", "properties": { "sessionID": "ses_child", "status": { "type": "idle" } } }),
            read(SESSION, "call_own"),
        ],
    ));

    assert_eq!(
        out.iter().map(kind).collect::<Vec<_>>(),
        ["agentStart", "toolStart", "toolEnd"],
        "a subagent's calls belong to its task card, and its going idle does not end the turn"
    );
    assert!(matches!(&out[2], AgentEvent::ToolEnd { tool_call_id, .. } if tool_call_id == "call_own"));
}

#[test]
fn errors_fail_the_turn_or_become_a_notice_once() {
    let mut mapper = mapper();
    mapper.prompt_sent();

    let error = |message: &str| {
        json!({ "type": "session.error", "properties": { "sessionID": SESSION,
            "error": { "name": "APIError", "data": { "message": message, "isRetryable": true } } } })
    };

    let out = events(feed(
        &mut mapper,
        &[
            status("busy"),
            error("Rate limited"),
            error("A later error"),
            status("idle"),
            error("APIError: Rate limited\n    at stack"),
            error("Something else"),
        ],
    ));

    let result = out
        .iter()
        .find(|event| matches!(event, AgentEvent::PromptResult { .. }))
        .expect("prompt result");
    assert_eq!(
        result,
        &AgentEvent::PromptResult {
            request_id: None,
            status: PromptStatus::Error,
            error: Some("Rate limited".to_owned()),
            retryable: true,
        }
    );

    let notices: Vec<&AgentEvent> = out
        .iter()
        .filter(|event| matches!(event, AgentEvent::Notice { .. }))
        .collect();
    assert_eq!(
        notices,
        [&AgentEvent::Notice {
            level: NoticeLevel::Error,
            message: "Something else".to_owned()
        }]
    );
}
