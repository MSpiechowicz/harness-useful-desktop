//! The OpenCode 2 chat engine's lifecycle and pickers against the scripted OpenCode 2 server:
//! startup, readiness, resume, the server password, startup failures, crash and stop.

use crate::agent_support;
use crate::opencode2_engine_support;

use std::time::{Duration, Instant};

use agent_support::{binary, options};
use harness_business::BusinessError;
use harness_business::agent::SessionOptions;
use harness_business::harness::{Harness, LaunchRequest, OpenCodeGeneration};
use harness_business::harnesses::OpenCode;
use harness_business::model::{
    AgentAction, AgentEvent, AgentReply, ImageAttachment, IpcErrorCode, ModelOption, SessionState,
};
use harness_business::opencode_server::routes::percent_encode;
use opencode2_engine_support::{
    SERVE_ARGS, SESSION, count, env, kind, start, start_with, terminated, text_prompt, until_marker,
};
use pretty_assertions::assert_eq;

fn code(result: harness_business::Result<AgentReply>) -> IpcErrorCode {
    result.expect_err("the action is refused").code()
}

fn big_pickle() -> ModelOption {
    ModelOption {
        provider: "opencode".to_owned(),
        id: "big-pickle".to_owned(),
        name: Some("Big Pickle".to_owned()),
        reasoning: false,
        context_window: Some(200_000),
        max_tokens: Some(32_000),
        input: vec!["text".to_owned()],
    }
}

