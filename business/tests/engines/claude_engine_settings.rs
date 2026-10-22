//! The model and thinking pickers and the slash commands of the Claude Code chat engine against
//! the scripted `claude` (recorded control replies of controls.jsonl and a crafted handshake).

use crate::agent_support;
use crate::claude_engine_support;

use claude_engine_support::{Setup, argv, prompt, start};
use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, CommandSource, CommandSpec, IpcErrorCode, ModelOption,
};
use pretty_assertions::assert_eq;

fn set_model(id: &str) -> AgentAction {
    AgentAction::SetModel {
        provider: "anthropic".to_owned(),
        model_id: id.to_owned(),
    }
}

fn thinking(level: &str) -> AgentAction {
    AgentAction::SetThinkingLevel {
        level: level.to_owned(),
    }
}

fn source_of(commands: &[CommandSpec], name: &str) -> Option<CommandSource> {
    commands
        .iter()
        .find(|command| command.name == name)
        .map(|command| command.source)
}

/// The newest `commandsUpdated` catalog.
fn last_catalog(events: &[AgentEvent]) -> Vec<CommandSpec> {
    events
        .iter()
        .rev()
        .find_map(|event| match event {
            AgentEvent::CommandsUpdated { commands } => Some(commands.clone()),
            _ => None,
        })
        .expect("a catalog")
}

