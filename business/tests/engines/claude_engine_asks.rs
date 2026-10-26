//! Permission prompts, questions and the CLI's other asks in the Claude Code chat engine, against
//! the scripted `claude` replaying tools.jsonl and question.jsonl (and asks crafted after them).
//! Each scenario checks the answers the engine writes: the scripted CLI exits 0 only when every
//! one matched.

use crate::claude_engine_support;

use claude_engine_support::{Chat, marker, prompt, start, start_in, stop_cleanly};
use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, ApprovalMode, IpcErrorCode, NoticeLevel, UiOutcome, UiPurpose, UiRequest,
    UiRequestKind, UiResponse,
};
use pretty_assertions::assert_eq;

const READ_ASK: &str = "f9c23b59-dafb-4bcb-aa8f-fd0d3dd816c9";
const WRITE_ASK: &str = "33b799d2-2e75-432e-8d84-d2a660dd3979";
const BASH_ASK: &str = "42443ed3-2830-4af2-ac90-1baccc6fc545";
const SETTINGS_WRITE_ASK: &str = "5e1f0a2b-0000-4000-8000-000000000002";
const OUTSIDE_WRITE_ASK: &str = "5e1f0a2b-0000-4000-8000-000000000003";

/// Waits for the dialog `id`.
async fn dialog(chat: &Chat, id: &str) -> UiRequest {
    let event = chat
        .wait(
            id,
            |event| matches!(event, AgentEvent::UiRequest { request } if request.id == id),
        )
        .await;

    match event {
        AgentEvent::UiRequest { request } => request,
        _ => unreachable!(),
    }
}

/// Waits for dialog `id` to close.
async fn closed(chat: &Chat, id: &str) {
    chat.wait(
        "uiCancel",
        |event| matches!(event, AgentEvent::UiCancel { request_id } if request_id == id),
    )
    .await;
}

async fn answer(chat: &Chat, id: &str, value: &str) {
    let response = UiResponse {
        id: id.to_owned(),
        outcome: UiOutcome::Value {
            value: value.to_owned(),
        },
    };

    chat.session()
        .respond(response)
        .await
        .unwrap_or_else(|error| panic!("answering {id} fails: {error}"));
}

async fn dismiss(chat: &Chat, id: &str) -> harness_business::Result<()> {
    let response = UiResponse {
        id: id.to_owned(),
        outcome: UiOutcome::Cancelled,
    };

    chat.session().respond(response).await
}

/// The title, options and details of a `select`.
fn select(request: &UiRequest) -> (&str, Vec<&str>, Vec<Option<&str>>) {
    match &request.kind {
        UiRequestKind::Select {
            title,
            options,
            details,
            ..
        } => (
            title,
            options.iter().map(String::as_str).collect(),
            details.iter().map(Option::as_deref).collect(),
        ),
        other => panic!("not a select: {other:?}"),
    }
}

