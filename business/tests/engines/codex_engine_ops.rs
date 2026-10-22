//! The Codex chat's session operations against the scripted app server: clone, switch and new
//! chat as threads of the same process, a fork cut at a message, renaming, compaction, a
//! `!command` and its abort, and skills as slash commands.

use crate::codex_engine_support;

use codex_engine_support::{CodexChat, Setup, THREAD, prompt, start, start_as, start_with};
use harness_business::BusinessError;
use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, ApprovalMode, CommandInvocation, CommandSource, HarnessId, PromptStatus,
    SessionRef, SessionState,
};
use pretty_assertions::assert_eq;

/// The threads the scenarios move to.
const CLONE: &str = "01a164a2-3333-7000-8000-000000000003";
const SAVED: &str = "01a164a2-4444-7000-8000-000000000004";
const NEW: &str = "01a164a2-1111-7000-8000-000000000001";
const FORK: &str = "01a164a2-2222-7000-8000-000000000002";

/// The first and second turn of the saved history.
const FIRST_TURN: &str = "01a164a0-4c86-7902-8de6-44d0f7bc8ac7";

/// A chat in Write, where `!command`s run (the scenarios' Codex answers with Always ask's policy).
fn writes() -> Setup {
    Setup {
        approval_override: Some(ApprovalMode::Write),
        ..Setup::default()
    }
}

/// Waits for a `state` of thread `id` with `messages` messages counted.
async fn state_of(chat: &CodexChat, id: &str, messages: u32) -> SessionState {
    let shown = chat
        .wait("the thread's state", |event| {
            matches!(event, AgentEvent::State { state } if state.session_id == id && state.message_count == messages)
        })
        .await;

    match shown {
        AgentEvent::State { state } => state,
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn clone_switch_and_new_chat_move_the_chat_to_threads_of_the_same_process() {
    let chat = start("codex_engine_ops.jsonl").await;

    let refused = chat.session.send(AgentAction::Clone).await.unwrap_err();
    assert!(matches!(&refused, BusinessError::Unsupported(reason) if reason.contains("Send a message first")));

    chat.act(prompt("Ask me about colours.")).await;
    chat.settled(1).await;

    assert_eq!(chat.act(AgentAction::Clone).await, AgentReply::Ack);
    state_of(&chat, CLONE, 2).await;

    let AgentReply::Stats { stats } = chat.act(AgentAction::GetSessionStats).await else {
        panic!("not stats");
    };
    assert_eq!((stats.user_messages, stats.assistant_messages), (1, 1));

    let saved = SessionRef {
        harness: HarnessId::Codex,
        id: SAVED.to_owned(),
    };
    chat.act(AgentAction::SwitchSession { session: saved.clone() }).await;
    let state = state_of(&chat, SAVED, 6).await;
    assert_eq!(state.session_name.as_deref(), Some("Saved elsewhere"));

    // Already there: nothing is asked.
    assert_eq!(
        chat.act(AgentAction::SwitchSession { session: saved }).await,
        AgentReply::Ack
    );

    let elsewhere = SessionRef {
        harness: HarnessId::Claude,
        id: SAVED.to_owned(),
    };
    let refused = chat
        .session
        .send(AgentAction::SwitchSession { session: elsewhere })
        .await
        .unwrap_err();
    assert!(matches!(refused, BusinessError::InvalidInput(_)));

    chat.act(AgentAction::NewSession).await;
    let AgentReply::State { state } = chat.act(AgentAction::GetState).await else {
        panic!("not a state");
    };
    assert_eq!((state.session_id.as_str(), state.message_count), (NEW, 0));
    assert_eq!(state.session_name, None);

    assert_eq!(
        chat.spawner.requested().len(),
        1,
        "every thread runs in the one app server"
    );
    chat.finish().await;
}

#[tokio::test]
async fn a_fork_keeps_the_turns_up_to_the_message_it_forks_at() {
    let chat = start_as("codex_engine_fork.jsonl", Some(THREAD), false).await;
    state_of(&chat, THREAD, 6).await;

    let unknown = AgentAction::Fork {
        entry_id: "01a164a0-0000-7000-8000-00000000beef".to_owned(),
    };
    assert!(matches!(
        chat.session.send(unknown).await,
        Err(BusinessError::NotFound(_))
    ));

    let fork = AgentAction::Fork {
        entry_id: FIRST_TURN.to_owned(),
    };
    assert_eq!(chat.act(fork).await, AgentReply::Ack);

    let state = state_of(&chat, FORK, 2).await;
    assert_eq!(state.session_name.as_deref(), Some("Recorded thread"));
    assert_eq!(state.thinking_level.as_deref(), Some("high"));

    chat.finish().await;
}

#[tokio::test]
async fn rename_compaction_and_a_command_run_on_the_thread() {
    let chat = start_with("codex_engine_runs.jsonl", writes(), |_| {}).await;

    let blank = AgentAction::SetSessionName { name: "  ".to_owned() };
    assert!(matches!(
        chat.session.send(blank).await,
        Err(BusinessError::InvalidInput(_))
    ));

    let rename = AgentAction::SetSessionName {
        name: " Colour talk ".to_owned(),
    };
    assert_eq!(chat.act(rename).await, AgentReply::Ack);
    chat.wait(
        "the new title",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Colour talk")),
    )
    .await;

    let instructed = AgentAction::Compact {
        instructions: Some("Keep the colours.".to_owned()),
    };
    assert!(matches!(
        chat.session.send(instructed).await,
        Err(BusinessError::Unsupported(_))
    ));

    chat.act(AgentAction::Compact { instructions: None }).await;
    chat.wait("the compaction's turn end", |event| {
        matches!(
            event,
            AgentEvent::PromptResult {
                status: PromptStatus::Handled,
                ..
            }
        )
    })
    .await;
    assert!(chat.events().iter().any(|event| matches!(
        event,
        AgentEvent::Compaction { phase, reason, .. } if phase == "end" && reason.as_deref() == Some("manual")
    )));

    let output = chat.session.bash("echo from-shell").await.expect("the command runs");
    assert_eq!(output.output, "from-shell\n");
    assert_eq!(output.exit_code, Some(0));
    assert!(!output.cancelled);

    let events = chat.finish().await;
    assert!(
        !events.iter().any(|event| matches!(event, AgentEvent::ToolStart { .. })),
        "the command shows no tool card of its own"
    );

    // Each quiet turn ends with the chat's state (no `settled` asks for it), and none streams.
    let ends: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| matches!(event, AgentEvent::PromptResult { .. }))
        .map(|(index, _)| index)
        .collect();
    assert!(!ends.is_empty());
    assert!(
        matches!(events.get(ends[0] + 1), Some(AgentEvent::State { .. })),
        "a state follows the compaction"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::State { state } if state.is_streaming))
    );
}

