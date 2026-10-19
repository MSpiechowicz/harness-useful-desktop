//! Session actions of the OpenCode chat engine against the scripted `opencode serve`: new chat,
//! fork, clone, rename, the refusals while the agent works and the context meter of a resumed
//! session.

mod agent_support;
mod opencode_engine_support;

use harness_business::model::{AgentAction, AgentEvent, AgentReply, ContextUsage, IpcErrorCode, SessionState};
use opencode_engine_support::{Chat, SESSION, count, kind, prompt, start, start_with, until_marker};
use pretty_assertions::assert_eq;

const FORK: &str = "ses_eaa55fce6ffeMnl5ZAKxvDBT61";
const CLONE: &str = "ses_eaa55fcdaffeh5ZC0QE665Y9mM";

fn code(result: harness_business::Result<AgentReply>) -> IpcErrorCode {
    result.expect_err("the action is refused").code()
}

fn fork(entry_id: &str) -> AgentAction {
    AgentAction::Fork {
        entry_id: entry_id.to_owned(),
    }
}

/// The states the chat emitted after startup.
fn states(chat: &Chat) -> Vec<SessionState> {
    chat.events()
        .into_iter()
        .skip(3)
        .filter_map(|event| match event {
            AgentEvent::State { state } => Some(state),
            _ => None,
        })
        .collect()
}

/// Waits for a `state` of session `id`.
async fn moved_to(chat: &Chat, id: &str) -> SessionState {
    let event = chat
        .wait(
            "state",
            |event| matches!(event, AgentEvent::State { state } if state.session_id == id),
        )
        .await;

    match event {
        AgentEvent::State { state } => state,
        _ => unreachable!(),
    }
}

async fn state(chat: &Chat) -> SessionState {
    match chat.session().send(AgentAction::GetState).await {
        Ok(AgentReply::State { state }) => state,
        other => panic!("state: {other:?}"),
    }
}

/// Waits until `settled` arrived once.
async fn settled(chat: &Chat) {
    chat.wait("settled", |event| matches!(event, AgentEvent::Settled { .. }))
        .await;
}

#[tokio::test]
async fn session_actions_wait_until_the_agent_is_idle() {
    let chat = start("opencode_new_session.jsonl").await;
    let session = chat.session();

    prompt(&chat, "Say hello").await;

    for action in [
        AgentAction::NewSession,
        AgentAction::Clone,
        fork("msg_155a9eeb3001SWCCbgpm0fXcQJ"),
        AgentAction::Compact { instructions: None },
    ] {
        assert_eq!(code(session.send(action).await), IpcErrorCode::Unsupported);
    }
    let busy = session.bash("ls").await.expect_err("refused while working");
    assert!(busy.to_string().contains("OpenCode is working"), "{busy}");

    // The sync point of the scenario: nothing above reached the server.
    session.send(AgentAction::GetSessionStats).await.expect("stats");
    settled(&chat).await;

    session.stop().await;
}

#[tokio::test]
async fn a_prompt_while_a_new_chat_is_created_wins_and_the_new_session_is_deleted() {
    let chat = start("opencode_new_session_race.jsonl").await;
    let session = chat.session();

    let prompt_meanwhile = async {
        chat.wait(
            "held",
            |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Held")),
        )
        .await;
        prompt(&chat, "Hurry").await;
    };
    let (moved, ()) = tokio::join!(session.send(AgentAction::NewSession), prompt_meanwhile);

    let refused = moved.expect_err("the move is refused");
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);
    assert!(refused.to_string().contains("the agent is working"), "{refused}");

    until_marker(&chat).await;
    assert!(
        states(&chat).iter().all(|state| state.session_id == SESSION),
        "the chat stays on its session"
    );
    assert!(chat.session().is_running(), "the new session was deleted");

    session.stop().await;
}