#[tokio::test]
async fn a_new_session_starts_on_the_picked_model_as_one_argument() {
    let chat = Setup::new(&["claude_model_pick.jsonl", "claude_startup.jsonl"])
        .start()
        .await;

    assert_eq!(chat.send(set_model("sonnet")).await, AgentReply::Ack);
    assert_eq!(chat.send(AgentAction::NewSession).await, AgentReply::Ack);
    let deadline = tokio::time::Instant::now() + agent_support::WAIT;
    while chat.spawner.requested().len() < 2 {
        assert!(tokio::time::Instant::now() < deadline, "no second process");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let first = argv(&chat, 0);
    let second = argv(&chat, 1);
    assert!(!first.iter().any(|arg| arg.starts_with("--model")), "{first:?}");
    assert!(second.contains(&"--model=sonnet".to_owned()), "{second:?}");
    assert!(!second.contains(&"--model".to_owned()), "{second:?}");

    chat.stop().await;
}

#[tokio::test]
async fn the_pickers_switch_the_model_and_its_thinking_level() {
    let chat = start("claude_controls.jsonl").await;
    let session = chat.session();

    let AgentReply::Models { models } = chat.send(AgentAction::GetAvailableModels).await else {
        panic!("models")
    };
    assert_eq!(
        models.iter().map(|model| model.id.as_str()).collect::<Vec<_>>(),
        ["default", "sonnet", "claude-opus-5"],
        "list_models replaces the handshake's list"
    );
    assert_eq!(
        models[1],
        ModelOption {
            provider: "anthropic".to_owned(),
            id: "sonnet".to_owned(),
            name: Some("Sonnet 5.5".to_owned()),
            reasoning: true,
            context_window: None,
            max_tokens: None,
            input: Vec::new(),
        }
    );

    assert_eq!(chat.send(set_model("sonnet")).await, AgentReply::Ack);
    chat.wait(
        "model",
        |event| matches!(event, AgentEvent::ModelChanged { model: Some(model) } if model.id == "sonnet"),
    )
    .await;
    chat.wait("set model output", |event| {
        matches!(event, AgentEvent::Notice { message, .. } | AgentEvent::CommandOutput { text: message } if message.contains("Set model to"))
    })
    .await;

    assert_eq!(
        chat.send(AgentAction::GetAvailableThinkingLevels).await,
        AgentReply::ThinkingLevels {
            levels: ["low", "medium", "high", "xhigh", "max"].map(str::to_owned).to_vec()
        }
    );
    assert_eq!(chat.send(thinking("low")).await, AgentReply::Ack);
    let refused = session
        .send(thinking("off"))
        .await
        .expect_err("sonnet cannot turn thinking off");
    assert_eq!(refused.code(), IpcErrorCode::InvalidInput);

    assert_eq!(chat.send(set_model("claude-opus-5")).await, AgentReply::Ack);
    assert_eq!(
        chat.send(AgentAction::GetAvailableThinkingLevels).await,
        AgentReply::ThinkingLevels {
            levels: ["off", "low", "medium", "high"].map(str::to_owned).to_vec()
        }
    );
    assert_eq!(chat.send(thinking("off")).await, AgentReply::Ack);
    assert_eq!(chat.send(thinking("high")).await, AgentReply::Ack);

    let AgentReply::State { state } = chat.send(AgentAction::GetState).await else {
        panic!("state")
    };
    assert_eq!(state.model.map(|model| model.id).as_deref(), Some("claude-opus-5"));
    assert_eq!(state.thinking_level.as_deref(), Some("high"));

    assert_eq!(chat.send(set_model("sonnet")).await, AgentReply::Ack);
    let refused = session
        .send(set_model("default"))
        .await
        .expect_err("Claude Code refuses it");
    assert_eq!(refused.code(), IpcErrorCode::Protocol);
    assert!(refused.to_string().contains("Model not available"), "{refused}");

    let unknown = session.send(set_model("gpt-5")).await.expect_err("not offered");
    assert_eq!(unknown.code(), IpcErrorCode::InvalidInput);

    chat.send(prompt("Reply with exactly: two")).await;
    let events = chat.settled(1).await;

    let thinking_changes: Vec<String> = events
        .into_iter()
        .filter_map(|event| match event {
            AgentEvent::ThinkingChanged { level } => Some(level),
            _ => None,
        })
        .collect();
    assert_eq!(thinking_changes, ["low", "off", "high"]);

    let AgentReply::State { state } = chat.send(AgentAction::GetState).await else {
        panic!("state")
    };
    assert_eq!(
        state.model.map(|model| model.id).as_deref(),
        Some("sonnet"),
        "the pick stays"
    );
    assert_eq!(state.thinking_level.as_deref(), Some("high"));

    let stopped = chat.stop().await;
    assert_eq!(
        stopped.last(),
        Some(&AgentEvent::Exited {
            code: Some(0),
            signal: None
        })
    );
}

#[tokio::test]
async fn commands_follow_reload_skills_commands_changed_and_the_skills_of_each_turn() {
    let chat = start("claude_commands.jsonl").await;

    let AgentReply::Commands { commands } = chat.send(AgentAction::RefreshCommands).await else {
        panic!("commands")
    };
    assert_eq!(source_of(&commands, "figma"), Some(CommandSource::Skill));
    assert_eq!(
        source_of(&commands, "review-pr"),
        Some(CommandSource::Skill),
        "a skill the list lacked"
    );
    assert_eq!(source_of(&commands, "deploy"), Some(CommandSource::Custom));
    let review = commands
        .iter()
        .find(|command| command.name == "review-pr")
        .expect("review-pr");
    assert_eq!(review.argument_hint.as_deref(), Some("<number>"));
    chat.wait(
        "the refreshed catalog",
        |event| matches!(event, AgentEvent::CommandsUpdated { commands: emitted } if emitted == &commands),
    )
    .await;

    chat.wait("commands changed", |event| {
        matches!(event, AgentEvent::CommandsUpdated { commands } if source_of(commands, "embedded-captions").is_some())
    })
    .await;
    let changed = last_catalog(&chat.events());
    assert_eq!(
        source_of(&changed, "deploy"),
        None,
        "commands_changed replaces the list"
    );
    assert_eq!(source_of(&changed, "embedded-captions"), Some(CommandSource::Custom));

    chat.send(prompt("Reply with exactly: one")).await;
    let events = chat.settled(1).await;
    let after_turn = last_catalog(&events);
    assert_eq!(
        source_of(&after_turn, "embedded-captions"),
        Some(CommandSource::Skill),
        "system/init lists it as a skill"
    );

    chat.stop().await;
}
