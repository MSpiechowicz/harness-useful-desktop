//! The event stream of the OpenCode 2 chat engine against the scripted OpenCode 2 server: a stream
//! OpenCode ends is opened again and the chat catches up from the session without showing anything
//! twice; a silent stream of a server that still answers is opened again the same way.

mod agent_support;
mod opencode2_engine_helpers;
mod opencode2_engine_support;

use std::time::Duration;

use agent_support::options;
use harness_business::agent::SessionOptions;
use harness_business::model::{AgentEvent, ChatBlock, MessageRole, PromptStatus};
use opencode2_engine_helpers::{message_end, prompt, results, streamed, streaming, user_texts};
use opencode2_engine_support::{count, start, start_with, until_marker};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn a_stream_ended_mid_run_is_opened_again_and_the_chat_catches_up_once() {
    let chat = start("opencode2_reconnect.jsonl").await;
    prompt(&chat, "Say hello").await;
    streaming(&chat).await;

    let events = until_marker(&chat).await;

    assert_eq!(
        chat.requests_to("GET /api/event").len(),
        2,
        "the stream was opened again"
    );
    assert!(
        !events.iter().any(|event| matches!(event, AgentEvent::Notice { .. })),
        "{events:#?}"
    );

    assert_eq!(user_texts(&events), ["Say hello"], "the prompt shows once");
    assert_eq!(
        streamed(&events, 0),
        "Hello there.",
        "only what was not streamed yet is added"
    );

    let replies = message_end(&events, MessageRole::Assistant);
    assert_eq!(replies.len(), 1, "{replies:#?}");
    assert_eq!(
        replies[0].blocks.first(),
        Some(&ChatBlock::Text {
            text: "Hello there.".to_owned()
        })
    );
    assert_eq!(replies[0].stop_reason.as_deref(), Some("stop"));

    let touched: Vec<Vec<String>> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolEnd {
                name, touched_paths, ..
            } if name == "write" => Some(touched_paths.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(touched, [vec!["notes.md".to_owned()]], "the missed call ends once");

    assert_eq!(results(&events), [PromptStatus::Completed]);
    assert_eq!((count(&events, "agentStart"), count(&events, "settled")), (1, 1));

    let names: Vec<Option<String>> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::State { state } => Some(state.session_name.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(names, [None, Some("Caught up".to_owned()), Some("Marker".to_owned())]);
    assert!(chat.session().is_running());

    chat.session().stop().await;
}

#[tokio::test]
async fn a_silent_stream_of_a_server_that_answers_is_opened_again_and_the_chat_catches_up() {
    let quick = SessionOptions {
        event_silence: Duration::from_millis(200),
        ..options()
    };
    let chat = start_with("opencode2_silent.jsonl", None, quick).await;

    let events = until_marker(&chat).await;

    assert!(
        chat.requests_to("GET /api/event").len() >= 2,
        "the silent stream was opened again"
    );
    assert!(
        !chat.requests_to("GET /api/session/*/message").is_empty(),
        "the chat caught up"
    );
    assert!(
        !events.iter().any(|event| matches!(event, AgentEvent::Notice { .. })),
        "{events:#?}"
    );

    // A stream that stays silent is opened again each time, and the chat goes on.
    let opened = chat.requests_to("GET /api/event").len();
    tokio::time::sleep(Duration::from_millis(900)).await;

    assert!(chat.requests_to("GET /api/event").len() > opened);
    assert!(chat.session().is_running());
    assert!(
        !chat
            .events()
            .iter()
            .any(|event| matches!(event, AgentEvent::Notice { .. } | AgentEvent::Exited { .. })),
        "{:#?}",
        chat.events()
    );

    chat.session().stop().await;
}
