//! Stopping a subagent and messaging it, in the OpenCode 2 chat engine against the scripted
//! OpenCode 2 server: a subagent is a child session, so both go to that session.

use crate::opencode2_engine_helpers;
use crate::opencode2_engine_support;

use harness_business::BusinessError;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, Subagent, SubagentStatus, SubagentStepKind};
use opencode2_engine_helpers::prompt;
use opencode2_engine_support::{Chat, SESSION, start, until_marker};
use pretty_assertions::assert_eq;

const CHILD: &str = "ses_child1";

fn stop(id: &str) -> AgentAction {
    AgentAction::StopSubagent { id: id.to_owned() }
}

fn message(id: &str, text: &str) -> AgentAction {
    AgentAction::MessageSubagent {
        id: id.to_owned(),
        message: text.to_owned(),
    }
}

/// Waits for the snapshot of the subagent while its command runs.
async fn working(chat: &Chat) -> Subagent {
    let event = chat
        .wait("the working subagent", |event| {
            matches!(event, AgentEvent::Subagent { subagent }
                if subagent.id == CHILD && subagent.current_tool.as_deref() == Some("shell"))
        })
        .await;

    match event {
        AgentEvent::Subagent { subagent } => *subagent,
        _ => unreachable!(),
    }
}

/// The newest snapshot of the subagent.
fn latest(events: &[AgentEvent]) -> Subagent {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            AgentEvent::Subagent { subagent } if subagent.id == CHILD => Some((**subagent).clone()),
            _ => None,
        })
        .expect("a snapshot of the subagent")
}

#[tokio::test]
async fn stopping_a_subagent_interrupts_its_session_and_the_chat_goes_on() {
    let chat = start("opencode2_subagent_stop.jsonl").await;
    prompt(&chat, "Delegate it").await;

    let snapshot = working(&chat).await;
    assert_eq!(
        (snapshot.label.as_deref(), snapshot.can_stop),
        (Some("Check the tests"), Some(true))
    );

    let session = chat.session();
    assert_eq!(session.send(stop(CHILD)).await.expect("stopped"), AgentReply::Ack);

    let events = until_marker(&chat).await;
    let ended = latest(&events);
    assert_eq!((ended.status, ended.can_stop), (SubagentStatus::Aborted, Some(false)));
    assert!(
        !events.iter().any(|event| matches!(event, AgentEvent::Settled { .. })),
        "the chat's own run was not stopped"
    );
    assert_eq!(chat.requests_to("POST /api/session/ses_child1/interrupt").len(), 1);

    assert_eq!(
        session.send(stop(CHILD)).await.expect("nothing to stop"),
        AgentReply::Ack,
        "a subagent that finished needs no second interrupt"
    );
    assert!(matches!(
        session.send(stop("ses_stranger")).await,
        Err(BusinessError::NotFound(_))
    ));
    assert_eq!(chat.requests_to("POST /api/session/*/interrupt").len(), 1);

    session.stop().await;
}

#[tokio::test]
async fn a_message_to_a_subagent_steers_its_session() {
    let chat = start("opencode2_subagent_message.jsonl").await;
    prompt(&chat, "Delegate it").await;
    working(&chat).await;

    let session = chat.session();
    assert!(matches!(
        session.send(message("ses_stranger", "Hello")).await,
        Err(BusinessError::NotFound(_))
    ));
    assert_eq!(
        session.send(message(CHILD, "Use the cache")).await.expect("sent"),
        AgentReply::Ack
    );

    let events = until_marker(&chat).await;
    let answered = events.iter().any(|event| {
        matches!(event, AgentEvent::SubagentStep { subagent_id, step }
            if subagent_id == CHILD && step.kind == SubagentStepKind::Text && step.detail == "Used the cache.")
    });
    assert!(answered, "the subagent's answer shows in its steps: {events:#?}");
    assert_eq!(latest(&events).status, SubagentStatus::Completed);

    let sent = chat.requests_to("POST /api/session/ses_child1/prompt");
    assert_eq!(sent.len(), 1);
    assert_eq!(
        (sent[0].body["text"].as_str(), sent[0].body["delivery"].as_str()),
        (Some("Use the cache"), Some("steer"))
    );

    assert!(
        matches!(
            session.send(message(CHILD, "Again")).await,
            Err(BusinessError::Conflict(_))
        ),
        "a subagent that finished takes no message"
    );
    assert_eq!(chat.requests_to("POST /api/session/ses_child1/prompt").len(), 1);

    session.stop().await;
}

#[tokio::test]
async fn the_chats_own_session_is_never_a_subagent_to_stop_or_message() {
    let chat = start("opencode2_subagent_own_session.jsonl").await;
    prompt(&chat, "Delegate it").await;

    let events = until_marker(&chat).await;
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::Subagent { subagent } if subagent.id == SESSION)),
        "the chat's own session is not shown as a subagent"
    );

    let session = chat.session();
    assert!(matches!(
        session.send(stop(SESSION)).await,
        Err(BusinessError::NotFound(_))
    ));
    assert!(matches!(
        session.send(message(SESSION, "Hello")).await,
        Err(BusinessError::NotFound(_))
    ));
    assert!(
        chat.requests_to(&format!("POST /api/session/{SESSION}/interrupt"))
            .is_empty()
    );
    assert!(session.is_running());

    session.stop().await;
}
