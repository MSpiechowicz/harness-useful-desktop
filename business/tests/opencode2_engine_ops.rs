//! Session actions of the OpenCode 2 chat engine against the scripted OpenCode 2 server: new chat,
//! fork, clone, rename, the refusals while the agent works, switching and moving.

mod agent_support;
mod opencode2_engine_helpers;
mod opencode2_engine_support;

use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, ContextUsage, HarnessId, IpcErrorCode, SessionRef, SessionState, TokenUsage,
};
use opencode2_engine_helpers::prompt;
use opencode2_engine_support::{Chat, SESSION, count, kind, start, until_marker};
use pretty_assertions::assert_eq;

const NEW: &str = "ses_0new0chat0000000000000001";
const FORK: &str = "ses_ea27da712ffe14Nxq8YVmHrHyr";
const CLONE: &str = "ses_ea27da70fffea5dJXkjhzIlcBu";

/// The second prompt of the recorded session (`ops.jsonl`) and the reply before it.
const SECOND_PROMPT: &str = "msg_15d8253a80015RYHUZtV7CNAfL";
const FIRST_REPLY: &str = "msg_15d82448a0010Nd2YPKniIjq4b";

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
    let chat = start("opencode2_shell_busy.jsonl").await;
    let session = chat.session();

    prompt(&chat, "Say hello").await;

    for action in [
        AgentAction::NewSession,
        AgentAction::Clone,
        fork(SECOND_PROMPT),
        AgentAction::Compact { instructions: None },
    ] {
        let refused = session.send(action).await.expect_err("refused while working");
        assert_eq!(refused.code(), IpcErrorCode::Unsupported);
        assert!(refused.to_string().contains("the agent is working"), "{refused}");
    }
    let busy = session.bash("ls").await.expect_err("refused while working");
    assert!(busy.to_string().contains("OpenCode is working"), "{busy}");

    // The sync point of the scenario: nothing above reached the server.
    session.send(AgentAction::GetSessionStats).await.expect("stats");
    settled(&chat).await;
    until_marker(&chat).await;
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}

#[tokio::test]
async fn a_new_chat_moves_to_a_new_session_with_the_same_agent_and_model() {
    let chat = start("opencode2_new_session.jsonl").await;
    let session = chat.session();

    prompt(&chat, "Say hello").await;
    session.send(AgentAction::GetSessionStats).await.expect("stats");
    settled(&chat).await;

    assert_eq!(
        session.send(AgentAction::NewSession).await.expect("new chat"),
        AgentReply::Ack
    );

    let moved = moved_to(&chat, NEW).await;
    assert_eq!(moved.session_name, None, "OpenCode has not named it yet");
    assert_eq!(moved.agent.as_deref(), Some("build"));
    assert_eq!(moved.model.map(|model| model.id).as_deref(), Some("big-pickle"));
    assert!(!moved.is_streaming);
    assert_eq!(moved.context_usage, None);
    assert_eq!(state(&chat).await.session_id, NEW);

    // The scenario requires the prompt and the rename in the new session.
    prompt(&chat, "Again").await;
    let rename = AgentAction::SetSessionName {
        name: "Named".to_owned(),
    };
    assert_eq!(session.send(rename).await.expect("rename"), AgentReply::Ack);

    let events = until_marker(&chat).await;
    let names: Vec<Option<String>> = states(&chat).into_iter().map(|state| state.session_name).collect();
    assert!(names.contains(&Some("Named".to_owned())), "{names:?}");
    assert_eq!(count(&events, "textDelta"), 0, "the old session's reply is not shown");
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}

#[tokio::test]
async fn a_prompt_while_a_new_chat_is_created_wins_and_the_new_session_is_deleted() {
    let chat = start("opencode2_new_session_race.jsonl").await;
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
    assert_eq!(chat.requests_to("DELETE /api/session/*").len(), 1);
    assert!(session.is_running(), "the new session was deleted as scripted");

    session.stop().await;
}

