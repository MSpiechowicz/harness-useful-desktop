//! The model a Claude Code chat starts on against the scripted `claude` (the recorded handshake of
//! claude_init.jsonl): Harness Skills' default model role for a new session without a pick, never
//! for a resumed, forked, cloned or helper session, and the `default` entry's model otherwise.

mod agent_support;
mod claude_engine_support;

use claude_engine_support::{BASIC_SESSION, Chat, Setup, argv, prompt};
use harness_business::model::{AgentAction, AgentEvent, AgentReply, SessionState};
use pretty_assertions::assert_eq;

/// The models file that starts new chats on Sonnet 5.5 with high effort.
const SONNET_HIGH: &str =
    "# Useful Skills model roles\nmodelRoles:\n  plan: claude-opus-5-5:high\n  default: claude-sonnet-5-5:high\n";

/// In BASIC_SESSION's file: a prompt after its compaction (a fork point).
const AFTER_COMPACTION: &str = "00c001c2-18bd-4e78-a1db-d7d85c85cbb8";

fn models_flags(args: &[String]) -> Vec<&str> {
    args.iter()
        .map(String::as_str)
        .filter(|arg| arg.starts_with("--model") || arg.starts_with("--effort"))
        .collect()
}

/// The first `state` the chat emits.
async fn first_state(chat: &Chat) -> SessionState {
    chat.wait("state", |event| matches!(event, AgentEvent::State { .. }))
        .await;

    chat.events()
        .into_iter()
        .find_map(|event| match event {
            AgentEvent::State { state } => Some(state),
            _ => None,
        })
        .expect("a state")
}

async fn new_chat(models_file: Option<&str>) -> Chat {
    Setup {
        models_file,
        ..Setup::new(&["claude_init.jsonl"])
    }
    .start()
    .await
}

