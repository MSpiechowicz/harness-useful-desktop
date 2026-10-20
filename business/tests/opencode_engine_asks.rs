//! Approvals, questions, approval modes and helper chats of the OpenCode chat engine against the
//! scripted `opencode serve`.

mod agent_support;
mod opencode_engine_support;

use agent_support::options;
use harness_business::BusinessError;
use harness_business::model::{AgentAction, AgentEvent, UiOutcome, UiPurpose, UiRequestKind};
use opencode_engine_support::{
    Chat, SESSION, answer, choose, dialog, kind, prompt, start, start_as, start_mode, terminated, until_marker,
};
use pretty_assertions::assert_eq;

fn options_of(kind: &UiRequestKind) -> Vec<String> {
    match kind {
        UiRequestKind::Select { options, .. } => options.clone(),
        other => panic!("not a select: {other:?}"),
    }
}

fn title_of(kind: &UiRequestKind) -> String {
    match kind {
        UiRequestKind::Select { title, .. } | UiRequestKind::Input { title, .. } => title.clone(),
        other => panic!("no title: {other:?}"),
    }
}

/// The ids of the dialogs the chat closed, in order.
fn cancelled(events: &[AgentEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::UiCancel { request_id } => Some(request_id.clone()),
            _ => None,
        })
        .collect()
}

/// Waits until the chat closed dialog `id`.
async fn closed(chat: &Chat, id: &str) {
    chat.wait(
        id,
        |event| matches!(event, AgentEvent::UiCancel { request_id } if request_id == id),
    )
    .await;
}

#[tokio::test]
async fn permission_asks_are_answered_once_always_or_rejected() {
    let chat = start("opencode_approvals.jsonl").await;
    prompt(&chat, "Say hello").await;

    let first = dialog(&chat, "per_1").await;
    assert_eq!(options_of(&first.kind), ["Allow once", "Always allow", "Reject"]);
    assert_eq!(title_of(&first.kind), "Allow bash: echo recorded?");
    choose(&chat, "per_1", "Allow once").await;

    dialog(&chat, "per_3").await;
    choose(&chat, "per_2", "Reject").await;
    closed(&chat, "per_3").await;

    dialog(&chat, "per_4").await;
    choose(&chat, "per_4", "Always allow").await;

    closed(&chat, "per_5").await;

    let subagent = dialog(&chat, "per_6").await;
    assert_eq!(title_of(&subagent.kind), "Allow bash for a subagent: make clean?");
    answer(&chat, "per_6", UiOutcome::Cancelled).await;

    let events = until_marker(&chat).await;
    assert_eq!(
        cancelled(&events),
        ["per_3", "per_5"],
        "the cascade and the ask answered elsewhere"
    );
    assert!(chat.session().is_running(), "every reply matched the scenario");

    let gone = choose_err(&chat, "per_1").await;
    assert!(matches!(gone, BusinessError::NotFound(_)), "{gone:?}");

    chat.session().stop().await;
}

async fn choose_err(chat: &Chat, id: &str) -> BusinessError {
    let response = harness_business::model::UiResponse {
        id: id.to_owned(),
        outcome: UiOutcome::Value {
            value: "Allow once".to_owned(),
        },
    };

    chat.session().respond(response).await.expect_err("not open")
}

#[tokio::test]
async fn questions_are_selects_or_inputs_answered_together() {
    let chat = start("opencode_questions.jsonl").await;
    prompt(&chat, "Say hello").await;

    let single = dialog(&chat, "que_1").await;
    assert_eq!(options_of(&single.kind), ["Alpha", "Beta", "Type your own answer…"]);
    choose(&chat, "que_1", "Beta").await;

    let many = dialog(&chat, "que_2").await;
    assert!(matches!(many.kind, UiRequestKind::Input { .. }));
    choose(&chat, "que_2", "alpha, Gamma").await;
    dialog(&chat, "que_2:2").await;
    choose(&chat, "que_2:2", "nothing").await;

    dialog(&chat, "que_3").await;
    answer(&chat, "que_3", UiOutcome::Cancelled).await;

    closed(&chat, "que_4").await;

    let yes_no = dialog(&chat, "que_5").await;
    assert_eq!(yes_no.purpose, Some(UiPurpose::Question), "never an approval card");
    assert_eq!(options_of(&yes_no.kind), ["Yes", "No", "Type your own answer…"]);
    choose(&chat, "que_5", "No").await;

    until_marker(&chat).await;
    assert!(chat.session().is_running(), "every answer matched the scenario");

    chat.session().stop().await;
}

#[tokio::test]
async fn an_abort_closes_the_open_ask_with_the_turn() {
    let chat = start("opencode_abort_ask.jsonl").await;
    prompt(&chat, "Say hello").await;
    dialog(&chat, "per_1").await;

    chat.session().send(AgentAction::Abort).await.expect("aborted");
    let events = until_marker(&chat).await;

    let closed_at = events
        .iter()
        .position(|event| kind(event) == "uiCancel")
        .expect("closed");
    let ended_at = events
        .iter()
        .position(|event| kind(event) == "agentEnd")
        .expect("ended");
    assert!(closed_at < ended_at, "{events:#?}");
    assert!(events.contains(&AgentEvent::Settled { aborted: Some(true) }));

    chat.session().stop().await;
}

#[tokio::test]
async fn a_helper_chat_denies_every_tool_and_deletes_its_session_at_stop() {
    let chat = start_mode("opencode_utility.jsonl", None, true).await;
    until_marker(&chat).await;

    chat.session().stop().await;

    let events = chat.events();
    let deleted = events
        .iter()
        .position(|event| matches!(event, AgentEvent::Stderr { line } if line == "deleted"));
    assert!(deleted.is_some(), "the session was deleted at stop: {events:#?}");
    assert_eq!(events.last(), Some(&terminated()));

    let resumed = start_as("opencode_utility.jsonl", Some(SESSION), None, true, options()).await;
    let error = resumed.session.as_ref().err().expect("refused");
    assert!(matches!(error, BusinessError::InvalidInput(_)), "{error:?}");
    assert!(resumed.spawner.requested().is_empty(), "nothing was started");
}

/// How many times dialog `id` was shown.
fn shown(events: &[AgentEvent], id: &str) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, AgentEvent::UiRequest { request } if request.id == id))
        .count()
}

#[tokio::test]
async fn an_answer_that_did_not_reach_opencode_shows_the_dialog_again() {
    let chat = start("opencode_reply_retry.jsonl").await;
    prompt(&chat, "Say hello").await;

    dialog(&chat, "per_1").await;
    let failed = choose_err(&chat, "per_1").await;
    assert!(failed.to_string().contains("HTTP 500"), "{failed}");

    let deadline = tokio::time::Instant::now() + agent_support::WAIT;
    while shown(&chat.events(), "per_1") < 2 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "never shown again: {:#?}",
            chat.events()
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    choose(&chat, "per_1", "Allow once").await;

    dialog(&chat, "que_1").await;
    let gone = chat
        .session()
        .respond(harness_business::model::UiResponse {
            id: "que_1".to_owned(),
            outcome: UiOutcome::Value {
                value: "Alpha".to_owned(),
            },
        })
        .await
        .expect_err("the ask is gone");
    assert!(gone.to_string().contains("HTTP 404"), "{gone}");

    let events = until_marker(&chat).await;
    assert_eq!(shown(&events, "que_1"), 1, "a 404 leaves the dialog closed");
    assert!(chat.session().is_running(), "every reply matched the scenario");

    chat.session().stop().await;
}
