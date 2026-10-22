//! Asks and subagent sessions the OpenCode 2 chat engine did not see live, against the scripted
//! OpenCode 2 server: after the event stream was opened again, an ask raised meanwhile (a
//! subagent's too) shows and the dialog of one answered meanwhile closes; a subagent session known
//! only from its call's progress, once OpenCode lists it as a child, or from the session list of a
//! resumed session, reaches the chat with its asks; a session another tool's progress names, or one
//! OpenCode does not list, does not.

use crate::agent_support;
use crate::opencode2_engine_helpers;
use crate::opencode2_engine_support;

use agent_support::options;
use harness_business::model::{AgentEvent, UiOutcome, UiPurpose, UiRequest, UiRequestKind, UiResponse};
use opencode2_engine_helpers::prompt;
use opencode2_engine_support::{SESSION, start, start_with, until_marker};
use pretty_assertions::assert_eq;

/// The dialogs shown, in order.
fn shown(events: &[AgentEvent]) -> Vec<UiRequest> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::UiRequest { request } => Some(request.clone()),
            _ => None,
        })
        .collect()
}

/// The ids of the dialogs closed, in order.
fn closed(events: &[AgentEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::UiCancel { request_id } => Some(request_id.clone()),
            _ => None,
        })
        .collect()
}

fn title(request: &UiRequest) -> String {
    match &request.kind {
        UiRequestKind::Select { title, .. } => title.clone(),
        other => panic!("not a select: {other:?}"),
    }
}

#[tokio::test]
async fn after_the_stream_is_opened_again_missed_asks_show_and_settled_ones_close() {
    let chat = start("opencode2_reconnect_asks.jsonl").await;
    prompt(&chat, "Run it").await;

    let events = until_marker(&chat).await;

    let dialogs = shown(&events);
    let ids: Vec<&str> = dialogs.iter().map(|request| request.id.as_str()).collect();
    assert_eq!(ids.first(), Some(&"per_1"), "{dialogs:#?}");
    assert!(
        ids.contains(&"per_2"),
        "the subagent's ask raised meanwhile shows: {ids:?}"
    );
    assert!(
        dialogs
            .iter()
            .any(|request| request.purpose == Some(UiPurpose::Question)),
        "the form waiting meanwhile shows: {dialogs:#?}"
    );
    assert_eq!(ids.iter().filter(|id| **id == "per_2").count(), 1, "{ids:?}");
    assert_eq!(closed(&events), ["per_1"], "the ask answered meanwhile closes");

    let missed = dialogs.iter().find(|request| request.id == "per_2").expect("per_2");
    assert_eq!(title(missed), "Allow shell for a subagent: make test?");
    assert_eq!(missed.purpose, Some(UiPurpose::Approval));

    let lists = chat.requests_to("GET /api/permission/request");
    assert!(!lists.is_empty());
    assert!(
        lists
            .iter()
            .all(|request| request.target.contains("location%5Bdirectory%5D=")),
        "{lists:#?}"
    );

    let answer = UiResponse {
        id: "per_2".to_owned(),
        outcome: UiOutcome::Value {
            value: "Allow once".to_owned(),
        },
    };
    chat.session().respond(answer).await.expect("answered");

    let replies = chat.requests_to("POST /api/session/ses_child1/permission/per_2/reply");
    assert_eq!(replies.len(), 1, "the answer goes to the subagent session");
    assert_eq!(replies[0].body["decision"], "once");

    chat.session().stop().await;
}

#[tokio::test]
async fn a_subagent_known_from_its_calls_progress_reaches_the_chat_with_its_asks() {
    let chat = start("opencode2_subagent_progress.jsonl").await;
    prompt(&chat, "Delegate it").await;

    let events = until_marker(&chat).await;

    let dialogs = shown(&events);
    let ask = dialogs.iter().find(|request| request.id == "per_7");
    let ask = ask.unwrap_or_else(|| panic!("the subagent's ask shows: {events:#?}"));
    assert_eq!(title(ask), "Allow shell for a subagent: make test?");
    assert_eq!(ask.tool_call_id, None, "a subagent's call has no card in the chat");

    let lists = chat.requests_to("GET /api/session");
    assert!(
        lists
            .iter()
            .any(|request| request.target.contains(&format!("parentID={SESSION}"))),
        "OpenCode's list of children confirms the named session: {lists:#?}"
    );

    chat.session().stop().await;
}

#[tokio::test]
async fn a_session_a_call_names_that_opencode_does_not_list_as_a_child_never_reaches_the_chat() {
    let chat = start("opencode2_foreign_progress.jsonl").await;
    prompt(&chat, "Delegate it").await;

    let events = until_marker(&chat).await;

    let dialogs = shown(&events);
    assert!(
        dialogs.iter().all(|request| !request.id.starts_with("per_f")),
        "a foreign session's asks are not this chat's: {dialogs:#?}"
    );
    assert!(chat.requests_to("POST /api/session/*/permission/*/reply").is_empty());

    let lists: Vec<_> = chat
        .requests_to("GET /api/session")
        .into_iter()
        .filter(|request| request.target.contains(&format!("parentID={SESSION}")))
        .collect();
    assert_eq!(
        lists.len(),
        1,
        "only the subagent call has the children listed: {lists:#?}"
    );

    chat.session().stop().await;
}

#[tokio::test]
async fn a_resumed_chat_counts_the_subagent_sessions_of_its_session() {
    let chat = start_with("opencode2_resume_subagent.jsonl", Some(SESSION), options()).await;

    let events = until_marker(&chat).await;

    let children = chat.requests_to("GET /api/session");
    assert!(
        children
            .iter()
            .any(|request| request.target.contains(&format!("parentID={SESSION}"))),
        "{children:#?}"
    );

    let dialogs = shown(&events);
    let ask = dialogs.iter().find(|request| request.id == "per_8");
    let ask = ask.unwrap_or_else(|| panic!("the subagent's ask shows: {events:#?}"));
    assert_eq!(title(ask), "Allow shell for a subagent: make test?");

    chat.session().stop().await;
}
