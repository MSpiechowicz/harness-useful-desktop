//! Stopping a Claude Code subagent from the chat: `stop_task` for a running one, nothing for one
//! that finished, `NotFound` for one the chat never saw, and no message path (the orchestrator
//! takes messages).

use crate::claude_engine_support;

use claude_engine_support::{Setup, stop_cleanly};
use harness_business::model::{AgentAction, AgentEvent, AgentReply, IpcErrorCode, SubagentStatus};
use pretty_assertions::assert_eq;

fn subagent_status(event: &AgentEvent, id: &str) -> Option<SubagentStatus> {
    match event {
        AgentEvent::Subagent { subagent } if subagent.id == id => Some(subagent.status),
        _ => None,
    }
}

#[tokio::test]
async fn stopping_a_running_subagent_asks_the_cli_to_stop_its_task() {
    let chat = Setup::new(&["claude_subagents.jsonl"]).start().await;
    chat.wait("the finished subagent", |event| {
        subagent_status(event, "t-done") == Some(SubagentStatus::Completed)
    })
    .await;
    chat.wait("the running subagent", |event| {
        subagent_status(event, "t-live") == Some(SubagentStatus::Running)
    })
    .await;

    let finished = AgentAction::StopSubagent {
        id: "t-done".to_owned(),
    };
    assert_eq!(chat.send(finished).await, AgentReply::Ack);

    let running = AgentAction::StopSubagent {
        id: "t-live".to_owned(),
    };
    assert_eq!(chat.send(running).await, AgentReply::Ack);

    // The CLI only reports the stop when it got `stop_task` for exactly this task.
    chat.wait("the stopped subagent", |event| {
        subagent_status(event, "t-live") == Some(SubagentStatus::Aborted)
    })
    .await;

    stop_cleanly(&chat).await;
}

#[tokio::test]
async fn a_subagent_the_chat_never_saw_is_not_found_and_messages_have_no_path() {
    let chat = Setup::new(&["claude_subagents.jsonl"]).start().await;
    let unknown = chat
        .session()
        .send(AgentAction::StopSubagent {
            id: "t-other".to_owned(),
        })
        .await
        .unwrap_err();
    assert_eq!(unknown.code(), IpcErrorCode::NotFound);

    let message = chat
        .session()
        .send(AgentAction::MessageSubagent {
            id: "t-live".to_owned(),
            message: "hello".to_owned(),
        })
        .await
        .unwrap_err();
    assert_eq!(message.code(), IpcErrorCode::Unsupported);

    stop_cleanly(&chat).await;
}
