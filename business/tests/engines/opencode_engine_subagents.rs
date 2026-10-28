//! Stopping a subagent and messaging it, in the OpenCode chat engine against the scripted
//! `opencode serve`: a subagent is a child session, so both go to that session.

use crate::opencode_engine_support;

use harness_business::BusinessError;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, Subagent, SubagentStatus, SubagentStepKind};
use opencode_engine_support::{Chat, SESSION, prompt, start, until_marker};
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
                if subagent.id == CHILD && subagent.current_tool.as_deref() == Some("bash"))
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
async fn stopping_a_subagent_aborts_its_session_and_the_chat_goes_on() {
    let chat = start("opencode_subagent_stop.jsonl").await;
    prompt(&chat, "Delegate it").await;

    let snapshot = working(&chat).await;
    assert_eq!(
        (snapshot.label.as_deref(), snapshot.can_stop),
        (Some("Check the tests"), Some(true))
    );

    let session = chat.session();
    assert_eq!(session.send(stop(CHILD)).await.expect("stopped"), AgentReply::Ack);

    let events = until_marker(&chat).await;
    let shown = serde_json::to_string(&events).expect("events serialize");
    assert!(shown.contains("call_secret"), "the subagent's command shows as a step");
    assert!(
        !shown.contains(&chat.password()),
        "the server password is masked in the steps too"
    );

    let ended = latest(&events);
    assert_eq!((ended.status, ended.can_stop), (SubagentStatus::Aborted, Some(false)));
    assert!(
        !events.iter().any(|event| matches!(event, AgentEvent::Settled { .. })),
        "the chat's own turn was not stopped"
    );

    assert_eq!(
        session.send(stop(CHILD)).await.expect("nothing to stop"),
        AgentReply::Ack,
        "a subagent that finished needs no second abort"
    );
    assert!(matches!(
        session.send(stop("ses_stranger")).await,
        Err(BusinessError::NotFound(_))
    ));
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}

#[tokio::test]
async fn a_message_to_a_subagent_is_a_prompt_of_its_session_with_its_agent_and_model() {
    let chat = start("opencode_subagent_message.jsonl").await;
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

    assert!(
        matches!(
            session.send(message(CHILD, "Again")).await,
            Err(BusinessError::Conflict(_))
        ),
        "a subagent that finished takes no message"
    );
    assert!(
        session.is_running(),
        "the prompt matched the scenario's agent and model"
    );

    session.stop().await;
}

#[tokio::test]
async fn the_chats_own_session_is_never_a_subagent_to_stop_or_message() {
    let chat = start("opencode_subagent_own_session.jsonl").await;
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
        session.is_running(),
        "no abort or prompt request left for a scenario that expects none"
    );

    session.stop().await;
}

#[tokio::test]
async fn a_message_waits_for_the_agent_the_server_reports_for_the_subagent() {
    let chat = start("opencode_subagent_unreported_agent.jsonl").await;
    prompt(&chat, "Delegate it").await;
    working(&chat).await;

    let session = chat.session();
    let refused = session.send(message(CHILD, "Use the cache")).await;

    assert!(
        matches!(&refused, Err(BusinessError::Conflict(reason)) if reason.contains("agent")),
        "the subagent's agent is only what a model wrote: {refused:?}"
    );
    assert!(session.is_running(), "no prompt was sent to the subagent");

    session.stop().await;
}

#[tokio::test]
async fn a_message_is_refused_when_the_reported_agent_is_too_long_to_send_as_it_is() {
    let chat = start("opencode_subagent_long_agent.jsonl").await;
    prompt(&chat, "Delegate it").await;
    working(&chat).await;

    let session = chat.session();
    let refused = session.send(message(CHILD, "Use the cache")).await;

    assert!(
        matches!(&refused, Err(BusinessError::Conflict(reason)) if reason.contains("agent")),
        "the shown copy of the agent is cut, so it is never sent: {refused:?}"
    );
    assert!(session.is_running(), "no prompt was sent to the subagent");

    session.stop().await;
}
