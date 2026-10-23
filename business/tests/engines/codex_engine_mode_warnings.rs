//! What a Codex chat says when Codex runs a thread with more than its approval mode shows: with
//! or without an override, folders outside the project to write in and network access, and
//! without one a model reviewer too. Each warning shows once per chat, not again for every thread
//! the chat moves to.

use crate::codex_engine_ops_modes::{FORK, NEW, moved_to};
use crate::codex_engine_support;

use codex_engine_support::{CodexChat, Setup, start_with};
use harness_business::model::{AgentAction, AgentEvent, ApprovalMode, NoticeLevel};
use pretty_assertions::assert_eq;

/// The threads of the new chats.
const SECOND: &str = NEW;
const THIRD: &str = FORK;

/// The warnings the chat showed so far.
fn warnings(chat: &CodexChat) -> Vec<String> {
    chat.events()
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::Notice {
                level: NoticeLevel::Warning,
                message,
            } => Some(message),
            _ => None,
        })
        .collect()
}

/// Starts a new chat in `chat` and waits until it runs thread `id`.
async fn new_chat(chat: &CodexChat, id: &str) {
    chat.act(AgentAction::NewSession).await;
    moved_to(chat, id).await;
}

#[tokio::test]
async fn without_an_override_a_wider_thread_is_named_once_per_answer() {
    let chat = start_with("codex_engine_mode_widened.jsonl", Setup::default(), |_| {}).await;
    chat.wait("the commands", |event| {
        matches!(event, AgentEvent::CommandsUpdated { .. })
    })
    .await;

    let first = warnings(&chat);
    assert_eq!(
        first,
        [
            "Codex's settings give this chat more than its mode shows: approvals reviewed by auto_review, writing also in /srv/shared, network access"
        ]
    );

    new_chat(&chat, SECOND).await;
    assert_eq!(warnings(&chat), first, "the same answer is not named again");

    new_chat(&chat, THIRD).await;
    let shown = warnings(&chat);
    assert_eq!(shown.len(), 2, "{shown:?}");
    assert_eq!(
        shown[1],
        "Codex's settings give this chat more than its mode shows: network access"
    );
    assert!(shown.iter().all(|message| !message.contains(';')));

    chat.finish().await;
}

#[tokio::test]
async fn an_override_codex_runs_with_extra_writable_roots_is_named() {
    let write = Setup {
        approval_override: Some(ApprovalMode::Write),
        ..Setup::default()
    };
    let chat = start_with("codex_engine_mode_write_root.jsonl", write, |_| {}).await;
    chat.wait("the commands", |event| {
        matches!(event, AgentEvent::CommandsUpdated { .. })
    })
    .await;

    assert_eq!(
        warnings(&chat),
        ["Codex's settings give this chat more than its mode: writing also in /"]
    );

    chat.finish().await;
}