/// Waits until the chat started `count` processes.
async fn spawned(chat: &Chat, count: usize) {
    let deadline = tokio::time::Instant::now() + agent_support::WAIT;

    while chat.spawner.requested().len() < count {
        assert!(tokio::time::Instant::now() < deadline, "no process {count}");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn a_new_chat_starts_on_the_default_role_and_its_effort() {
    let chat = Setup {
        models_file: Some(SONNET_HIGH),
        ..Setup::new(&["claude_init.jsonl", "claude_init.jsonl"])
    }
    .start()
    .await;

    assert_eq!(
        models_flags(&argv(&chat, 0)),
        ["--model=claude-sonnet-5-5", "--effort=high"]
    );

    let state = first_state(&chat).await;
    let model = state.model.expect("the model it started on");
    assert_eq!(model.id, "sonnet", "the picker's entry of that model");
    assert_eq!(model.name.as_deref(), Some("Sonnet 5.5"));
    assert_eq!(state.thinking_level.as_deref(), Some("high"));

    assert_eq!(
        chat.send(AgentAction::GetAvailableThinkingLevels).await,
        AgentReply::ThinkingLevels {
            levels: ["low", "medium", "high", "xhigh", "max"].map(str::to_owned).to_vec()
        }
    );

    assert_eq!(chat.send(AgentAction::NewSession).await, AgentReply::Ack);
    spawned(&chat, 2).await;
    assert_eq!(
        models_flags(&argv(&chat, 1)),
        ["--model=claude-sonnet-5-5", "--effort=high"],
        "a new session without a pick reads it again"
    );

    chat.stop().await;
}

#[tokio::test]
async fn the_model_system_init_names_wins_over_the_one_it_started_on() {
    let chat = Setup {
        models_file: Some(SONNET_HIGH),
        ..Setup::new(&["claude_basic.jsonl"])
    }
    .start()
    .await;

    assert_eq!(
        first_state(&chat).await.model.map(|model| model.id).as_deref(),
        Some("sonnet")
    );

    chat.send(prompt("Reply with exactly the word: pong")).await;
    let events = chat.settled(1).await;

    let changed = events.iter().find_map(|event| match event {
        AgentEvent::ModelChanged { model } => model.clone(),
        _ => None,
    });
    assert_eq!(
        changed.map(|model| model.id).as_deref(),
        Some("opus"),
        "the recorded turn ran on claude-opus-5-5"
    );

    chat.stop().await;
}

#[tokio::test]
async fn an_effort_the_model_does_not_offer_is_passed_but_not_shown() {
    let chat = new_chat(Some("modelRoles:\n  default: claude-haiku-4-5-20251001:high\n")).await;

    assert_eq!(
        models_flags(&argv(&chat, 0)),
        ["--model=claude-haiku-4-5-20251001", "--effort=high"]
    );

    let state = first_state(&chat).await;
    assert_eq!(
        state.model.map(|model| model.id).as_deref(),
        Some("claude-haiku-4-5-20251001")
    );
    assert_eq!(state.thinking_level, None);

    chat.stop().await;
}

#[tokio::test]
async fn without_a_models_file_the_chat_reports_the_default_entrys_model() {
    let chat = new_chat(None).await;

    assert_eq!(models_flags(&argv(&chat, 0)), Vec::<&str>::new());

    let state = first_state(&chat).await;
    let model = state.model.expect("the default model");
    assert_eq!(model.id, "opus", "the entry of the default's resolved model");
    assert_eq!(model.name.as_deref(), Some("Opus 5.5"));
    assert_eq!(state.thinking_level, None);

    let AgentReply::Models { models } = chat.send(AgentAction::GetAvailableModels).await else {
        panic!("models")
    };
    assert!(
        models.iter().any(|option| option.id == model.id),
        "the picker highlights it"
    );

    chat.stop().await;
}

#[tokio::test]
async fn inherit_an_invalid_file_and_hostile_values_add_nothing() {
    for text in [
        "modelRoles:\n  default: inherit\n",
        "modelRoles:\n  default: inherit:high\n",
        "modelRoles: [\n",
        "modelRoles:\n  default: \"--x\"\n",
        "modelRoles:\n  default: \"claude-opus-5-5 --x\"\n",
        "modelRoles:\n  default: \"opus:--x\"\n",
        "modelRoles:\n  default: \"opus:high extra\"\n",
        "modelRoles:\n  default: opus\n  unknown: sonnet\n",
        "\u{0}\u{1}not yaml",
    ] {
        let chat = new_chat(Some(text)).await;

        assert_eq!(models_flags(&argv(&chat, 0)), Vec::<&str>::new(), "{text:?}");

        let state = first_state(&chat).await;
        assert_eq!(state.model.map(|model| model.id).as_deref(), Some("opus"), "{text:?}");
        assert_eq!(state.thinking_level, None, "{text:?}");

        chat.stop().await;
    }
}

#[tokio::test]
async fn a_resume_fork_or_clone_never_reads_the_models_file() {
    let fork = AgentAction::Fork {
        entry_id: AFTER_COMPACTION.to_owned(),
    };

    for action in [fork, AgentAction::Clone] {
        let chat = Setup {
            resume: Some(BASIC_SESSION),
            sessions: &[BASIC_SESSION],
            models_file: Some(SONNET_HIGH),
            ..Setup::new(&["claude_init.jsonl", "claude_init.jsonl"])
        }
        .start()
        .await;

        assert_eq!(models_flags(&argv(&chat, 0)), Vec::<&str>::new(), "resumed");
        assert_eq!(first_state(&chat).await.thinking_level, None);

        assert_eq!(chat.send(action.clone()).await, AgentReply::Ack);
        assert_eq!(models_flags(&argv(&chat, 1)), Vec::<&str>::new(), "{action:?}");

        chat.stop().await;
    }
}

#[tokio::test]
async fn a_helper_chat_never_reads_the_models_file() {
    let chat = Setup {
        utility: true,
        models_file: Some(SONNET_HIGH),
        ..Setup::new(&["claude_utility.jsonl"])
    }
    .start()
    .await;

    assert_eq!(models_flags(&argv(&chat, 0)), Vec::<&str>::new());
    assert_eq!(first_state(&chat).await.thinking_level, None);

    chat.stop().await;
}
