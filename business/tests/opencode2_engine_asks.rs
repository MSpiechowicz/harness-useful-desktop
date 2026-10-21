//! Approvals and question forms of the OpenCode 2 chat engine against the scripted OpenCode 2
//! server: answers go to the asking session's routes, a reject's cascade closes the other dialogs,
//! "Always allow" lasts as long as the chat (OpenCode is told `once`, so nothing is saved for later
//! chats), and an answer that did not reach OpenCode shows the dialog again. With an approval mode,
//! a subagent ask for an agent the mode's probe did not list can only be rejected.

mod agent_support;
mod opencode2_engine_helpers;
mod opencode2_engine_support;

use std::time::Duration;

use agent_support::WAIT;
use harness_business::BusinessError;
use harness_business::model::{AgentEvent, ApprovalMode, UiOutcome, UiPurpose, UiRequest, UiRequestKind, UiResponse};
use opencode2_engine_helpers::prompt;
use opencode2_engine_support::{Chat, start, start_mode, until_marker};
use pretty_assertions::assert_eq;

fn select(kind: &UiRequestKind) -> (String, Vec<String>, Vec<Option<String>>) {
    match kind {
        UiRequestKind::Select {
            title,
            options,
            details,
            ..
        } => (title.clone(), options.clone(), details.clone()),
        other => panic!("not a select: {other:?}"),
    }
}

/// Waits for dialog `id`.
async fn dialog(chat: &Chat, id: &str) -> UiRequest {
    let shown = chat
        .wait(
            id,
            |event| matches!(event, AgentEvent::UiRequest { request } if request.id == id),
        )
        .await;

    match shown {
        AgentEvent::UiRequest { request } => request,
        _ => unreachable!(),
    }
}

/// Answers dialog `id` with `outcome`.
async fn answer(chat: &Chat, id: &str, outcome: UiOutcome) -> harness_business::Result<()> {
    let response = UiResponse {
        id: id.to_owned(),
        outcome,
    };

    chat.session().respond(response).await
}

/// Chooses `value` in dialog `id`.
async fn choose(chat: &Chat, id: &str, value: &str) -> harness_business::Result<()> {
    answer(
        chat,
        id,
        UiOutcome::Value {
            value: value.to_owned(),
        },
    )
    .await
}

/// Waits until the chat closed dialog `id`.
async fn closed(chat: &Chat, id: &str) {
    chat.wait(
        id,
        |event| matches!(event, AgentEvent::UiCancel { request_id } if request_id == id),
    )
    .await;
}

/// How many times dialog `id` was shown.
fn shown(events: &[AgentEvent], id: &str) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, AgentEvent::UiRequest { request } if request.id == id))
        .count()
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

/// Panics when the fake reported a request the scenario did not expect (a `fake_agent:` stderr line).
fn assert_matched(events: &[AgentEvent]) {
    let mismatches: Vec<&AgentEvent> = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Stderr { line } if line.starts_with("fake_agent:")))
        .collect();

    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

