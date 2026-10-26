//! The event-stream watchdog of the OpenCode chat engine against the scripted `opencode serve`: a
//! silent or closed stream while the server runs stops the chat, a notice first and `exited` last.

use crate::agent_support;
use crate::opencode_engine_support;

use std::time::Duration;

use agent_support::options;
use harness_business::agent::SessionOptions;
use harness_business::model::{AgentEvent, NoticeLevel};
use opencode_engine_support::{Chat, start, start_with, terminated};
use pretty_assertions::assert_eq;

/// Waits for `exited`; the events then.
async fn exited(chat: &Chat) -> Vec<AgentEvent> {
    chat.wait("exit", |event| matches!(event, AgentEvent::Exited { .. }))
        .await;

    chat.events()
}

/// Checks that the lost-connection notice comes before `exited`, which is last.
fn assert_lost(events: &[AgentEvent]) {
    let notice = events
        .iter()
        .position(|event| {
            matches!(event, AgentEvent::Notice { level: NoticeLevel::Error, message }
                if message.contains("connection to OpenCode was lost"))
        })
        .unwrap_or_else(|| panic!("no lost-connection notice: {events:#?}"));
    let exit = events
        .iter()
        .position(|event| matches!(event, AgentEvent::Exited { .. }))
        .expect("exited");

    assert!(notice < exit, "the notice comes first");
    assert_eq!(exit, events.len() - 1, "exited is the last event");
}

#[tokio::test]
async fn a_silent_event_stream_stops_the_chat() {
    let quick = SessionOptions {
        event_silence: Duration::from_millis(300),
        ..options()
    };
    let chat = start_with("opencode_hang.jsonl", None, quick).await;

    assert_lost(&exited(&chat).await);
    assert!(!chat.session().is_running());
}

#[tokio::test]
async fn a_closed_event_stream_stops_the_chat() {
    let chat = start("opencode_stream_closed.jsonl").await;
    let events = exited(&chat).await;

    assert_lost(&events);
    assert_eq!(events.last(), Some(&terminated()), "the server was ended");
}
