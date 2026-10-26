//! `!command`s and compaction of the OpenCode 2 chat engine against the scripted OpenCode 2 server:
//! quiet runs that never show as a turn.

use crate::opencode2_engine_helpers;
use crate::opencode2_engine_support;

use harness_business::model::{AgentAction, AgentEvent, AgentReply, BashOutput, IpcErrorCode, StreamingBehavior};
use opencode2_engine_helpers::user_texts;
use opencode2_engine_support::{count, kind, start, until_marker};
use pretty_assertions::assert_eq;

fn code(result: harness_business::Result<AgentReply>) -> IpcErrorCode {
    result.expect_err("the action is refused").code()
}

#[tokio::test]
async fn commands_run_in_the_session_shell_and_answer_with_output_and_exit_code() {
    let chat = start("opencode2_shell.jsonl").await;
    let session = chat.session();

    assert_eq!(
        session.bash("   ").await.expect_err("empty").code(),
        IpcErrorCode::InvalidInput
    );

    let output = session
        .bash("echo hi && ls")
        .await
        .expect("its end came after the answer");
    assert_eq!(
        output,
        BashOutput {
            output: "hi\ndot.png\nnotes.txt\n".to_owned(),
            exit_code: Some(0),
            cancelled: false,
            truncated: false,
        }
    );

    let password = chat.password();
    let secret = session.bash("printenv").await.expect("its end came before the answer");
    assert!(!secret.output.contains(&password), "{secret:?}");
    assert!(secret.output.contains("••••"), "{secret:?}");
    assert_eq!(secret.exit_code, Some(3));

    let events = until_marker(&chat).await;
    let shown: Vec<String> = events.iter().map(kind).collect();
    for hidden in [
        "agentStart",
        "settled",
        "messageStart",
        "messageEnd",
        "toolStart",
        "toolEnd",
    ] {
        assert!(!shown.iter().any(|kind| kind == hidden), "{hidden} in {shown:?}");
    }

    let agent = AgentAction::SetAgent {
        agent: "plan".to_owned(),
    };
    assert_eq!(
        session.send(agent).await.expect("idle once the commands ended"),
        AgentReply::Ack
    );
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}

#[tokio::test]
async fn compaction_runs_in_the_background_and_holds_prompts_until_it_ends() {
    let chat = start("opencode2_compact.jsonl").await;
    let session = chat.session();

    let instructed = AgentAction::Compact {
        instructions: Some("keep the API notes".to_owned()),
    };
    assert_eq!(code(session.send(instructed).await), IpcErrorCode::Unsupported);

    let compact = AgentAction::Compact { instructions: None };
    assert_eq!(
        session.send(compact.clone()).await.expect("compaction starts"),
        AgentReply::Ack
    );
    chat.wait(
        "the compact call",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Compacting")),
    )
    .await;

    let state = match session.send(AgentAction::GetState).await {
        Ok(AgentReply::State { state }) => state,
        other => panic!("state: {other:?}"),
    };
    assert!(state.is_compacting);
    assert!(!state.is_streaming);
    assert_eq!(code(session.send(compact).await), IpcErrorCode::Unsupported);

    let steer = AgentAction::Prompt {
        message: "after compaction".to_owned(),
        images: Vec::new(),
        streaming_behavior: Some(StreamingBehavior::Steer),
    };
    assert_eq!(
        session.send(steer).await.expect("held"),
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("queued".to_owned())
        },
        "steering waits while OpenCode compacts"
    );

    // The scenario's sync point: the prompt is held before the compaction runs.
    session.send(AgentAction::GetSessionStats).await.expect("stats");

    let events = until_marker(&chat).await;
    let compactions: Vec<&AgentEvent> = events.iter().filter(|event| kind(event) == "compaction").collect();
    assert_eq!(
        compactions,
        [
            &AgentEvent::Compaction {
                phase: "start".to_owned(),
                reason: Some("manual".to_owned()),
                error: None,
            },
            &AgentEvent::Compaction {
                phase: "end".to_owned(),
                reason: Some("manual".to_owned()),
                error: None,
            },
        ]
    );

    let shown: Vec<String> = events.iter().map(kind).collect();
    let end = shown.iter().rposition(|kind| kind == "compaction").expect("end");
    let first_start = shown
        .iter()
        .position(|kind| kind == "agentStart")
        .expect("the prompt's turn");
    assert!(end < first_start, "the summary's run is not a turn: {shown:?}");
    assert_eq!(count(&events, "agentStart"), 1);
    assert_eq!(count(&events, "settled"), 1);
    assert_eq!(user_texts(&events), ["after compaction"]);
    assert_eq!(
        count(&events, "textDelta"),
        1,
        "only the prompt's reply streams, not the summary"
    );
    assert!(session.is_running(), "every request matched the scenario");

    session.stop().await;
}