fn dialog_ids(events: &[AgentEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::UiRequest { request } => Some(request.id.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn always_ask_shows_every_prompt_and_keeps_an_always_answer_for_the_chat() {
    let chat = start_in("claude_asks.jsonl", Some(ApprovalMode::AlwaysAsk)).await;
    chat.send(prompt("Read, write, then list")).await;

    let read = dialog(&chat, READ_ASK).await;
    assert_eq!(read.purpose, Some(UiPurpose::Approval));
    assert_eq!(read.tool_call_id.as_deref(), Some("toolu_01ATYkU9KWcM1vnvdzgqt7b2"));
    assert_eq!(
        select(&read),
        (
            "Allow Read: notes.txt?",
            vec!["Allow once", "Always allow", "Reject"],
            vec![
                None,
                Some("Allows every Read call on files in this project until this chat ends"),
                None
            ]
        )
    );
    answer(&chat, READ_ASK, "Allow once").await;

    let write = dialog(&chat, WRITE_ASK).await;
    assert_eq!(select(&write).0, "Allow Write: hello.txt?");
    answer(&chat, WRITE_ASK, "Always allow").await;

    let settings = dialog(&chat, SETTINGS_WRITE_ASK).await;
    assert_eq!(
        select(&settings),
        (
            "Allow Write: .claude/settings.json?",
            vec!["Allow once", "Reject"],
            vec![Some("Claude Code settings files need approval"), None]
        ),
        "a call the CLI allows no lasting rule for is never covered and offers no Always allow"
    );
    answer(&chat, SETTINGS_WRITE_ASK, "Reject").await;

    let outside = dialog(&chat, OUTSIDE_WRITE_ASK).await;
    assert_eq!(
        select(&outside).2[1],
        Some("Allows this exact Write call again until this chat ends"),
        "a file outside the project is not covered by the project's Write answer"
    );
    answer(&chat, OUTSIDE_WRITE_ASK, "Allow once").await;

    let bash = dialog(&chat, BASH_ASK).await;
    let (title, _, details) = select(&bash);
    assert_eq!(title, "Allow Bash: ls?");
    assert_eq!(details[1], Some("Allows this exact command again until this chat ends"));
    answer(&chat, BASH_ASK, "Reject").await;

    let events = chat.settled(1).await;
    assert_eq!(
        dialog_ids(&events),
        [READ_ASK, WRITE_ASK, SETTINGS_WRITE_ASK, OUTSIDE_WRITE_ASK, BASH_ASK],
        "the second Write prompt in the project was allowed without a dialog"
    );

    let refused = dismiss(&chat, READ_ASK).await.expect_err("already answered");
    assert_eq!(refused.code(), IpcErrorCode::NotFound);

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn always_allow_echoes_the_suggested_rules_for_the_session_and_subagents_and_withdrawals_work() {
    let chat = start_in("claude_ask_rules.jsonl", Some(ApprovalMode::Write)).await;

    let status = dialog(&chat, "0b7e0c11-0000-4000-8000-000000000002").await;
    assert_eq!(
        select(&status),
        (
            "Allow Bash: git status?",
            vec!["Allow once", "Always allow", "Reject"],
            vec![
                None,
                Some("Also allows Bash(git status:*), access to /tmp/shared until this chat ends"),
                None
            ]
        )
    );
    answer(&chat, &status.id, "Always allow").await;

    let subagent = dialog(&chat, "0b7e0c11-0000-4000-8000-000000000003").await;
    let (title, _, details) = select(&subagent);
    assert_eq!(title, "Allow Read for a subagent: /etc/hosts?");
    assert_eq!(details[0], Some("Path is outside allowed working directories"));
    assert_eq!(subagent.tool_call_id, None, "a subagent's calls are not in the chat");
    answer(&chat, &subagent.id, "Allow once").await;

    let withdrawn = "0b7e0c11-0000-4000-8000-000000000004";
    dialog(&chat, withdrawn).await;
    closed(&chat, withdrawn).await;
    let refused = dismiss(&chat, withdrawn).await.expect_err("withdrawn");
    assert_eq!(refused.code(), IpcErrorCode::NotFound);

    marker(&chat).await;
    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn only_the_exact_kept_command_runs_again_and_other_asks_are_declined_at_once() {
    let chat = start("claude_ask_commands.jsonl").await;

    let first = dialog(&chat, "0c7e0c11-0000-4000-8000-000000000001").await;
    assert_eq!(
        select(&first).2[1],
        Some("Allows this exact command again until this chat ends")
    );
    answer(&chat, &first.id, "Always allow").await;

    let chained = dialog(&chat, "0c7e0c11-0000-4000-8000-000000000003").await;
    assert_eq!(
        select(&chained).2[1],
        Some("Allows this exact command again until this chat ends")
    );
    answer(&chat, &chained.id, "Reject").await;

    let events = marker(&chat).await;
    assert_eq!(
        dialog_ids(&events),
        [
            "0c7e0c11-0000-4000-8000-000000000001",
            "0c7e0c11-0000-4000-8000-000000000003"
        ]
    );

    let notices: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::Notice {
                level: NoticeLevel::Warning,
                message,
            } => Some(message.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        notices,
        [
            "Claude Code asked for its oauth dialog, which this chat cannot show yet.",
            "The MCP server docs asked for input, which this chat cannot show yet. It was declined.",
        ]
    );

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_recorded_multi_select_question_is_answered_with_its_labels() {
    let chat = start("claude_question.jsonl").await;
    chat.send(prompt("Ask me which colours I like")).await;

    let question = dialog(&chat, "bbb7f33f-6e53-4681-ad89-6e1977429fc2").await;
    assert_eq!(question.purpose, Some(UiPurpose::Question));
    assert_eq!(
        question.kind,
        UiRequestKind::Input {
            title: "Which colours do you like? (any of Red, Green, Blue, separated by commas)".to_owned(),
            placeholder: Some("Red, Green, Blue".to_owned()),
            timeout_ms: None,
        }
    );
    answer(&chat, &question.id, "red, Green").await;

    chat.settled(1).await;
    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn questions_are_asked_one_at_a_time_take_typed_answers_and_can_be_dismissed() {
    let chat = start("claude_questions.jsonl").await;
    let first = "0d7e0c11-0000-4000-8000-000000000001";

    let database = dialog(&chat, first).await;
    assert_eq!(
        select(&database),
        (
            "Question 1 of 2: Which database?",
            vec!["Postgres", "Mongo", "Type your own answer…"],
            vec![Some("Relational"), Some("Documents"), None]
        )
    );
    let wrong = UiResponse {
        id: first.to_owned(),
        outcome: UiOutcome::Value {
            value: "Oracle".to_owned(),
        },
    };
    let refused = chat.session().respond(wrong).await.expect_err("not an option");
    assert_eq!(refused.code(), IpcErrorCode::InvalidInput);
    answer(&chat, first, "Mongo").await;

    let colours = dialog(&chat, &format!("{first}:2")).await;
    assert!(
        matches!(&colours.kind, UiRequestKind::Input { title, .. } if title.starts_with("Question 2 of 2: Which colours do you like?"))
    );
    answer(&chat, &colours.id, "blue, Purple").await;

    let second = "0d7e0c11-0000-4000-8000-000000000002";
    dialog(&chat, second).await;
    answer(&chat, second, "Type your own answer…").await;
    let typed = dialog(&chat, &format!("{second}:1:own")).await;
    assert!(matches!(&typed.kind, UiRequestKind::Input { title, .. } if title == "Which database?"));
    answer(&chat, &typed.id, "  SQLite please ").await;

    let third = "0d7e0c11-0000-4000-8000-000000000003";
    dialog(&chat, third).await;
    dismiss(&chat, third).await.expect("dismissed");

    marker(&chat).await;
    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn abort_closes_the_open_dialogs_and_denies_them() {
    let chat = start("claude_abort_asks.jsonl").await;
    chat.send(prompt("Read, write, then list")).await;
    marker(&chat).await;

    assert_eq!(dialog_ids(&chat.events()), [READ_ASK, WRITE_ASK]);

    assert_eq!(chat.send(AgentAction::Abort).await, AgentReply::Ack);
    closed(&chat, READ_ASK).await;
    closed(&chat, WRITE_ASK).await;
    chat.settled(1).await;

    let refused = dismiss(&chat, WRITE_ASK).await.expect_err("denied by the abort");
    assert_eq!(refused.code(), IpcErrorCode::NotFound);

    stop_cleanly(&chat).await;
}
