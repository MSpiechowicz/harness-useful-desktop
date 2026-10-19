//! OpenCode chat turns against the scripted `opencode serve`: streaming, tool calls of the chat and
//! its subagents, errors, aborts, the prompt-taken order and the masked server password.

mod agent_support;
mod opencode_engine_support;

use std::sync::{Arc, Mutex};

use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, ChatBlock, ChatMessage, MessageRole, NoticeLevel, PromptStatus,
};
use opencode_engine_support::{Chat, kind, start};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

fn say_hello() -> AgentAction {
    AgentAction::Prompt {
        message: "Say hello".to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    }
}

/// Sends the prompt and returns how many events were delivered when the prompt-taken call ran.
async fn prompt(chat: &Chat) -> Arc<Mutex<Option<usize>>> {
    let taken_at = Arc::new(Mutex::new(None));
    let (sink, record) = (chat.sink.clone(), taken_at.clone());
    chat.session().on_prompt_taken(Box::new(move || {
        *record.lock().unwrap() = Some(sink.events().len());
    }));

    let reply = chat.session().send(say_hello()).await.expect("prompt taken");
    assert_eq!(
        reply,
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("started".to_owned())
        }
    );

    taken_at
}

/// Waits for the `Marker` title the scenarios end with; the events of the scenario after startup.
async fn until_marker(chat: &Chat) -> Vec<AgentEvent> {
    chat.wait(
        "marker",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Marker")),
    )
    .await;

    chat.events().split_off(3)
}

fn message_end(events: &[AgentEvent], role: MessageRole) -> Vec<ChatMessage> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd { message, .. } if message.role == role => Some(message.clone()),
            _ => None,
        })
        .collect()
}

fn tool_ends(events: &[AgentEvent]) -> Vec<(String, String, bool, Value, Vec<String>)> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolEnd {
                tool_call_id,
                name,
                is_error,
                result,
                touched_paths,
            } => Some((
                tool_call_id.clone(),
                name.clone(),
                *is_error,
                result.clone(),
                touched_paths.clone(),
            )),
            _ => None,
        })
        .collect()
}

fn text_result(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }] })
}

#[tokio::test]
async fn a_streamed_turn_starts_streams_ends_and_settles_in_order() {
    let chat = start("opencode_turn.jsonl").await;
    let taken_at = prompt(&chat).await;
    let events = until_marker(&chat).await;

    assert_eq!(
        events.iter().map(kind).collect::<Vec<_>>(),
        [
            "messageStart",
            "messageEnd",
            "state",
            "agentStart",
            "messageStart",
            "thinkingDelta",
            "textDelta",
            "textDelta",
            "textDelta",
            "textDelta",
            "messageEnd",
            "state",
            "agentEnd",
            "promptResult",
            "settled",
            "state",
        ]
    );

    let agent_start = 3 + events
        .iter()
        .position(|event| *event == AgentEvent::AgentStart)
        .expect("agentStart");
    let taken_at = taken_at.lock().unwrap().expect("the prompt-taken call ran");
    assert!(
        taken_at <= agent_start,
        "taken after {taken_at} events, agentStart is event {agent_start}"
    );

    let streamed: String = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::TextDelta {
                content_index: 1,
                delta,
                ..
            } => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(streamed, "Hello from the mock model.");

    let reply = message_end(&events, MessageRole::Assistant).pop().expect("reply");
    assert_eq!(
        reply.blocks,
        [
            ChatBlock::Thinking {
                text: "Thinking about it.".to_owned(),
                redacted: false
            },
            ChatBlock::Text {
                text: "Hello from the mock model.".to_owned()
            },
        ]
    );
    assert_eq!(reply.stop_reason.as_deref(), Some("stop"));
    assert_eq!(reply.model.as_deref(), Some("m1"));
    assert_eq!(reply.usage.map(|usage| usage.total), Some(25));

    let user = message_end(&events, MessageRole::User);
    assert_eq!(user.len(), 1, "the late repeat of the user message is not shown again");
    assert_eq!(
        user[0].blocks,
        [ChatBlock::Text {
            text: "Say hello".to_owned()
        }]
    );

    let states: Vec<(Option<String>, Option<String>)> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::State { state } => {
                Some((state.session_name.clone(), state.model.as_ref().map(|m| m.id.clone())))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        states,
        [
            (None, Some("m1".to_owned())),
            (Some("Mock title".to_owned()), Some("m1".to_owned())),
            (Some("Marker".to_owned()), Some("m1".to_owned())),
        ],
        "the model OpenCode picked is learned, then the titles"
    );

    assert!(events.contains(&AgentEvent::PromptResult {
        request_id: None,
        status: PromptStatus::Completed,
        error: None,
        retryable: false
    }));
    assert!(events.contains(&AgentEvent::Settled { aborted: Some(false) }));

    chat.session().stop().await;
}