#[tokio::test]
async fn a_new_chat_moves_to_a_new_session_of_the_project() {
    let chat = start("opencode_new_session.jsonl").await;
    let session = chat.session();

    prompt(&chat, "Say hello").await;
    session.send(AgentAction::GetSessionStats).await.expect("stats");
    settled(&chat).await;

    assert_eq!(
        session.send(AgentAction::NewSession).await.expect("new chat"),
        AgentReply::Ack
    );

    let moved = moved_to(&chat, "ses_0new0chat0000000000000001").await;
    assert_eq!(moved.session_name, None, "OpenCode has not named it yet");
    assert_eq!(moved.agent.as_deref(), Some("build"));
    assert!(!moved.is_streaming);
    assert_eq!(state(&chat).await, moved);

    // The scenario requires the prompt in the new session.
    prompt(&chat, "Again").await;
    let renamed = AgentAction::SetSessionName {
        name: "Named".to_owned(),
    };
    assert_eq!(session.send(renamed).await.expect("rename"), AgentReply::Ack);

    let events = until_marker(&chat).await;
    let names: Vec<Option<String>> = states(&chat).into_iter().map(|state| state.session_name).collect();
    assert!(names.contains(&Some("Named".to_owned())), "{names:?}");
    assert_eq!(count(&events, "messageEnd"), 0, "the old session's reply is not shown");
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}

#[tokio::test]
async fn fork_and_clone_move_to_the_copy_without_showing_the_copied_messages() {
    let chat = start("opencode_fork.jsonl").await;
    let session = chat.session();

    assert_eq!(code(session.send(fork("prt_1")).await), IpcErrorCode::InvalidInput);
    assert_eq!(
        code(session.send(fork("msg_155a9e000001unknownMessage")).await),
        IpcErrorCode::NotFound
    );
    assert_eq!(
        code(session.send(fork("msg_155a9ebc0001sN2nswbVTUYQaw")).await),
        IpcErrorCode::InvalidInput,
        "a fork starts at a message the user wrote"
    );

    let forked = session
        .send(fork("msg_155a9eeb3001SWCCbgpm0fXcQJ"))
        .await
        .expect("fork");
    assert_eq!(
        forked,
        AgentReply::Ack,
        "like PI, the forked message's text is not returned"
    );

    let moved = moved_to(&chat, FORK).await;
    assert_eq!(moved.session_name.as_deref(), Some("Renamed chat (fork #1)"));
    assert_eq!(
        state(&chat).await.context_usage,
        Some(ContextUsage {
            tokens: 25,
            context_window: 100_000,
            percent: 0.025,
        }),
        "the copied reply sizes the context"
    );

    assert_eq!(session.send(AgentAction::Clone).await.expect("clone"), AgentReply::Ack);
    assert_eq!(state(&chat).await.session_id, CLONE);

    let events = until_marker(&chat).await;
    let shown: Vec<String> = events.iter().map(kind).collect();
    for hidden in ["messageStart", "messageEnd", "toolStart", "toolEnd", "agentStart"] {
        assert!(!shown.iter().any(|kind| kind == hidden), "{hidden} in {shown:?}");
    }
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}

#[tokio::test]
async fn switching_and_moving_are_refused() {
    let chat = start("opencode_fork.jsonl").await;
    let session = chat.session();

    let switch = AgentAction::SwitchSession {
        session: harness_business::model::SessionRef {
            harness: harness_business::model::HarnessId::OpenCode,
            id: SESSION.to_owned(),
        },
    };
    assert_eq!(code(session.send(switch).await), IpcErrorCode::Unsupported);

    let moved = session.move_project(&chat.project, &chat.project).await;
    assert_eq!(moved.expect_err("refused").code(), IpcErrorCode::Unsupported);

    session.stop().await;
}

#[tokio::test]
async fn a_resumed_session_reads_its_newest_reply_for_the_context_meter_once() {
    let chat = start_with("opencode_resume_context.jsonl", Some(SESSION), agent_support::options()).await;

    let expected = Some(ContextUsage {
        tokens: 25,
        context_window: 100_000,
        percent: 0.025,
    });
    assert_eq!(state(&chat).await.context_usage, expected);
    assert_eq!(state(&chat).await.context_usage, expected, "read once");
    assert!(chat.session().is_running(), "every request matched the scenario");

    chat.session().stop().await;
}

#[tokio::test]
async fn a_copy_in_another_directory_is_refused_and_left_alone() {
    let chat = start("opencode_clone_elsewhere.jsonl").await;
    let session = chat.session();

    assert_eq!(code(session.send(AgentAction::Clone).await), IpcErrorCode::Protocol);
    let rename = AgentAction::SetSessionName {
        name: "Named".to_owned(),
    };
    session
        .send(rename)
        .await
        .expect("the rename is the next request, no delete before it");

    until_marker(&chat).await;
    assert!(session.is_running(), "every request matched the scenario");
    session.stop().await;
}
