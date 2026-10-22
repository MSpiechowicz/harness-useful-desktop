//! The Claude Code chat engine when things go wrong, against the scripted `claude`: an abort whose
//! interrupt the CLI refuses still closes and denies the open prompts, a `!command` frees its slot
//! however it ends and never frees a later command's, a move whose old process exited is refused,
//! and the open prompts are bounded.

use crate::agent_support;
use crate::claude_engine_support;

use std::sync::Arc;
use std::time::Duration;

use claude_engine_support::{Chat, Setup, lf, marker, prompt, start, stop_cleanly};
use harness_business::BusinessError;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, IpcErrorCode};
use pretty_assertions::assert_eq;

const READ_ASK: &str = "f9c23b59-dafb-4bcb-aa8f-fd0d3dd816c9";
const WRITE_ASK: &str = "33b799d2-2e75-432e-8d84-d2a660dd3979";

/// Waits for dialog `id` to close.
async fn closed(chat: &Chat, id: &str) {
    chat.wait(
        "uiCancel",
        |event| matches!(event, AgentEvent::UiCancel { request_id } if request_id == id),
    )
    .await;
}

fn dialogs(events: &[AgentEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, AgentEvent::UiRequest { .. }))
        .count()
}

#[tokio::test]
async fn an_abort_whose_interrupt_fails_still_closes_and_denies_the_open_prompts() {
    let chat = start("claude_abort_asks_refused.jsonl").await;
    chat.send(prompt("Read, write, then list")).await;
    marker(&chat).await;
    assert_eq!(dialogs(&chat.events()), 2);

    let refused = chat.session().send(AgentAction::Abort).await;
    assert!(matches!(refused, Err(BusinessError::Protocol(_))), "{refused:?}");

    closed(&chat, READ_ASK).await;
    closed(&chat, WRITE_ASK).await;
    chat.settled(1).await;

    // The scripted CLI saw both denies.
    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_dropped_command_frees_the_chat_for_the_next_one() {
    let chat = Setup::new(&["claude_init.jsonl"]).start().await;

    let dropped = tokio::time::timeout(Duration::from_millis(300), chat.session().bash("sleep 30")).await;
    assert!(dropped.is_err(), "the call was dropped while the command ran");

    let output = chat.session().bash("echo freed").await.expect("the chat is free");
    assert_eq!(lf(&output.output), "freed\n");

    chat.stop().await;
}

#[tokio::test]
async fn an_aborted_command_never_frees_the_slot_of_the_next_one() {
    let chat = Arc::new(Setup::new(&["claude_init.jsonl"]).start().await);

    let first = {
        let chat = chat.clone();
        tokio::spawn(async move { chat.session().bash("sleep 30").await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert_eq!(chat.send(AgentAction::Abort).await, AgentReply::Ack);

    // The next command starts before the aborted one has ended.
    let second = chat.session().bash("sleep 30");
    tokio::pin!(second);
    tokio::select! {
        biased;
        _ = &mut second => panic!("the second command ended early"),
        () = tokio::time::sleep(Duration::from_millis(300)) => {}
    }

    let aborted = first.await.expect("joined").expect("answered");
    assert!(aborted.cancelled);

    let busy = chat
        .session()
        .bash("echo again")
        .await
        .expect_err("the second still runs");
    assert_eq!(busy.code(), IpcErrorCode::Unsupported);

    assert_eq!(chat.send(AgentAction::Abort).await, AgentReply::Ack);
    let output = second.await.expect("answered");
    assert!(output.cancelled);

    chat.stop().await;
}

#[tokio::test]
async fn a_move_whose_old_process_exited_meanwhile_is_refused() {
    let chat = Setup::new(&["claude_exit_later.jsonl", "claude_init_slow.jsonl"])
        .start()
        .await;

    let moved = chat.session().send(AgentAction::NewSession).await;
    assert!(matches!(moved, Err(BusinessError::Process(_))), "{moved:?}");

    let events = chat
        .wait("exit", |event| matches!(event, AgentEvent::Exited { .. }))
        .await;
    assert!(matches!(events, AgentEvent::Exited { .. }));

    let deadline = tokio::time::Instant::now() + agent_support::WAIT;
    while chat.spawner.exited() < 2 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the new process was never stopped"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let events = chat.events();
    let ready = events
        .iter()
        .filter(|event| matches!(event, AgentEvent::Ready { .. }))
        .count();
    assert_eq!(ready, 1, "the new process never opened: {events:#?}");
    assert!(matches!(events.last(), Some(AgentEvent::Exited { .. })), "{events:#?}");
    assert!(!chat.session().is_running());
}

#[tokio::test]
async fn prompts_past_the_open_bound_are_denied_at_once() {
    let chat = start("claude_many_asks.jsonl").await;
    chat.send(prompt("Read it all")).await;
    marker(&chat).await;

    assert_eq!(dialogs(&chat.events()), 256);

    // The scripted CLI saw the deny of the 257th.
    stop_cleanly(&chat).await;
}