#[tokio::test]
async fn tool_calls_of_the_chat_and_its_subagents_end_with_touched_paths() {
    let chat = start("opencode_tools.jsonl").await;
    prompt(&chat).await;

    chat.wait("settled", |event| matches!(event, AgentEvent::Settled { .. }))
        .await;
    chat.wait("stderr", |event| matches!(event, AgentEvent::Stderr { .. }))
        .await;
    let events = chat.events();

    let starts: Vec<(String, Value)> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolStart { tool_call_id, args, .. } => Some((tool_call_id.clone(), args.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        starts,
        [
            (
                "call_1".to_owned(),
                json!({"command": "echo recorded", "description": "Echo a word"})
            ),
            (
                "call_2".to_owned(),
                json!({"filePath": "src/new.rs", "content": "hello\n"})
            ),
            (
                "call_child".to_owned(),
                json!({"filePath": "src/child.rs", "content": "x"})
            ),
        ],
        "one start per call, once it runs with its input"
    );

    let updates: Vec<Value> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolUpdate { partial, .. } => Some(partial.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(updates, [text_result(""), text_result("recorded\n")]);

    let ends = tool_ends(&events);
    assert_eq!(ends.len(), 3, "another session's call is not this chat's: {ends:#?}");
    assert_eq!(
        (ends[0].0.as_str(), ends[0].1.as_str(), ends[0].2),
        ("call_1", "bash", false)
    );
    assert_eq!(
        ends[1],
        (
            "call_2".to_owned(),
            "write".to_owned(),
            true,
            text_result("The user rejected permission to use this specific tool call."),
            vec!["src/new.rs".to_owned()],
        )
    );
    assert_eq!(
        ends[2],
        (
            "call_child".to_owned(),
            "write".to_owned(),
            false,
            text_result("Wrote file successfully."),
            vec!["src/child.rs".to_owned()],
        )
    );

    let shown = serde_json::to_string(&events).expect("events serialize");
    assert!(
        !shown.contains("Child text is not shown"),
        "a subagent's messages stay in its task"
    );

    let last = events
        .iter()
        .rposition(|event| matches!(event, AgentEvent::ToolEnd { .. }))
        .expect("tool end");
    let settled = events
        .iter()
        .position(|event| matches!(event, AgentEvent::Settled { .. }))
        .expect("settled");
    assert!(last < settled, "a subagent going idle does not end the turn");

    chat.session().stop().await;
}

#[tokio::test]
async fn the_server_password_never_reaches_the_chat() {
    let chat = start("opencode_tools.jsonl").await;
    prompt(&chat).await;

    chat.wait("settled", |event| matches!(event, AgentEvent::Settled { .. }))
        .await;
    let lines = |events: &[AgentEvent]| {
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::Stderr { .. }))
            .count()
    };
    let deadline = tokio::time::Instant::now() + agent_support::WAIT;
    while lines(&chat.events()) < 2 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "stdout and stderr lines: {:#?}",
            chat.events()
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let password = chat.password();
    let events = chat.events();
    let shown = serde_json::to_string(&events).expect("events serialize");

    assert!(!shown.contains(&password), "the password leaked: {shown}");
    assert!(shown.contains("••••"), "the streamed password was masked");

    chat.session().stop().await;

    let after = serde_json::to_string(&chat.events()).expect("events serialize");
    assert!(!after.contains(&password));
}

#[tokio::test]
async fn an_error_fails_the_prompt_and_a_repeated_error_is_not_shown_again() {
    let chat = start("opencode_error.jsonl").await;
    prompt(&chat).await;
    let events = until_marker(&chat).await;

    assert_eq!(
        events.iter().map(kind).collect::<Vec<_>>(),
        [
            "messageStart",
            "messageEnd",
            "agentStart",
            "agentEnd",
            "promptResult",
            "settled",
            "notice",
            "state"
        ]
    );
    assert!(events.contains(&AgentEvent::PromptResult {
        request_id: None,
        status: PromptStatus::Error,
        error: Some("Model not found: nope/missing. Did you mean: standardcompute?".to_owned()),
        retryable: false
    }));
    assert!(events.contains(&AgentEvent::Notice {
        level: NoticeLevel::Error,
        message: "Bad credentials".to_owned()
    }));

    chat.session().stop().await;
}

#[tokio::test]
async fn an_abort_ends_the_reply_with_what_arrived_and_settles_aborted() {
    let chat = start("opencode_abort.jsonl").await;
    prompt(&chat).await;
    let events = until_marker(&chat).await;

    assert_eq!(
        events.iter().map(kind).collect::<Vec<_>>(),
        [
            "messageStart",
            "messageEnd",
            "agentStart",
            "messageStart",
            "textDelta",
            "messageEnd",
            "agentEnd",
            "promptResult",
            "settled",
            "state"
        ],
        "the reply's late parts and the repeated idle add nothing"
    );

    let reply = message_end(&events, MessageRole::Assistant).pop().expect("reply");
    assert_eq!(
        reply.blocks,
        [ChatBlock::Text {
            text: "Hello ".to_owned()
        }]
    );
    assert_eq!(reply.stop_reason.as_deref(), Some("aborted"));

    assert!(events.contains(&AgentEvent::PromptResult {
        request_id: None,
        status: PromptStatus::Aborted,
        error: None,
        retryable: false
    }));
    assert!(events.contains(&AgentEvent::Settled { aborted: Some(true) }));

    chat.session().stop().await;
}
