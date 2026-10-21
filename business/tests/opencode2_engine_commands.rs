//! Slash commands of the OpenCode 2 chat engine against the scripted OpenCode 2 server, as on
//! OpenCode 1: the server's commands go to the command route, its skills as prompts with the skill
//! attached, anything else as prompt text; a refused command or skill ends with an error result.
//! MCP prompts and skills are listed as such.

mod agent_support;
mod opencode2_engine_helpers;
mod opencode2_engine_support;

use harness_business::model::{AgentEvent, CommandSource, PromptStatus};
use opencode2_engine_helpers::{prompt, user_texts};
use opencode2_engine_support::{Chat, count, start, until_marker};
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

    // As on OpenCode 1, a command goes in the background: one OpenCode refuses was taken, then
    // fails with an error result and the chat settles.
    let init_taken_at = prompt(&chat, "/init").await;
    settled(&chat, 3).await;

    let events = until_marker(&chat).await;
    let results: Vec<(PromptStatus, Option<String>)> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::PromptResult { status, error, .. } => Some((*status, error.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        results,
        [
            (PromptStatus::Completed, None),
            (PromptStatus::Completed, None),
            (
                PromptStatus::Error,
                Some("protocol error: the OpenCode server answered HTTP 400".to_owned())
            ),
        ]
    );
    assert_eq!(
        count(&events, "settled"),
        3,
        "a command that fails before its turn settles alone"
    );

    let init_failed = events
        .iter()
        .position(|event| {
            matches!(
                event,
                AgentEvent::PromptResult {
                    status: PromptStatus::Error,
                    ..
                }
            )
        })
        .expect("the failed command's result");
    let init_taken_at = init_taken_at
        .lock()
        .unwrap()
        .expect("a command OpenCode answered was taken");
    assert!(
        init_taken_at <= init_failed,
        "taken, then its error result, then settled"
    );

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

#[tokio::test]
async fn skills_are_listed_and_run_as_prompts_with_the_skill_attached() {
    let chat = start("opencode2_skill.jsonl").await;

    let listed = chat
        .wait("commands", |event| matches!(event, AgentEvent::CommandsUpdated { .. }))
        .await;
    let AgentEvent::CommandsUpdated { commands } = &listed else {
        unreachable!();
    };
    let listed = |name: &str| {
        commands
            .iter()
            .find(|command| command.name == name)
            .map(|command| (command.source, command.description.clone()))
    };
    assert_eq!(
        listed("greet"),
        Some((CommandSource::Skill, "Greets the user warmly".to_owned()))
    );
    assert_eq!(
        listed("review").map(|(source, _)| source),
        Some(CommandSource::Custom),
        "a command wins over a skill of the same name"
    );

    prompt(&chat, "/greet be brief").await;
    settled(&chat, 1).await;

    prompt(&chat, "/greet").await;
    settled(&chat, 2).await;

    let taken_at = prompt(&chat, "/greet").await;
    settled(&chat, 3).await;

    let events = until_marker(&chat).await;
    let results: Vec<(PromptStatus, Option<String>)> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::PromptResult { status, error, .. } => Some((*status, error.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        results,
        [
            (PromptStatus::Completed, None),
            (PromptStatus::Completed, None),
            (
                PromptStatus::Error,
                Some("protocol error: the OpenCode server answered HTTP 400".to_owned())
            ),
        ],
        "a skill OpenCode refuses fails like a refused command"
    );
    assert_eq!(
        user_texts(&events),
        ["/greet be brief", "/greet"],
        "the user's message names the skill, also with no text after it"
    );

    let failed = events
        .iter()
        .position(|event| {
            matches!(
                event,
                AgentEvent::PromptResult {
                    status: PromptStatus::Error,
                    ..
                }
            )
        })
        .expect("the refused skill's result");
    let taken_at = taken_at.lock().unwrap().expect("a skill OpenCode answered was taken");
    assert!(taken_at <= failed, "taken, then its error result");

    let prompts: Vec<serde_json::Value> = chat
        .requests_to("POST /api/session/*/prompt")
        .into_iter()
        .map(|request| request.body)
        .collect();
    assert_eq!(
        prompts,
        [
            serde_json::json!({ "text": "be brief", "skills": [{ "id": "greet" }] }),
            serde_json::json!({ "text": "", "skills": [{ "id": "greet" }] }),
            serde_json::json!({ "text": "", "skills": [{ "id": "greet" }] }),
        ]
    );
    assert!(chat.requests_to("POST /api/session/*/command").is_empty());
    assert!(chat.session().is_running(), "every request matched the scenario");

    chat.session().stop().await;
}