#[tokio::test]
async fn fork_and_clone_move_to_the_copy_without_showing_the_copied_messages() {
    let chat = start("opencode2_fork.jsonl").await;
    let session = chat.session();

    assert_eq!(code(session.send(fork("prt_1")).await), IpcErrorCode::InvalidInput);
    assert_eq!(
        code(session.send(fork("msg_15d8253a80015unknownMessage")).await),
        IpcErrorCode::NotFound
    );
    assert_eq!(
        code(session.send(fork(FIRST_REPLY)).await),
        IpcErrorCode::InvalidInput,
        "a fork starts at a message the user wrote"
    );

    let forked = session.send(fork(SECOND_PROMPT)).await.expect("fork");
    assert_eq!(
        forked,
        AgentReply::Ack,
        "like PI, the forked message's text is not returned"
    );

    let moved = moved_to(&chat, FORK).await;
    assert_eq!(moved.session_name.as_deref(), Some("Recorded title (fork #1)"));
    assert_eq!(
        state(&chat).await.context_usage,
        Some(ContextUsage {
            tokens: 3686 + 4 + 1710,
            context_window: 200_000,
            percent: 2.7,
        }),
        "the copied reply sizes the context"
    );

    // OpenCode 2 starts the copy's own totals at zero: as on OpenCode 1, the copied messages count.
    let stats = match session.send(AgentAction::GetSessionStats).await {
        Ok(AgentReply::Stats { stats }) => stats,
        other => panic!("stats: {other:?}"),
    };
    assert_eq!(
        (stats.user_messages, stats.assistant_messages),
        (1, 1),
        "the first prompt and its reply"
    );
    assert_eq!(
        stats.tokens,
        TokenUsage {
            input: 3686,
            output: 4,
            cache_read: 1710,
            cache_write: 0,
            total: 3686 + 4 + 1710,
        }
    );
    assert_eq!(stats.cost_usd, Some(0.0));

    assert_eq!(session.send(AgentAction::Clone).await.expect("clone"), AgentReply::Ack);
    assert_eq!(state(&chat).await.session_id, CLONE);
    assert_eq!(
        session
            .send(AgentAction::GetLastAssistantText)
            .await
            .expect("last reply"),
        AgentReply::Text {
            text: Some("One.".to_owned())
        },
        "the clone holds the copied reply"
    );

    let events = until_marker(&chat).await;
    let shown: Vec<String> = events.iter().map(kind).collect();
    for hidden in ["messageStart", "messageEnd", "textDelta", "agentStart"] {
        assert!(!shown.iter().any(|kind| kind == hidden), "{hidden} in {shown:?}");
    }

    let forks = chat.requests_to("POST /api/session/*/fork");
    assert_eq!(forks.len(), 2);
    assert_eq!(forks[0].body, serde_json::json!({ "before": SECOND_PROMPT }));
    assert_eq!(forks[1].body, serde_json::json!({}), "a clone copies every message");
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}

#[tokio::test]
async fn a_copy_in_another_directory_is_refused_and_left_alone() {
    let chat = start("opencode2_clone_elsewhere.jsonl").await;
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
    assert_eq!(state(&chat).await.session_id, SESSION);
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}

#[tokio::test]
async fn switching_and_moving_are_refused() {
    let chat = start("opencode2_clone_elsewhere.jsonl").await;
    let session = chat.session();

    let switch = AgentAction::SwitchSession {
        session: SessionRef {
            harness: HarnessId::OpenCode,
            id: SESSION.to_owned(),
        },
    };
    assert_eq!(code(session.send(switch).await), IpcErrorCode::Unsupported);

    let moved = session.move_project(&chat.project, &chat.project).await;
    assert_eq!(moved.expect_err("refused").code(), IpcErrorCode::Unsupported);

    session.stop().await;
}
