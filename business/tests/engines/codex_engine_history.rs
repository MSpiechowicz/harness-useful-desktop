//! A resumed Codex chat's saved messages against the scripted app server: counted in the
//! statistics, paged as the history, the newest reply as the last reply, and the HTML export
//! written with the secrets masked.

use crate::codex_engine_support;

use codex_engine_support::{THREAD, start_as};
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ChatBlock, MessageRole};
use pretty_assertions::assert_eq;

/// The token-like word the saved command printed.
const TOKEN_LIKE: &str = "Zq8vT3nLw0pR6xYk2mB9sD4fH7jC1aE5";

#[tokio::test]
async fn a_resumed_thread_counts_pages_and_exports_its_saved_messages() {
    let chat = start_as("codex_engine_history.jsonl", Some(THREAD), false).await;

    chat.wait(
        "the counted messages",
        |event| matches!(event, AgentEvent::State { state } if state.message_count == 6),
    )
    .await;

    let AgentReply::Stats { stats } = chat.act(AgentAction::GetSessionStats).await else {
        panic!("not stats");
    };
    assert_eq!(
        (
            stats.total_messages,
            stats.user_messages,
            stats.assistant_messages,
            stats.tool_calls
        ),
        (6, 2, 3, 1)
    );

    let AgentReply::Text { text } = chat.act(AgentAction::GetLastAssistantText).await else {
        panic!("not a text");
    };
    assert_eq!(text.as_deref(), Some("README.md is the only file."));

    let newest = chat.act(AgentAction::GetHistory { cursor: None, limit: 4 }).await;
    let AgentReply::History {
        messages,
        next_cursor,
        total,
    } = newest
    else {
        panic!("not a history page");
    };
    let roles: Vec<MessageRole> = messages.iter().map(|message| message.role).collect();
    assert_eq!(
        roles,
        [
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::ToolResult,
            MessageRole::Assistant
        ]
    );
    assert_eq!((next_cursor.as_deref(), total), (Some("2"), Some(6)));

    let result = &messages[2].blocks;
    assert!(
        matches!(&result[..], [ChatBlock::Text { text }] if !text.contains(TOKEN_LIKE)),
        "the saved output is redacted: {result:?}"
    );

    let older = chat
        .act(AgentAction::GetHistory {
            cursor: next_cursor,
            limit: 4,
        })
        .await;
    let AgentReply::History {
        messages, next_cursor, ..
    } = older
    else {
        panic!("not a history page");
    };
    assert_eq!(messages.len(), 2);
    assert_eq!(next_cursor, None);

    let AgentReply::Exported { path } = chat.act(AgentAction::ExportHtml).await else {
        panic!("not an export");
    };
    assert_eq!(
        std::path::Path::new(&path),
        chat.exports.path().join(format!("codex-{THREAD}.html"))
    );

    let html = std::fs::read_to_string(&path).expect("the export");
    assert!(html.contains("Recorded thread") && html.contains("Codex") && html.contains("Teal."));
    assert!(!html.contains(TOKEN_LIKE), "the export holds no secret");

    chat.finish().await;
}

#[tokio::test]
async fn a_turn_that_runs_while_the_saved_messages_are_counted_counts_once() {
    let chat = start_as("codex_engine_history_live.jsonl", Some(THREAD), false).await;

    chat.act(codex_engine_support::prompt("Ask me about colours.")).await;
    chat.settled(1).await;

    // Six saved messages and the new turn's three.
    chat.wait(
        "the counted messages",
        |event| matches!(event, AgentEvent::State { state } if state.message_count >= 9),
    )
    .await;
    let AgentReply::Stats { stats } = chat.act(AgentAction::GetSessionStats).await else {
        panic!("not stats");
    };
    assert_eq!(stats.total_messages, 9);

    chat.finish().await;
}
