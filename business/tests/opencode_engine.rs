//! The OpenCode chat engine's lifecycle and pickers against the scripted `opencode serve`:
//! startup, resume, the server password, startup failures, crash and stop.

mod agent_support;
mod opencode_engine_support;

use std::time::{Duration, Instant};

use agent_support::options;
use harness_business::BusinessError;
use harness_business::agent::SessionOptions;
use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, CommandSource, ImageAttachment, IpcErrorCode, ModelOption,
};
use opencode_engine_support::{SESSION, env, kind, start, start_with};
use pretty_assertions::assert_eq;

fn mock_model() -> ModelOption {
    ModelOption {
        provider: "mock".to_owned(),
        id: "m1".to_owned(),
        name: Some("Mock One".to_owned()),
        reasoning: false,
        context_window: Some(100_000),
        max_tokens: Some(4096),
        input: vec!["text".to_owned()],
    }
}

fn prompt(message: &str) -> AgentAction {
    AgentAction::Prompt {
        message: message.to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    }
}

fn code(result: harness_business::Result<AgentReply>) -> IpcErrorCode {
    result.expect_err("the action is refused").code()
}

#[tokio::test]
async fn startup_creates_a_session_and_emits_ready_state_and_commands_first() {
    let chat = start("opencode_hang.jsonl").await;
    let session = chat.session();

    chat.wait("catalog", |event| matches!(event, AgentEvent::CommandsUpdated { .. }))
        .await;
    let events = chat.events();
    assert_eq!(
        events[..3].iter().map(kind).collect::<Vec<_>>(),
        ["ready", "state", "commandsUpdated"]
    );

    let AgentEvent::State { state } = &events[1] else {
        unreachable!()
    };
    assert_eq!(state.session_id, SESSION);
    assert_eq!(state.session_name, None, "OpenCode's default title is not a name");
    assert_eq!(state.agent.as_deref(), Some("build"));
    assert_eq!(state.model, None, "OpenCode picks the model of a new session");
    assert!(!state.is_streaming);

    let AgentEvent::CommandsUpdated { commands } = &events[2] else {
        unreachable!()
    };
    let skill = commands
        .iter()
        .find(|command| command.name == "customize-opencode")
        .expect("live skill");
    assert_eq!(skill.source, CommandSource::Skill);

    let spec = chat.spawner.requested().pop().expect("spawned");
    let password = env(&spec, "OPENCODE_SERVER_PASSWORD").expect("password");
    assert_eq!(password.len(), 64);
    assert!(password.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(env(&spec, "OPENCODE_SERVER_USERNAME").as_deref(), Some("opencode"));
    assert_eq!(spec.cwd.as_deref(), Some(chat.project.as_path()));

    let agents = match session.send(AgentAction::GetAvailableAgents).await {
        Ok(AgentReply::Agents { agents }) => agents,
        other => panic!("agents: {other:?}"),
    };
    assert_eq!(
        agents.iter().map(|agent| agent.id.as_str()).collect::<Vec<_>>(),
        ["build", "plan"]
    );

    let models = match session.send(AgentAction::GetAvailableModels).await {
        Ok(AgentReply::Models { models }) => models,
        other => panic!("models: {other:?}"),
    };
    assert!(models.contains(&mock_model()));

    let refreshed = session.send(AgentAction::RefreshCommands).await.expect("commands");
    assert!(matches!(refreshed, AgentReply::Commands { commands } if commands.iter().any(|c| c.name == "review")));

    // A live command is not sent as text yet; nothing reaches the server (it would fail the scenario).
    assert_eq!(
        code(session.send(prompt("/customize-opencode set it up")).await),
        IpcErrorCode::Unsupported
    );
    assert_eq!(code(session.send(AgentAction::Abort).await), IpcErrorCode::Unsupported);
    assert_eq!(
        code(session.send(AgentAction::NewSession).await),
        IpcErrorCode::Unsupported
    );
    assert!(session.bash("ls").await.is_err());

    session.stop().await;
}

#[tokio::test]
async fn stop_sends_sigterm_to_a_server_that_ignores_stdin_eof() {
    let options = SessionOptions {
        stop_grace: Duration::from_secs(20),
        ..options()
    };
    let chat = start_with("opencode_hang.jsonl", None, options).await;
    chat.wait("catalog", |event| matches!(event, AgentEvent::CommandsUpdated { .. }))
        .await;

    let started = Instant::now();
    chat.session().stop().await;

    assert!(
        started.elapsed() < Duration::from_secs(5),
        "stopped by SIGTERM, not after the grace"
    );
    assert_eq!(
        chat.events().last(),
        Some(&AgentEvent::Exited {
            code: None,
            signal: Some(15)
        })
    );
    assert!(!chat.session().is_running());
}

#[tokio::test]
async fn resume_takes_title_agent_and_model_from_the_session() {
    let chat = start_with("opencode_resume.jsonl", Some(SESSION), options()).await;

    let state = match chat
        .wait("state", |event| matches!(event, AgentEvent::State { .. }))
        .await
    {
        AgentEvent::State { state } => state,
        _ => unreachable!(),
    };
    assert_eq!(state.session_id, SESSION);
    assert_eq!(state.session_name.as_deref(), Some("Renamed chat"));
    assert_eq!(state.agent.as_deref(), Some("build"));
    assert_eq!(state.model, Some(mock_model()));
    assert_eq!(state.thinking_level, None, "the mock model has no variants");

    chat.session().stop().await;
}

#[tokio::test]
async fn a_session_of_another_project_is_not_resumed() {
    let chat = start_with("opencode_resume_elsewhere.jsonl", Some(SESSION), options()).await;

    let error = chat.session.as_ref().err().expect("refused");
    assert!(matches!(error, BusinessError::NotFound(_)), "{error:?}");

    chat.wait_reaped().await;
    assert!(chat.events().is_empty(), "a failed start emits nothing");
}

#[tokio::test]
async fn a_server_without_a_listening_line_times_out() {
    let options = SessionOptions {
        startup_timeout: Duration::from_millis(300),
        ..options()
    };
    let chat = start_with("opencode_no_listen.jsonl", None, options).await;

    let error = chat.session.as_ref().err().expect("startup fails");
    assert!(matches!(error, BusinessError::Timeout(_)), "{error:?}");
    assert!(error.to_string().contains("server address"), "{error}");
    assert!(!error.to_string().contains(&chat.password()));

    chat.wait_reaped().await;
    assert!(chat.events().is_empty());
}

#[tokio::test]
async fn a_crashed_server_ends_the_chat_without_a_lost_stream_notice() {
    let chat = start("opencode_crash.jsonl").await;

    chat.wait("exit", |event| matches!(event, AgentEvent::Exited { .. }))
        .await;
    let events = chat.events();

    assert_eq!(
        events.last(),
        Some(&AgentEvent::Exited {
            code: Some(3),
            signal: None
        })
    );
    assert!(
        !events.iter().any(|event| matches!(event, AgentEvent::Notice { .. })),
        "{events:#?}"
    );

    let refused = chat.session().send(AgentAction::GetState).await;
    assert_eq!(code(refused), IpcErrorCode::Process);
}

#[tokio::test]
async fn model_thinking_level_and_agent_go_with_the_next_prompt() {
    let chat = start("opencode_prompt_settings.jsonl").await;
    let session = chat.session();
    let send = |action| session.send(action);

    let longcat = AgentAction::SetModel {
        provider: "opencode".to_owned(),
        model_id: "longcat-2.5-preview-free".to_owned(),
    };
    send(longcat).await.expect("model set");

    let levels = send(AgentAction::GetAvailableThinkingLevels).await.expect("levels");
    let expected = ["default", "low", "medium", "high"].map(str::to_owned).to_vec();
    assert_eq!(levels, AgentReply::ThinkingLevels { levels: expected });

    send(AgentAction::SetThinkingLevel {
        level: "high".to_owned(),
    })
    .await
    .expect("level set");
    send(AgentAction::SetAgent {
        agent: "plan".to_owned(),
    })
    .await
    .expect("agent set");

    let unknown_model = AgentAction::SetModel {
        provider: "nope".to_owned(),
        model_id: "missing".to_owned(),
    };
    assert_eq!(code(send(unknown_model).await), IpcErrorCode::InvalidInput);
    assert_eq!(
        code(
            send(AgentAction::SetThinkingLevel {
                level: "max".to_owned()
            })
            .await
        ),
        IpcErrorCode::InvalidInput
    );
    assert_eq!(
        code(
            send(AgentAction::SetAgent {
                agent: "explore".to_owned()
            })
            .await
        ),
        IpcErrorCode::InvalidInput
    );

    let image = ImageAttachment {
        data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC".to_owned(),
        mime_type: "image/png".to_owned(),
    };
    let look = AgentAction::Prompt {
        message: "Look".to_owned(),
        images: vec![image],
        streaming_behavior: None,
    };
    let reply = send(look).await.expect("the server takes the prompt");
    assert_eq!(
        reply,
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("started".to_owned())
        }
    );

    let state = match send(AgentAction::GetState).await {
        Ok(AgentReply::State { state }) => state,
        other => panic!("state: {other:?}"),
    };
    assert!(state.is_streaming, "the prompt is on its way");
    assert_eq!(state.agent.as_deref(), Some("plan"));
    assert_eq!(state.thinking_level.as_deref(), Some("high"));
    assert_eq!(
        state.model.map(|model| model.id).as_deref(),
        Some("longcat-2.5-preview-free")
    );

    assert_eq!(
        code(
            send(AgentAction::SetAgent {
                agent: "build".to_owned()
            })
            .await
        ),
        IpcErrorCode::Unsupported
    );
    assert_eq!(code(send(prompt("again")).await), IpcErrorCode::Unsupported);

    let changes: Vec<String> = chat
        .events()
        .iter()
        .filter(|event| {
            matches!(
                event,
                AgentEvent::ModelChanged { .. } | AgentEvent::ThinkingChanged { .. } | AgentEvent::AgentChanged { .. }
            )
        })
        .map(kind)
        .collect();
    assert_eq!(changes, ["modelChanged", "thinkingChanged", "agentChanged"]);

    session.stop().await;
}

#[test]
fn a_password_cut_off_by_the_line_limit_is_masked_too() {
    use harness_business::security::mask_secret;

    let secret = "0123456789abcdef";

    assert_eq!(mask_secret("pw=0123456789abcdef!", secret, false), "pw=••••!");
    assert_eq!(mask_secret("pw=01234567", secret, true), "pw=••••");
    assert_eq!(
        mask_secret("pw=01234567", secret, false),
        "pw=01234567",
        "only a cut line ends early"
    );
    assert_eq!(
        mask_secret("ends in 012", secret, true),
        "ends in 012",
        "three bytes say nothing"
    );
}
