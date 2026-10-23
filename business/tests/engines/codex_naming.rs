//! AI naming with Codex: the helper chat's ephemeral thread never asks and cannot write, and every
//! MCP server Codex lists for the project (`config/read`) is switched off for it. A helper whose
//! servers Codex does not list does not start. A prompt that fails on the configured model goes
//! once more on Codex's default model before the chat settles, and one that the configured model
//! answers settles the chat once.

use crate::codex_engine_support;

use codex_engine_support::{replies, start_as};
use harness_business::BusinessError;
use harness_business::model::{AgentAction, AgentEvent, PromptStatus};
use pretty_assertions::assert_eq;

const PROMPT: &str =
    "Give a 3-word title for a chat that starts with: 'create hello.txt and list files'. Reply with the title only.";

#[tokio::test]
async fn a_failed_model_is_retried_once_on_the_default_model_before_the_helper_settles() {
    // The servers Codex lists are switched off for the helper's thread, and a server only the
    // app's own reading of config.toml has is not named (the scenario checks both).
    let helper = codex_engine_support::Setup {
        utility: true,
        ..codex_engine_support::Setup::default()
    };
    let chat = codex_engine_support::start_with("codex_engine_naming.jsonl", helper, |project| {
        let config = project.join(codex_engine_support::HOME).join("config.toml");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(config, "[mcp_servers.local-only]\ncommand = \"local-server\"\n").unwrap();
    })
    .await;

    chat.act(codex_engine_support::prompt(PROMPT)).await;
    let events = chat.settled(1).await;

    let results: Vec<PromptStatus> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::PromptResult { status, .. } => Some(*status),
            _ => None,
        })
        .collect();
    assert_eq!(results, [PromptStatus::Error, PromptStatus::Completed]);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::Settled { .. }))
            .count(),
        1,
        "the failed try does not settle the helper"
    );
    assert_eq!(replies(&events), ["Create Hello File"]);

    let refused = chat.session.send(AgentAction::Clone).await.unwrap_err();
    assert!(
        matches!(refused, BusinessError::Unsupported(_)),
        "a helper has no saved session"
    );

    chat.finish().await;
}

#[tokio::test]
async fn a_helper_whose_model_answers_settles_once_with_the_answer() {
    let chat = start_as("codex_engine_naming_ok.jsonl", None, true).await;

    chat.act(codex_engine_support::prompt(PROMPT)).await;
    let events = chat.settled(1).await;

    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::PromptResult {
            status: PromptStatus::Completed,
            ..
        }
    )));
    assert_eq!(replies(&events), ["Create Hello File"]);

    let events = chat.finish().await;
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::Settled { .. }))
            .count(),
        1
    );
}

#[tokio::test]
async fn a_helper_whose_servers_codex_does_not_list_does_not_start() {
    let launched = codex_engine_support::launch_as("codex_engine_naming_unread.jsonl", None, true).await;

    let Err(error) = launched.result else {
        panic!("the helper started with servers it could not switch off");
    };
    assert!(error.to_string().contains("MCP servers"), "{error}");
}
