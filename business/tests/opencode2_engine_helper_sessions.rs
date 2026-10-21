//! The sessions of an OpenCode 2 helper chat against the scripted OpenCode 2 server: they are never
//! resumed, so one the startup created is deleted when the startup fails after it, and the one the
//! chat leaves for a new session is deleted too.

mod agent_support;
mod opencode2_engine_support;

use harness_business::model::{AgentAction, AgentEvent, AgentReply};
use opencode2_engine_support::{Chat, SESSION, start_mode, until_marker};
use pretty_assertions::assert_eq;

/// The sessions deleted, in order.
fn deleted(chat: &Chat) -> Vec<String> {
    chat.requests_to("DELETE /api/session/*")
        .iter()
        .map(|request| request.path().trim_start_matches("/api/session/").to_owned())
        .collect()
}

/// Lines the fake reports a scenario mismatch with.
fn mismatches(events: &[AgentEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Stderr { line } if line.starts_with("fake_agent:") => Some(line.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_helper_startup_that_fails_after_creating_its_session_deletes_it() {
    let chat = start_mode("opencode2_helper_failed.jsonl", None, true).await;

    assert!(chat.session.is_err(), "the start fails");
    chat.wait_reaped().await;
    assert_eq!(deleted(&chat), [SESSION]);
}

#[tokio::test]
async fn a_helper_chat_that_moves_to_a_new_session_deletes_the_one_it_left() {
    let chat = start_mode("opencode2_helper_new_session.jsonl", None, true).await;
    until_marker(&chat).await;

    let reply = chat.session().send(AgentAction::NewSession).await.expect("new session");
    assert_eq!(reply, AgentReply::Ack);
    assert_eq!(deleted(&chat), [SESSION], "the session it left");

    chat.session().stop().await;

    assert_eq!(deleted(&chat), [SESSION, "ses_0new0chat0000000000000001"]);
    assert_eq!(mismatches(&chat.events()), Vec::<String>::new());
}