#[tokio::test]
async fn an_abort_cancels_a_running_command() {
    let chat = start_with("codex_engine_bash_abort.jsonl", writes(), |_| {}).await;

    let stop = async {
        // The command's quiet turn shows no events and streams nothing (its state says so too):
        // the first poll of `bash` already counts the turn, so a moment later the stop reaches it.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let AgentReply::State { state } = chat.act(AgentAction::GetState).await else {
            panic!("not a state");
        };
        assert!(!state.is_streaming, "a quiet turn does not stream");

        chat.act(AgentAction::Abort).await
    };
    let (ran, stopped) = tokio::join!(chat.session.bash("sleep 60"), stop);

    assert_eq!(stopped, AgentReply::Ack);
    let output = ran.expect("the command ends");
    assert!(output.cancelled);
    assert_eq!((output.output.as_str(), output.exit_code), ("", None));

    chat.finish().await;
}

#[tokio::test]
async fn skills_are_slash_commands_and_a_called_skill_goes_with_the_message() {
    let chat = start("codex_engine_skills.jsonl").await;

    let commands = chat
        .wait("the skills", |event| {
            matches!(event, AgentEvent::CommandsUpdated { commands } if commands.iter().any(|command| command.name == "brandkit"))
        })
        .await;
    let AgentEvent::CommandsUpdated { commands } = commands else {
        unreachable!()
    };

    let skill = commands.iter().find(|command| command.name == "brandkit").unwrap();
    assert_eq!(skill.source, CommandSource::Skill);
    assert_eq!(skill.invocation, CommandInvocation::PromptText);
    assert!(
        !commands.iter().any(|command| command.name == "sleepy"),
        "a disabled skill"
    );

    chat.act(prompt("/brandkit a tea shop")).await;
    chat.settled(1).await;

    chat.finish().await;
}
