//! The Codex chat's approval mode across its session operations against the scripted app server:
//! a clone, a fork and a new chat start their threads with the chat's mode and keep it, and a
//! `!command` is refused in Always ask before anything reaches Codex.

use crate::codex_engine_support;

use codex_engine_support::{CodexChat, Setup, prompt, start_with};
use harness_business::BusinessError;
use harness_business::agent::AgentSession;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ApprovalMode};
use pretty_assertions::assert_eq;

pub(crate) const CLONE: &str = "01a164a2-3333-7000-8000-000000000003";
pub(crate) const FORK: &str = "01a164a2-2222-7000-8000-000000000002";
pub(crate) const NEW: &str = "01a164a2-1111-7000-8000-000000000001";

/// The first turn of the saved history.
pub(crate) const FIRST_TURN: &str = "01a164a0-4c86-7902-8de6-44d0f7bc8ac7";

fn in_mode(approval: ApprovalMode) -> Setup {
    Setup {
        approval_override: Some(approval),
        ..Setup::default()
    }
}

/// The chat's approval mode.
fn mode_of(chat: &CodexChat) -> Option<ApprovalMode> {
    match &chat.session {
        AgentSession::Codex(session) => session.approval_mode(),
        _ => panic!("not the Codex engine"),
    }
}

/// Waits until the chat's state names thread `id`.
pub(crate) async fn moved_to(chat: &CodexChat, id: &str) {
    chat.wait(
        "the new thread",
        |event| matches!(event, AgentEvent::State { state } if state.session_id == id),
    )
    .await;
}

#[tokio::test]
async fn clone_fork_and_new_chat_keep_the_chats_mode() {
    let chat = start_with("codex_engine_mode_ops.jsonl", in_mode(ApprovalMode::Write), |_| {}).await;

    chat.act(prompt("Ask me about colours.")).await;
    chat.settled(1).await;

    assert_eq!(chat.act(AgentAction::Clone).await, AgentReply::Ack);
    moved_to(&chat, CLONE).await;
    assert_eq!(mode_of(&chat), Some(ApprovalMode::Write));

    let fork = AgentAction::Fork {
        entry_id: FIRST_TURN.to_owned(),
    };
    assert_eq!(chat.act(fork).await, AgentReply::Ack);
    moved_to(&chat, FORK).await;
    assert_eq!(mode_of(&chat), Some(ApprovalMode::Write));

    chat.act(AgentAction::NewSession).await;
    moved_to(&chat, NEW).await;
    assert_eq!(mode_of(&chat), Some(ApprovalMode::Write));

    // Each thread went to Codex with Write's policy and sandbox: the scripted Codex checked them.
    chat.finish().await;
}

#[tokio::test]
async fn a_command_is_refused_in_always_ask_before_it_reaches_codex() {
    let chat = start_with("codex_engine_mode_ask.jsonl", in_mode(ApprovalMode::AlwaysAsk), |_| {}).await;

    let refused = chat.session.bash("echo from-shell").await.unwrap_err();
    assert!(matches!(&refused, BusinessError::Forbidden(reason) if reason.contains("Always ask")));

    let AgentReply::State { state } = chat.act(AgentAction::GetState).await else {
        panic!("not a state");
    };
    assert!(!state.is_streaming, "no shell turn was started");

    chat.finish().await;
}
