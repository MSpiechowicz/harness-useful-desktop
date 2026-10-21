//! Slash commands of the OpenCode 2 chat engine against the scripted OpenCode 2 server: the server's
//! commands go to the command route, anything else as prompt text, a refused command fails like a
//! refused prompt, and MCP prompts are listed as such.

mod agent_support;
mod opencode2_engine_helpers;
mod opencode2_engine_support;

use harness_business::model::{AgentEvent, CommandSource, IpcErrorCode, PromptStatus};
use opencode2_engine_helpers::prompt;
use opencode2_engine_support::{Chat, count, start, text_prompt, until_marker};
use pretty_assertions::assert_eq;

/// Waits until `settled` arrived `times` times.
async fn settled(chat: &Chat, times: usize) {
    let deadline = tokio::time::Instant::now() + agent_support::WAIT;

    while count(&chat.events(), "settled") < times {
        assert!(
            tokio::time::Instant::now() < deadline,
            "never settled {times} times: {:#?}",
            chat.events()
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn server_commands_use_the_command_route_and_other_slashes_go_as_text() {
    let chat = start("opencode2_command.jsonl").await;

    let listed = chat
        .wait("commands", |event| matches!(event, AgentEvent::CommandsUpdated { .. }))
        .await;
    let AgentEvent::CommandsUpdated { commands } = &listed else {
        unreachable!();
    };
    let source = |name: &str| {
        commands
            .iter()
            .find(|command| command.name == name)
            .map(|command| command.source)
    };
    assert_eq!(source("review"), Some(CommandSource::Custom));
    assert_eq!(source("docs:search"), Some(CommandSource::McpPrompt), "{commands:#?}");

    prompt(&chat, "/nonexistent arg").await;
    settled(&chat, 1).await;

    let taken_at = prompt(&chat, "/review the parser").await;
    settled(&chat, 2).await;

    // OpenCode 2 admits a command before it runs: one it refuses fails like a refused prompt.
    let refused = chat
        .session()
        .send(text_prompt("/init"))
        .await
        .expect_err("OpenCode refused the command");
    assert_eq!(refused.code(), IpcErrorCode::Protocol);
    assert!(refused.to_string().contains("HTTP 400"), "{refused}");

    let events = until_marker(&chat).await;
    let results: Vec<PromptStatus> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::PromptResult { status, .. } => Some(*status),
            _ => None,
        })
        .collect();
    assert_eq!(results, [PromptStatus::Completed, PromptStatus::Completed]);
    assert_eq!(count(&events, "settled"), 2);

    let second_start = events
        .iter()
        .enumerate()
        .filter(|(_, event)| **event == AgentEvent::AgentStart)
        .nth(1)
        .map(|(index, _)| index)
        .expect("the command's turn");
    let taken_at = taken_at.lock().unwrap().expect("taken");
    assert!(taken_at <= second_start, "the command was taken when its turn started");

    let commands = chat.requests_to("POST /api/session/*/command");
    assert_eq!(commands.len(), 2);
    assert_eq!(
        commands[0].body,
        serde_json::json!({ "name": "review", "text": "the parser" })
    );
    assert!(chat.session().is_running(), "every request matched the scenario");

    chat.session().stop().await;
}
