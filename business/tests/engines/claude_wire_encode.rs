//! The encoders of Claude Code's stream-json input (`claude_stream::wire::outbound`): user
//! messages, the app's control requests and its replies to the CLI's requests come out exactly as
//! the recordings sent them, and request ids never repeat within a process.

use crate::claude_recorded;

use std::collections::HashSet;

use claude_recorded::{sent, sent_request};
use harness_business::claude_stream::wire::outbound::{
    HookRegistration, HostRequest, ImagePart, Priority, UserMessage, ask_hook_output, control_error, control_request,
    hook_output, next_request_id, permission_allow, permission_deny, question_answers, user_message,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

#[test]
fn user_messages_are_written_as_recorded() {
    let basic = &sent("basic.jsonl", "user")[0];
    let prompt = UserMessage::prompt(basic["uuid"].as_str().unwrap(), "Reply with exactly the word: pong");
    assert_eq!(&user_message(&prompt), basic);

    let queue = sent("queue2.jsonl", "user");
    let mut steer = UserMessage::prompt(
        queue[1]["uuid"].as_str().unwrap(),
        "Also end your final answer with the word BANANA.",
    );
    steer.priority = Some(Priority::Next);
    assert_eq!(user_message(&steer), queue[1]);

    let mut follow_up = UserMessage::prompt(queue[2]["uuid"].as_str().unwrap(), "Follow-up one: what is 2+2?");
    follow_up.priority = Some(Priority::Later);
    assert_eq!(user_message(&follow_up), queue[2]);

    let injected = &sent("bash.jsonl", "user")[1];
    let mut quiet = UserMessage::prompt(
        injected["uuid"].as_str().unwrap(),
        injected["message"]["content"].as_str().unwrap(),
    );
    quiet.should_query = false;
    assert_eq!(&user_message(&quiet), injected);
}

#[test]
fn images_go_as_base64_blocks_after_the_text() {
    let mut message = UserMessage::prompt("u1", "What is this?");
    message.images.push(ImagePart {
        media_type: "image/png".to_owned(),
        data: "iVBORw0KGgo=".to_owned(),
    });

    assert_eq!(
        user_message(&message)["message"]["content"],
        json!([
            { "type": "text", "text": "What is this?" },
            { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "iVBORw0KGgo=" } },
        ])
    );
}

/// The recorded control request `subtype` (the n-th) and its id.
fn recorded(file: &str, subtype: &str, nth: usize) -> (Value, String) {
    let frame = sent_request(file, subtype, nth);
    let id = frame["request_id"].as_str().unwrap().to_owned();

    (frame, id)
}

#[test]
fn control_requests_are_written_as_recorded() {
    let cases: Vec<(&str, &str, usize, HostRequest)> = vec![
        (
            "basic.jsonl",
            "initialize",
            0,
            HostRequest::Initialize { hooks: Vec::new() },
        ),
        ("basic.jsonl", "get_context_usage", 0, HostRequest::GetContextUsage),
        ("basic.jsonl", "get_session_cost", 0, HostRequest::GetSessionCost),
        ("basic.jsonl", "mcp_status", 0, HostRequest::McpStatus),
        (
            "interrupt.jsonl",
            "interrupt",
            0,
            HostRequest::Interrupt { cancel_queued: false },
        ),
        (
            "queue2.jsonl",
            "cancel_async_message",
            0,
            HostRequest::CancelAsyncMessage {
                message_uuid: "482f1e5c-db56-4e62-8c67-aabc0f67df7e".to_owned(),
            },
        ),
        ("controls.jsonl", "list_models", 0, HostRequest::ListModels),
        (
            "controls.jsonl",
            "set_model",
            0,
            HostRequest::SetModel {
                model: Some("sonnet".to_owned()),
            },
        ),
        (
            "controls.jsonl",
            "apply_flag_settings",
            0,
            HostRequest::SetEffort {
                level: "low".to_owned(),
            },
        ),
        (
            "controls.jsonl",
            "set_max_thinking_tokens",
            0,
            HostRequest::SetMaxThinkingTokens { tokens: Some(0) },
        ),
        (
            "controls.jsonl",
            "rename_session",
            0,
            HostRequest::RenameSession {
                title: "Recorded rename".to_owned(),
            },
        ),
        ("controls.jsonl", "reload_skills", 0, HostRequest::ReloadSkills),
        ("controls.jsonl", "reload_plugins", 0, HostRequest::ReloadPlugins),
        (
            "tools.jsonl",
            "list_permission_rules",
            0,
            HostRequest::ListPermissionRules,
        ),
    ];

    for (file, subtype, nth, request) in cases {
        let (frame, id) = recorded(file, subtype, nth);
        assert_eq!(control_request(&id, &request), frame, "{file} {subtype}");
    }

    let (frame, id) = recorded("tools.jsonl", "initialize", 0);
    let hooks = vec![HookRegistration {
        event: "PreToolUse".to_owned(),
        matcher: Some(".*".to_owned()),
        callback_ids: vec!["askall".to_owned()],
    }];
    assert_eq!(control_request(&id, &HostRequest::Initialize { hooks }), frame);

    let cancel_all = control_request("req_9", &HostRequest::Interrupt { cancel_queued: true });
    assert_eq!(
        cancel_all["request"],
        json!({ "subtype": "interrupt", "cancel_queued": true })
    );
}

#[test]
fn replies_to_the_cli_are_written_as_recorded() {
    let replies = sent("tools.jsonl", "control_response");
    let id = |reply: &Value| reply["response"]["request_id"].as_str().unwrap().to_owned();

    assert_eq!(hook_output(&id(&replies[0]), ask_hook_output()), replies[0]);

    let read = json!({ "file_path": "/home/user/project/notes.txt" });
    assert_eq!(permission_allow(&id(&replies[1]), &read, &[]), replies[1]);

    assert_eq!(
        permission_deny(&id(&replies[5]), "The user declined this command."),
        replies[5]
    );

    let answered = &sent("question.jsonl", "control_response")[0];
    let input = json!({ "questions": answered["response"]["response"]["updatedInput"]["questions"] });
    let answers = vec![(
        "Which colours do you like?".to_owned(),
        vec!["Red".to_owned(), "Green".to_owned()],
    )];
    assert_eq!(&question_answers(&id(answered), &input, &answers), answered);

    assert_eq!(
        control_error("r1", "Not supported"),
        json!({ "type": "control_response", "response": { "subtype": "error", "request_id": "r1", "error": "Not supported" } })
    );
}

#[test]
fn request_ids_are_unique_within_the_process() {
    let ids: Vec<String> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| scope.spawn(|| (0..250).map(|_| next_request_id()).collect::<Vec<_>>()))
            .collect();

        workers.into_iter().flat_map(|worker| worker.join().unwrap()).collect()
    });

    let unique: HashSet<&String> = ids.iter().collect();
    assert_eq!(unique.len(), 1000);
    assert!(ids.iter().all(|id| id.starts_with("req_")));
}
