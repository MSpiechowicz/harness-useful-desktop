//! The memory identity of a Codex chat across its threads (plan decision 11): before its first
//! thread and before every clone, fork and new chat the chat asks Codex for its MCP servers
//! (`config/read`) and passes its key to the harness-memory servers Codex lists then, in the
//! thread's `config`. A server Codex no longer lists is no longer named, and a read that fails
//! keeps the servers of the one before. A resumed thread gets it too.

use crate::codex_engine_ops_modes::{CLONE, FIRST_TURN, FORK, NEW, moved_to};
use crate::codex_engine_support;

use codex_engine_support::{Setup, THREAD, launch_with, prompt, start_with};
use harness_business::agent::AgentSession;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ApprovalMode};
use pretty_assertions::assert_eq;

fn with_memory() -> Setup {
    Setup {
        approval_override: Some(ApprovalMode::Write),
        memory_session: true,
        ..Setup::default()
    }
}

fn passed_on(session: &AgentSession) -> bool {
    match session {
        AgentSession::Codex(session) => session.memory_passed_on(),
        _ => panic!("not the Codex engine"),
    }
}

#[tokio::test]
async fn every_thread_of_the_chat_passes_its_key_to_the_servers_codex_lists_then() {
    let chat = start_with("codex_memory_ops.jsonl", with_memory(), |_| {}).await;
    assert!(passed_on(&chat.session));

    chat.act(prompt("Ask me about colours.")).await;
    chat.settled(1).await;

    assert_eq!(chat.act(AgentAction::Clone).await, AgentReply::Ack);
    moved_to(&chat, CLONE).await;

    let fork = AgentAction::Fork {
        entry_id: FIRST_TURN.to_owned(),
    };
    assert_eq!(chat.act(fork).await, AgentReply::Ack);
    moved_to(&chat, FORK).await;

    chat.act(AgentAction::NewSession).await;
    moved_to(&chat, NEW).await;

    // Each thread's config went to Codex as the scripted Codex expected.
    chat.finish().await;
}

#[tokio::test]
async fn a_resumed_thread_passes_the_key_on_too() {
    let launched = launch_with("codex_memory_resume.jsonl", Some(THREAD), with_memory(), |_| {}).await;
    let session = launched.result.expect("the resumed chat starts");
    assert!(passed_on(&session));

    session.stop().await;
    assert_eq!(
        launched.sink.events().last(),
        Some(&AgentEvent::Exited {
            code: Some(0),
            signal: None
        })
    );
}
