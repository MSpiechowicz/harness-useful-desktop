//! What Codex asks a chat: approvals as approval cards bound to their tool card (allowed once, for
//! the session where Codex offers it, rejected), the model's questions and an MCP server's form as
//! question dialogs, and the requests the app declines or refuses at once.

use crate::codex_engine_support;

use codex_engine_support::{CodexChat, prompt, start};
use harness_business::BusinessError;
use harness_business::model::{AgentEvent, NoticeLevel, UiPurpose, UiRequest, UiRequestKind};
use pretty_assertions::assert_eq;

/// The title and options of a `select` dialog.
fn select(request: &UiRequest) -> (String, Vec<String>) {
    match &request.kind {
        UiRequestKind::Select { title, options, .. } => (title.clone(), options.clone()),
        other => panic!("not a select: {other:?}"),
    }
}

/// The title of an `input` dialog.
fn input_title(request: &UiRequest) -> String {
    match &request.kind {
        UiRequestKind::Input { title, .. } => title.clone(),
        other => panic!("not an input: {other:?}"),
    }
}

/// Answers the next dialog (the `seen`-th) with `value`; the dialog.
async fn answer(chat: &CodexChat, seen: usize, value: &str) -> UiRequest {
    let dialog = chat.dialog(seen).await;
    chat.pick(&dialog.id, value).await.expect("the answer is taken");
    dialog
}

#[tokio::test]
async fn approvals_are_allowed_once_for_the_session_or_rejected() {
    let chat = start("codex_engine_approvals.jsonl").await;
    chat.act(prompt("Create hello.txt, then run ls -1.")).await;

    let probe = answer(&chat, 0, "Allow once").await;
    let (title, options) = select(&probe);
    assert_eq!(probe.purpose, Some(UiPurpose::Approval));
    assert_eq!(
        probe.tool_call_id.as_deref(),
        Some("exec-f61152e4-887b-4723-add5-193b11a1ad79")
    );
    assert!(title.starts_with("Allow shell: pwd && rg --files"), "{title}");
    assert_eq!(options, ["Allow once", "Reject"], "Codex lists no session answer here");

    let change = answer(&chat, 1, "Always allow").await;
    let (title, options) = select(&change);
    assert_eq!(title, "Allow apply_patch: hello.txt?");
    assert_eq!(options, ["Allow once", "Always allow", "Reject"]);

    let listing = chat.dialog(2).await;
    let refused = chat.pick(&listing.id, "Always allow").await.unwrap_err();
    assert!(matches!(refused, BusinessError::InvalidInput(_)), "{refused:?}");
    chat.pick(&listing.id, "Reject").await.expect("the dialog stayed open");
    assert_eq!(select(&listing).0, "Allow shell: ls -1?");

    let events = chat.settled(1).await;
    let ends = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::ToolEnd { .. }))
        .count();
    assert_eq!(ends, 3);

    chat.finish().await;
}

#[tokio::test]
async fn always_allow_answers_for_the_session_when_codex_offers_it() {
    let chat = start("codex_engine_always.jsonl").await;
    chat.act(prompt("Check the repository.")).await;

    let dialog = answer(&chat, 0, "Always allow").await;
    let UiRequestKind::Select { title, details, .. } = &dialog.kind else {
        panic!("not a select");
    };
    assert_eq!(title, "Allow shell: git status?");
    assert_eq!(details[0].as_deref(), Some("Read the repository state"));
    assert_eq!(dialog.tool_call_id.as_deref(), Some("exec-always"));

    chat.settled(1).await;
    chat.finish().await;
}

#[tokio::test]
async fn questions_and_forms_are_asked_one_dialog_at_a_time() {
    let chat = start("codex_engine_questions.jsonl").await;
    chat.act(prompt("Ask me.")).await;

    let colour = answer(&chat, 0, "blue").await;
    let (title, options) = select(&colour);
    assert_eq!(colour.purpose, Some(UiPurpose::Question));
    assert_eq!(title, "Question 1 of 2: Which colour?");
    assert_eq!(&options[..2], ["red", "blue"]);

    let name = answer(&chat, 1, "Ada").await;
    assert_eq!(input_title(&name), "Question 2 of 2: Your name?");

    let count = answer(&chat, 2, "3").await;
    assert_eq!(
        input_title(&count),
        "Question 1 of 2: tracker asks for Count. Create the ticket?"
    );
    let urgent = answer(&chat, 3, "Yes").await;
    assert_eq!(&select(&urgent).1[..2], ["Yes", "No"]);

    let events = chat.settled(1).await;
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::Notice { level: NoticeLevel::Warning, message } if message.contains("tracker asked to open a web page")
    )));
    let shown = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::UiRequest { .. }))
        .count();
    assert_eq!(shown, 4, "the URL, the tool call and the token refresh show no dialog");

    chat.finish().await;
}
