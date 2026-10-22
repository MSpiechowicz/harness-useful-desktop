//! The model and reasoning pickers of a Codex chat (Verified fact 2): a configured model Codex
//! refuses fails the turn with its error in the chat, the picker offers `model/list` with Codex's
//! default, and a picked model and effort go with the next turn.

use crate::codex_engine_support;

use codex_engine_support::{prompt, replies, start};
use harness_business::BusinessError;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, PromptStatus};
use pretty_assertions::assert_eq;

#[tokio::test]
async fn an_unusable_model_shows_its_error_and_a_picked_one_runs_the_next_turn() {
    let chat = start("codex_engine_model.jsonl").await;

    let AgentReply::State { state } = chat.act(AgentAction::GetState).await else {
        panic!("not a state");
    };
    assert_eq!(state.model.map(|model| model.id).as_deref(), Some("gpt-6-astra"));

    chat.act(prompt("Hello?")).await;
    let events = chat.settled(1).await;
    let failure = events.iter().find_map(|event| match event {
        AgentEvent::PromptResult {
            status: PromptStatus::Error,
            error,
            ..
        } => error.clone(),
        _ => None,
    });
    assert_eq!(
        failure.as_deref(),
        Some("The 'gpt-6-astra' model is not supported when using Codex with a ChatGPT account.")
    );

    let AgentReply::Models { models } = chat.act(AgentAction::GetAvailableModels).await else {
        panic!("not models");
    };
    let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
    assert_eq!(ids, ["gpt-5.6-terra", "gpt-5.6-luna"]);
    assert_eq!(models[0].name.as_deref(), Some("GPT-5.6-Terra"));
    assert_eq!(models[0].input, ["text", "image"]);

    let unknown = chat
        .session
        .send(AgentAction::SetModel {
            provider: "openai".to_owned(),
            model_id: "gpt-6-astra".to_owned(),
        })
        .await
        .unwrap_err();
    assert!(matches!(unknown, BusinessError::InvalidInput(_)), "{unknown:?}");

    chat.act(AgentAction::SetModel {
        provider: "openai".to_owned(),
        model_id: "gpt-5.6-terra".to_owned(),
    })
    .await;
    chat.wait(
        "the default effort",
        |event| matches!(event, AgentEvent::ThinkingChanged { level } if level == "medium"),
    )
    .await;

    let AgentReply::ThinkingLevels { levels } = chat.act(AgentAction::GetAvailableThinkingLevels).await else {
        panic!("not levels");
    };
    assert_eq!(levels, ["low", "medium", "high"]);
    chat.act(AgentAction::SetThinkingLevel {
        level: "high".to_owned(),
    })
    .await;

    chat.act(prompt("Hello again?")).await;
    let events = chat.settled(2).await;
    assert_eq!(replies(&events).last().map(String::as_str), Some("Hello from terra."));

    let AgentReply::State { state } = chat.act(AgentAction::GetState).await else {
        panic!("not a state");
    };
    assert_eq!(state.model.map(|model| model.id).as_deref(), Some("gpt-5.6-terra"));
    assert_eq!(state.thinking_level.as_deref(), Some("high"));

    chat.finish().await;
}

#[tokio::test]
async fn a_model_list_codex_does_not_send_in_time_leaves_the_picker_empty() {
    let setup = codex_engine_support::Setup {
        startup_timeout: Some(std::time::Duration::from_millis(500)),
        ..codex_engine_support::Setup::default()
    };
    let chat = codex_engine_support::start_with("codex_engine_models_late.jsonl", setup, |_| {}).await;

    chat.wait("the chat opened", |event| matches!(event, AgentEvent::Ready { .. }))
        .await;
    chat.finish().await;
}