async fn first_state(chat: &opencode2_engine_support::Chat) -> SessionState {
    match chat
        .wait("state", |event| matches!(event, AgentEvent::State { .. }))
        .await
    {
        AgentEvent::State { state } => state,
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn startup_creates_a_session_and_emits_ready_state_and_commands_first() {
    let chat = start("opencode2_hang.jsonl").await;
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
    assert_eq!(state.session_name, None, "a new session has no name yet");
    assert_eq!(state.agent.as_deref(), Some("build"));
    assert_eq!(state.model, Some(big_pickle()), "the model OpenCode gave the session");
    assert_eq!(state.thinking_level, None, "the model has no variants");
    assert!(!state.is_streaming);

    let AgentEvent::CommandsUpdated { commands } = &events[2] else {
        unreachable!()
    };
    assert!(commands.iter().any(|command| command.name == "review"), "{commands:#?}");

    let spec = chat.spawner.requested().pop().expect("spawned");
    let password = env(&spec, "OPENCODE_SERVER_PASSWORD").expect("a server password");
    assert_eq!(password.len(), 64);
    assert!(password.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(spec.cwd.as_deref(), Some(chat.project.as_path()));

    let requests = chat.requests();
    assert!(requests.iter().all(|request| request.authorized), "{requests:#?}");
    let project = chat.project.to_str().expect("UTF-8 project");
    let created = chat.requests_to("POST /api/session").pop().expect("session created");
    assert!(created.target.ends_with(&percent_encode(project)), "{}", created.target);

    let agents = match session.send(AgentAction::GetAvailableAgents).await {
        Ok(AgentReply::Agents { agents }) => agents,
        other => panic!("agents: {other:?}"),
    };
    assert_eq!(
        agents.iter().map(|agent| agent.id.as_str()).collect::<Vec<_>>(),
        ["build", "plan"],
        "primary agents only, hidden ones left out"
    );

    let models = match session.send(AgentAction::GetAvailableModels).await {
        Ok(AgentReply::Models { models }) => models,
        other => panic!("models: {other:?}"),
    };
    assert!(models.contains(&big_pickle()));

    let refreshed = session.send(AgentAction::RefreshCommands).await.expect("commands");
    assert!(matches!(refreshed, AgentReply::Commands { commands } if commands.iter().any(|c| c.name == "init")));

    assert_eq!(
        session.send(AgentAction::Abort).await.expect("idle abort"),
        AgentReply::Ack
    );
    assert!(chat.requests_to("POST /api/session/*/interrupt").is_empty());

    session.stop().await;
}

#[tokio::test]
async fn without_agents_listed_the_default_model_shows_and_the_agents_fall_back_to_build_and_plan() {
    let chat = start("opencode2_offline.jsonl").await;

    let state = first_state(&chat).await;
    assert_eq!(
        state.model.as_ref().map(|model| model.id.as_str()),
        Some("deepseek-v4.1-flash-free"),
        "the model OpenCode uses while the session names none"
    );
    assert_eq!(state.thinking_level.as_deref(), Some("default"));
    assert_eq!(state.agent.as_deref(), Some("build"));

    let agents = match chat.session().send(AgentAction::GetAvailableAgents).await {
        Ok(AgentReply::Agents { agents }) => agents,
        other => panic!("agents: {other:?}"),
    };
    assert_eq!(
        agents.iter().map(|agent| agent.id.as_str()).collect::<Vec<_>>(),
        ["build", "plan"]
    );

    chat.session().stop().await;
}

#[tokio::test]
async fn readiness_waits_through_503() {
    let chat = start("opencode2_not_ready.jsonl").await;

    chat.wait("ready", |event| matches!(event, AgentEvent::Ready { .. }))
        .await;
    assert_eq!(chat.requests_to("GET /api/info").len(), 3);

    chat.session().stop().await;
}

#[tokio::test]
async fn a_refused_password_fails_the_start() {
    let chat = start("opencode2_refused.jsonl").await;

    let error = chat.session.as_ref().err().expect("refused");
    assert!(matches!(error, BusinessError::Protocol(_)), "{error:?}");
    assert!(error.to_string().contains("password"), "{error}");

    chat.wait_reaped().await;
    assert!(chat.events().is_empty(), "a failed start emits nothing");
}

#[tokio::test]
async fn stop_sends_sigterm_to_a_server_that_ignores_stdin_eof() {
    let options = SessionOptions {
        stop_grace: Duration::from_secs(20),
        ..options()
    };
    let chat = start_with("opencode2_hang.jsonl", None, options).await;
    chat.wait("catalog", |event| matches!(event, AgentEvent::CommandsUpdated { .. }))
        .await;

    let started = Instant::now();
    chat.session().stop().await;

    assert!(
        started.elapsed() < Duration::from_secs(5),
        "stopped by SIGTERM, not after the grace"
    );
    assert_eq!(chat.events().last(), Some(&terminated()));
    assert!(!chat.session().is_running());
}

#[tokio::test]
async fn resume_takes_title_agent_model_and_context_and_shows_no_old_message_again() {
    let chat = start_with("opencode2_resume.jsonl", Some(SESSION), options()).await;

    let state = first_state(&chat).await;
    assert_eq!(state.session_id, SESSION);
    assert_eq!(state.session_name.as_deref(), Some("Renamed chat"));
    assert_eq!(state.agent.as_deref(), Some("plan"));
    assert_eq!(
        state.model.as_ref().map(|model| model.id.as_str()),
        Some("deepseek-v4.1-flash-free")
    );
    assert_eq!(state.thinking_level.as_deref(), Some("high"));
    let context = state.context_usage.expect("the newest reply sizes the context");
    assert_eq!((context.tokens, context.context_window), (5403, 200_000));

    let events = until_marker(&chat).await;
    assert_eq!(
        count(&events, "messageStart") + count(&events, "messageEnd"),
        0,
        "{events:#?}"
    );

    let history = AgentAction::GetHistory {
        cursor: None,
        limit: 50,
    };
    let messages = match chat.session().send(history).await {
        Ok(AgentReply::History { messages, .. }) => messages,
        other => panic!("history: {other:?}"),
    };
    let ids: Vec<Option<&str>> = messages.iter().map(|message| message.id.as_deref()).collect();
    assert_eq!(
        ids,
        [
            Some("msg_15d7bc2db001z7i7HF20ZowIPY"),
            Some("msg_15d7bc3d6001sXHw8uAIflaGhD")
        ],
        "oldest first"
    );

    chat.session().stop().await;
}

#[tokio::test]
async fn a_session_of_another_project_is_not_resumed() {
    let chat = start_with("opencode2_resume_elsewhere.jsonl", Some(SESSION), options()).await;

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
    let chat = start_with("opencode2_no_listen.jsonl", None, options).await;

    let error = chat.session.as_ref().err().expect("startup fails");
    assert!(matches!(error, BusinessError::Timeout(_)), "{error:?}");
    assert!(error.to_string().contains("server address"), "{error}");
    assert!(!error.to_string().contains(&chat.password()));

    chat.wait_reaped().await;
    assert!(chat.events().is_empty());
}

#[tokio::test]
async fn a_crashed_server_ends_the_chat_without_a_lost_stream_notice() {
    let chat = start("opencode2_crash.jsonl").await;

    chat.wait("exit", |event| matches!(event, AgentEvent::Exited { .. }))
        .await;
    let events = chat.events();

    assert_eq!(
        events.last(),
        Some(&AgentEvent::Exited {
            code: Some(1),
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
async fn model_thinking_level_and_agent_go_to_the_session() {
    let chat = start("opencode2_settings.jsonl").await;
    let session = chat.session();
    let send = |action| session.send(action);

    let deepseek = AgentAction::SetModel {
        provider: "opencode".to_owned(),
        model_id: "deepseek-v4.1-flash-free".to_owned(),
    };
    send(deepseek).await.expect("model set");

    let levels = send(AgentAction::GetAvailableThinkingLevels).await.expect("levels");
    let expected = ["default", "low", "high", "max"].map(str::to_owned).to_vec();
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
    let unknown_level = AgentAction::SetThinkingLevel {
        level: "turbo".to_owned(),
    };
    assert_eq!(code(send(unknown_level).await), IpcErrorCode::InvalidInput);
    let unknown_agent = AgentAction::SetAgent {
        agent: "explore".to_owned(),
    };
    assert_eq!(code(send(unknown_agent).await), IpcErrorCode::InvalidInput);

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
        Some("deepseek-v4.1-flash-free")
    );

    let busy_agent = AgentAction::SetAgent {
        agent: "build".to_owned(),
    };
    assert_eq!(code(send(busy_agent).await), IpcErrorCode::Unsupported);

    let held = send(text_prompt("again")).await.expect("held for after the turn");
    assert_eq!(
        held,
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("queued".to_owned())
        }
    );

    let events = until_marker(&chat).await;
    let changes: Vec<String> = events
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
    assert_eq!(
        count(&events, "state"),
        2,
        "the first state and the marker: OpenCode's confirmations of the chat's picks change nothing: {events:#?}"
    );
    assert_eq!(chat.requests_to("POST /api/session/*/model").len(), 2);

    session.stop().await;
}

#[test]
fn opencode_2_serves_on_loopback_without_mdns_and_inherits_no_password() {
    let binary = binary();
    let project = std::path::PathBuf::from("/work/demo");
    let request = LaunchRequest {
        binary: &binary,
        project: &project,
        resume: None,
        approval_override: None,
        trust_once: false,
        utility: false,
    };

    for generation in [OpenCodeGeneration::V1, OpenCodeGeneration::V2] {
        let spec = OpenCode::with_generation(generation)
            .launch_spec(&request)
            .expect("launch spec");
        let args: Vec<String> = spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();

        match generation {
            OpenCodeGeneration::V1 => assert_eq!(args.last().map(String::as_str), Some("--mdns=false")),
            OpenCodeGeneration::V2 => assert_eq!(args, SERVE_ARGS, "OpenCode 2 refuses --mdns"),
        }

        for prefix in ["OPENCODE_SERVER_", "OPENCODE_PASSWORD", "OPENCODE_WORKSPACE_ID"] {
            assert!(
                spec.env_remove_prefixes.contains(&std::ffi::OsString::from(prefix)),
                "{generation:?}: {prefix}"
            );
        }
    }
}
