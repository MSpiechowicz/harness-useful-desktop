//! OpenCode 2 chat turns against the scripted OpenCode 2 server: streaming, tool calls, errors,
//! aborts, the prompt-taken order and the masked server password.

use crate::opencode2_engine_helpers;
use crate::opencode2_engine_support;

use harness_business::model::{AgentEvent, ChatBlock, MessageRole, NoticeLevel, PromptStatus};
use opencode2_engine_helpers::{message_end, prompt, results, streamed, user_texts};
use opencode2_engine_support::{kind, start, until_marker};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn tool_ends(events: &[AgentEvent]) -> Vec<(String, String, bool, Vec<String>)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolEnd {
                tool_call_id,
                name,
                is_error,
                touched_paths,
                ..
            } => Some((tool_call_id.clone(), name.clone(), *is_error, touched_paths.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_streamed_turn_starts_streams_ends_and_settles_in_order() {
    let chat = start("opencode2_turn.jsonl").await;
    let taken_at = prompt(&chat, "Say hello").await;
    let events = until_marker(&chat).await;

    assert_eq!(
        events.iter().map(kind).collect::<Vec<_>>(),
        [
            "ready",
            "state",
            "commandsUpdated",
            "agentStart",
            "messageStart",
            "messageEnd",
            "messageStart",
            "thinkingDelta",
            "thinkingDelta",
            "textDelta",
            "textDelta",
            "textDelta",
            "messageEnd",
            "agentEnd",
            "promptResult",
            "settled",
            "state",
        ],
        "catalog and unknown events show nothing"
    );

    let agent_start = events
        .iter()
        .position(|event| *event == AgentEvent::AgentStart)
        .expect("agentStart");
    let taken_at = taken_at.lock().unwrap().expect("the prompt-taken call ran");
    assert!(
        taken_at <= agent_start,
        "taken after {taken_at} events, agentStart is event {agent_start}"
    );

    assert_eq!(streamed(&events, 1), "Hello from the fake server.");

    let reply = message_end(&events, MessageRole::Assistant).pop().expect("reply");
    assert_eq!(
        reply.blocks,
        [
            ChatBlock::Thinking {
                text: "The user wants a greeting.".to_owned(),
                redacted: false
            },
            ChatBlock::Text {
                text: "Hello from the fake server.".to_owned()
            },
        ]
    );
    assert_eq!(reply.stop_reason.as_deref(), Some("stop"));
    assert_eq!(reply.model.as_deref(), Some("big-pickle"));
    assert_eq!(reply.usage.map(|usage| usage.total), Some(280 + 12 + 9 + 5120));

    assert_eq!(user_texts(&events), ["Say hello"]);
    assert_eq!(results(&events), [PromptStatus::Completed]);
    assert!(events.contains(&AgentEvent::Settled { aborted: Some(false) }));

    let state = match events.last() {
        Some(AgentEvent::State { state }) => state,
        other => panic!("last: {other:?}"),
    };
    let context = state.context_usage.as_ref().expect("the step sizes the context");
    assert_eq!(context.tokens, 280 + 12 + 9 + 5120);

    chat.session().stop().await;
}

#[tokio::test]
async fn tool_calls_end_with_their_output_and_v2_ids() {
    let chat = start("opencode2_tools.jsonl").await;
    prompt(
        &chat,
        "Use the read tool to read notes.txt, then use the glob tool to list *.txt files. Then reply with one short sentence.",
    )
    .await;
    let events = until_marker(&chat).await;

    let starts: Vec<(String, Value)> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolStart { name, args, .. } => Some((name.clone(), args.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(starts.len(), 2, "{starts:#?}");
    assert_eq!(
        starts.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(),
        ["read", "glob"]
    );
    assert!(starts[0].1.get("path").is_some(), "{:?}", starts[0].1);

    let ends = tool_ends(&events);
    assert_eq!(
        ends.iter()
            .map(|(_, name, is_error, touched)| (name.as_str(), *is_error, touched.len()))
            .collect::<Vec<_>>(),
        [("read", false, 0), ("glob", false, 0)],
        "reading touches nothing"
    );

    let last_end = events
        .iter()
        .rposition(|event| matches!(event, AgentEvent::ToolEnd { .. }))
        .expect("tool end");
    let settled = events
        .iter()
        .position(|event| matches!(event, AgentEvent::Settled { .. }))
        .expect("settled");
    assert!(last_end < settled);
    assert_eq!(results(&events), [PromptStatus::Completed]);

    chat.session().stop().await;
}

#[tokio::test]
async fn the_server_password_never_reaches_the_chat() {
    let chat = start("opencode2_password.jsonl").await;
    prompt(&chat, "Say hello").await;
    until_marker(&chat).await;

    let password = chat.password();
    let events = chat.events();
    let shown = serde_json::to_string(&events).expect("events serialize");

    assert!(!shown.contains(&password), "the password leaked: {shown}");
    assert!(shown.contains("••••"), "the password was masked");
    assert!(
        events.iter().any(|event| matches!(event, AgentEvent::Stderr { .. })),
        "server output is passed on"
    );

    chat.session().stop().await;

    let after = serde_json::to_string(&chat.events()).expect("events serialize");
    assert!(!after.contains(&password));
}

#[tokio::test]
async fn an_error_fails_the_prompt_and_a_later_error_is_a_notice() {
    let chat = start("opencode2_error.jsonl").await;
    prompt(&chat, "Say hello").await;
    let events = until_marker(&chat).await;

    assert_eq!(results(&events), [PromptStatus::Error]);
    assert!(events.contains(&AgentEvent::PromptResult {
        request_id: None,
        status: PromptStatus::Error,
        error: Some("Model not found: nope/missing".to_owned()),
        retryable: false
    }));
    assert!(events.contains(&AgentEvent::Notice {
        level: NoticeLevel::Error,
        message: "Bad credentials".to_owned()
    }));
    assert!(events.contains(&AgentEvent::Settled { aborted: Some(false) }));

    chat.session().stop().await;
}

#[tokio::test]
async fn an_abort_ends_the_reply_with_what_arrived_and_settles_aborted() {
    let chat = start("opencode2_abort.jsonl").await;
    prompt(&chat, "Say hello").await;
    opencode2_engine_helpers::streaming(&chat).await;

    let reply = chat
        .session()
        .send(harness_business::model::AgentAction::Abort)
        .await
        .expect("interrupted");
    assert_eq!(reply, harness_business::model::AgentReply::Ack);

    let events = until_marker(&chat).await;
    let reply = message_end(&events, MessageRole::Assistant).pop().expect("reply");
    assert_eq!(reply.stop_reason.as_deref(), Some("aborted"));
    assert_eq!(
        reply.blocks.first(),
        Some(&ChatBlock::Text {
            text: "Hello ".to_owned()
        }),
        "the reply ends with what arrived, the late delta is not added"
    );

    let ends = tool_ends(&events);
    assert_eq!(ends.len(), 1);
    assert!(ends[0].2, "the interrupted call failed");

    assert_eq!(results(&events), [PromptStatus::Aborted]);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::Settled { .. }))
            .collect::<Vec<_>>(),
        [&AgentEvent::Settled { aborted: Some(true) }],
        "the repeated end adds nothing"
    );
    assert_eq!(chat.requests_to("POST /api/session/*/interrupt").len(), 1);
    assert_eq!(
        json!(chat.requests_to("POST /api/session/*/interrupt")[0].body),
        Value::Null,
        "an interrupt has no body"
    );

    chat.session().stop().await;
}