/// The decisions the chat sent, in order.
fn decisions(chat: &Chat) -> Vec<String> {
    let replies = chat.requests_to("POST /api/session/*/permission/*/reply");

    replies
        .iter()
        .map(|reply| reply.body["decision"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[tokio::test]
async fn permission_asks_are_answered_once_rejected_or_allowed_for_the_chat() {
    let chat = start("opencode2_asks.jsonl").await;
    prompt(&chat, "Say hello").await;

    let first = dialog(&chat, "per_1").await;
    let (title, options, details) = select(&first.kind);
    assert_eq!(title, "Allow shell: echo hi?");
    assert_eq!(options, ["Allow once", "Always allow", "Reject"]);
    assert_eq!(details[1].as_deref(), Some("Also allows echo * until this chat ends"));
    assert_eq!(first.purpose, Some(UiPurpose::Approval));
    assert_eq!(first.tool_call_id.as_deref(), Some("call_1"));
    choose(&chat, "per_1", "Allow once").await.expect("once");

    dialog(&chat, "per_3").await;
    choose(&chat, "per_2", "Reject").await.expect("rejected");
    closed(&chat, "per_3").await;

    dialog(&chat, "per_5").await;
    choose(&chat, "per_4", "Always allow").await.expect("always");
    closed(&chat, "per_5").await;

    let other = dialog(&chat, "per_8").await;
    assert_eq!(
        select(&other.kind).0,
        "Allow shell: cat notes.txt?",
        "not covered: asked"
    );
    choose(&chat, "per_8", "Allow once").await.expect("once");

    let events = until_marker(&chat).await;
    assert_eq!(
        cancelled(&events),
        ["per_3", "per_5"],
        "the cascade and the covered waiting ask"
    );
    assert_eq!(shown(&events, "per_6"), 0, "covered: answered without a dialog");
    assert_eq!(shown(&events, "per_7"), 0, "a subagent's ask is covered too");
    assert_eq!(
        decisions(&chat),
        ["once", "reject", "once", "once", "once", "once", "once"],
        "OpenCode never hears `always`, so it saves nothing"
    );
    assert!(chat.session().is_running(), "every reply matched the scenario");
    assert_matched(&events);

    let gone = choose(&chat, "per_1", "Allow once").await.expect_err("not open");
    assert!(matches!(gone, BusinessError::NotFound(_)), "{gone:?}");

    chat.session().stop().await;
}

#[tokio::test]
async fn answers_that_do_not_reach_opencode_show_the_dialog_again() {
    let chat = start("opencode2_asks_retry.jsonl").await;
    prompt(&chat, "Say hello").await;

    dialog(&chat, "per_1").await;
    let failed = choose(&chat, "per_1", "Allow once").await.expect_err("a 500");
    assert!(failed.to_string().contains("HTTP 500"), "{failed}");

    let deadline = tokio::time::Instant::now() + agent_support::WAIT;
    while shown(&chat.events(), "per_1") < 2 {
        assert!(tokio::time::Instant::now() < deadline, "never shown again");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    choose(&chat, "per_1", "Allow once").await.expect("once");

    dialog(&chat, "per_2").await;
    choose(&chat, "per_2", "Always allow").await.expect("always");

    // Covered, but its answer failed: OpenCode still waits, so it shows.
    dialog(&chat, "per_3").await;
    choose(&chat, "per_3", "Allow once").await.expect("once");

    dialog(&chat, "frm_1").await;
    let gone = choose(&chat, "frm_1", "Red").await.expect_err("the form is gone");
    assert!(gone.to_string().contains("HTTP 404"), "{gone}");

    let events = until_marker(&chat).await;
    assert_eq!(shown(&events, "per_3"), 1);
    assert_eq!(shown(&events, "frm_1"), 1, "a 404 leaves the dialog closed");
    assert_eq!(decisions(&chat), ["once", "once", "once", "once", "once"]);
    assert!(chat.session().is_running(), "every reply matched the scenario");
    assert_matched(&events);

    chat.session().stop().await;
}

#[tokio::test]
async fn question_forms_are_answered_together_or_cancelled() {
    let chat = start("opencode2_question.jsonl").await;
    prompt(&chat, "Ask me").await;

    let color = dialog(&chat, "frm_1").await;
    assert_eq!(color.purpose, Some(UiPurpose::Question), "never an approval card");
    let (title, options, _) = select(&color.kind);
    assert_eq!(title, "Which color do you prefer, red or blue?");
    assert_eq!(options, ["Red", "Blue", "Type your own answer…"]);
    choose(&chat, "frm_1", "Blue").await.expect("answered");

    dialog(&chat, "frm_2").await;
    choose(&chat, "frm_2", "Red").await.expect("the first field");
    let sizes = dialog(&chat, "frm_2:2").await;
    assert!(matches!(sizes.kind, UiRequestKind::Input { .. }), "{sizes:?}");
    choose(&chat, "frm_2:2", "S, M").await.expect("both answered");

    dialog(&chat, "frm_3").await;
    answer(&chat, "frm_3", UiOutcome::Cancelled).await.expect("cancelled");

    dialog(&chat, "frm_4").await;
    closed(&chat, "frm_4").await;

    dialog(&chat, "frm_5").await;
    choose(&chat, "frm_5", "Red").await.expect("a subagent's form");

    let events = until_marker(&chat).await;
    assert!(chat.session().is_running(), "every answer matched the scenario");
    assert_matched(&events);

    chat.session().stop().await;
}

#[tokio::test]
async fn a_subagent_ask_for_an_agent_the_probe_did_not_list_can_only_be_rejected() {
    let chat = start_mode(
        "opencode2_modes_unknown_subagent.jsonl",
        Some(ApprovalMode::Write),
        false,
    )
    .await;
    let message = opencode2_engine_support::text_prompt("Say hello");
    chat.session().send(message).await.expect("prompt taken");

    let answer = |id: &str, value: &str| UiResponse {
        id: id.to_owned(),
        outcome: UiOutcome::Value {
            value: value.to_owned(),
        },
    };
    let per_2 = |event: &AgentEvent| matches!(event, AgentEvent::UiRequest { request } if request.id == "per_2");

    chat.wait(
        "per_1",
        |event| matches!(event, AgentEvent::UiRequest { request } if request.id == "per_1"),
    )
    .await;
    chat.session()
        .respond(answer("per_1", "Allow once"))
        .await
        .expect("allowed");
    until_marker(&chat).await;

    for allow in ["Allow once", "Always allow"] {
        let refused = chat.session().respond(answer("per_2", allow)).await;
        assert!(
            matches!(refused, Err(BusinessError::Forbidden(_))),
            "{allow}: {refused:?}"
        );
    }

    let replies = || chat.requests_to("POST /api/session/*/permission/per_2/reply");
    assert!(replies().is_empty(), "no allow reached OpenCode");

    let deadline = tokio::time::Instant::now() + WAIT;
    while chat.events().iter().filter(|event| per_2(event)).count() < 3 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the dialog shows again after each refused answer"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    chat.session()
        .respond(answer("per_2", "Reject"))
        .await
        .expect("rejected");
    assert_eq!(replies().len(), 1);
    assert_eq!(replies()[0].body["decision"], "reject");
    let mismatched =
        |event: &AgentEvent| matches!(event, AgentEvent::Stderr { line } if line.starts_with("fake_agent:"));
    assert!(!chat.events().iter().any(mismatched), "{:#?}", chat.events());

    chat.session().stop().await;
}
