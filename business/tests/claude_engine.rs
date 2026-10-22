//! The Claude Code chat engine's lifecycle against the scripted `claude`: startup and the first
//! events, the handshake's commands and models, resume, startup failures, crashes, the missing
//! login and the missing agent picker.

mod agent_support;
mod claude_engine_support;

use std::time::{Duration, Instant};

use agent_support::options;
use claude_engine_support::{BASIC_SESSION, argv, flag, kind, kinds, prompt, start, start_with};
use harness_business::BusinessError;
use harness_business::agent::SessionOptions;
use harness_business::claude_stream::mapper::SIGN_IN_ERROR;
use harness_business::model::{
    AgentAction, AgentEvent, CommandInvocation, CommandSource, IpcErrorCode, PromptStatus, UiOutcome, UiResponse,
};
use pretty_assertions::assert_eq;

/// The session of nologin.jsonl.
const NOLOGIN_SESSION: &str = "b291bc2d-4ae9-4e22-bbb4-7eadffecf665";

fn source_of(commands: &[harness_business::model::CommandSpec], name: &str) -> Option<CommandSource> {
    commands
        .iter()
        .find(|command| command.name == name)
        .map(|command| command.source)
}

#[tokio::test]
async fn startup_sends_initialize_and_emits_ready_state_and_commands_first() {
    let chat = start("claude_startup.jsonl").await;
    chat.wait(
        "session id",
        |event| matches!(event, AgentEvent::State { state } if state.session_id == BASIC_SESSION),
    )
    .await;

    let events = chat.events();
    assert_eq!(kinds(&events[..3]), ["ready", "state", "commandsUpdated"]);
    assert_eq!(events[0], AgentEvent::Ready { protocol_version: None });

    let AgentEvent::State { state } = &events[1] else {
        unreachable!()
    };
    let args = argv(&chat, 0);
    let named = args
        .iter()
        .position(|arg| arg == "--session-id")
        .map(|flag| args[flag + 1].clone());
    assert_eq!(
        named.as_deref(),
        Some(state.session_id.as_str()),
        "a new chat names its session at startup"
    );
    assert_eq!(state.session_id.len(), 36, "a UUID");
    assert_eq!(
        state.model.as_ref().map(|model| model.id.as_str()),
        Some("default"),
        "before a turn names the model: the entry of the default's resolved model"
    );
    assert_eq!(state.thinking_level, None, "Claude Code does not report its effort");
    assert!(!state.is_streaming);

    let AgentEvent::CommandsUpdated { commands } = &events[2] else {
        unreachable!()
    };
    assert_eq!(source_of(commands, "design"), Some(CommandSource::Builtin));
    assert_eq!(
        source_of(commands, "figma"),
        Some(CommandSource::Custom),
        "no turn listed the skills yet"
    );
    assert_eq!(
        source_of(commands, "useful-skills:us-plan"),
        Some(CommandSource::Extension)
    );
    assert_eq!(source_of(commands, "mcp__docs__search"), Some(CommandSource::McpPrompt));

    let deploy = commands
        .iter()
        .find(|command| command.name == "deploy")
        .expect("deploy");
    assert_eq!(deploy.argument_hint.as_deref(), Some("<env>"));
    assert_eq!(deploy.invocation, CommandInvocation::PromptText);

    let compact = commands
        .iter()
        .find(|command| command.name == "compact")
        .expect("compact");
    assert_eq!(compact.source, CommandSource::Builtin);
    assert!(
        matches!(compact.invocation, CommandInvocation::Rpc { .. }),
        "the native command wins"
    );
    assert!(
        commands
            .iter()
            .any(|command| command.name == "new" && command.source == CommandSource::Shared)
    );

    let json = serde_json::to_string(&chat.stop().await).expect("events serialize");
    assert!(!json.contains("CANARY"), "the account never reaches the chat: {json}");
}

#[tokio::test]
async fn the_spawn_uses_the_launch_spec_in_the_project() {
    let chat = start("claude_init.jsonl").await;
    chat.session();

    let spec = chat.spawner.requested().pop().expect("spawned");
    let args = argv(&chat, 0);
    assert_eq!(args[..2], ["--output-format", "stream-json"]);
    assert_eq!(spec.cwd.as_deref(), Some(chat.project.as_path()));
    assert_eq!(chat.spawner.requested().len(), 1, "one process per chat");

    chat.stop().await;
}

#[tokio::test]
async fn a_resumed_chat_knows_its_session_id_from_the_start() {
    let chat = start_with("claude_init.jsonl", Some(BASIC_SESSION), options()).await;

    let state = chat
        .wait("state", |event| matches!(event, AgentEvent::State { .. }))
        .await;
    assert!(matches!(state, AgentEvent::State { state } if state.session_id == BASIC_SESSION));
    assert_eq!(flag(&argv(&chat, 0), "--resume"), Some(BASIC_SESSION));

    chat.stop().await;
}

#[tokio::test]
async fn a_process_that_exits_before_the_handshake_fails_the_start_with_its_stderr() {
    let chat = start("claude_crash_start.jsonl").await;

    let Err(BusinessError::Process(message)) = &chat.session else {
        panic!("a process error: {:?}", chat.session.as_ref().err());
    };
    assert!(message.contains("exited during startup"), "{message}");
    assert!(message.contains("unknown option --bogus"), "{message}");
    assert!(chat.events().is_empty(), "a failed start emits nothing");
}

#[tokio::test]
async fn a_handshake_without_an_answer_times_out_and_kills_the_process() {
    let options = SessionOptions {
        startup_timeout: Duration::from_millis(300),
        ..options()
    };
    let started = Instant::now();
    let chat = start_with("claude_hang.jsonl", None, options).await;

    let Err(BusinessError::Timeout(message)) = &chat.session else {
        panic!("a timeout: {:?}", chat.session.as_ref().err());
    };
    assert!(message.contains("handshake"), "{message}");
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(chat.events().is_empty());

    let deadline = Instant::now() + Duration::from_secs(5);
    while chat.spawner.exited() == 0 {
        assert!(Instant::now() < deadline, "the hanging process was never reaped");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn the_startup_deadline_bounds_the_whole_start() {
    let options = SessionOptions {
        startup_deadline: Duration::from_millis(300),
        ..options()
    };
    let chat = start_with("claude_hang.jsonl", None, options).await;

    assert!(
        matches!(&chat.session, Err(BusinessError::Timeout(_))),
        "{:?}",
        chat.session.as_ref().err()
    );
}

#[tokio::test]
async fn a_crash_mid_turn_ends_the_chat_with_exited_last_and_the_stderr_line() {
    let chat = start("claude_crash.jsonl").await;
    chat.send(prompt("Reply with exactly the word: pong")).await;

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
        events
            .iter()
            .any(|event| { matches!(event, AgentEvent::Stderr { line } if line.contains("the model stream broke")) })
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::TextDelta { delta, .. } if delta == "pong"))
    );

    let refused = chat.session().send(prompt("again")).await;
    assert!(matches!(refused, Err(BusinessError::Process(_))), "{refused:?}");
}

#[tokio::test]
async fn sending_to_a_chat_whose_process_exited_fails() {
    let chat = start("claude_exit.jsonl").await;
    chat.wait("exit", |event| matches!(event, AgentEvent::Exited { .. }))
        .await;

    let refused = chat.session().send(prompt("hello")).await;
    assert!(matches!(refused, Err(BusinessError::Process(_))), "{refused:?}");
    assert!(!chat.session().is_running());
}

#[tokio::test]
async fn without_a_login_the_turn_fails_with_the_sign_in_error_and_the_chat_ends() {
    let chat = start("claude_nologin.jsonl").await;
    chat.send(prompt("Reply with exactly: pong")).await;

    chat.wait("exit", |event| matches!(event, AgentEvent::Exited { .. }))
        .await;
    let events = chat.events();

    let result = events
        .iter()
        .find(|event| matches!(event, AgentEvent::PromptResult { .. }))
        .expect("a prompt result");
    assert!(matches!(
        result,
        AgentEvent::PromptResult { status: PromptStatus::Error, error: Some(error), retryable: false, .. }
            if error == SIGN_IN_ERROR
    ));
    assert!(events.iter().any(|event| matches!(event, AgentEvent::Settled { .. })));
    assert_eq!(events.last().map(kind).as_deref(), Some("exited"));
    assert!(
        events
            .iter()
            .any(|event| { matches!(event, AgentEvent::State { state } if state.session_id == NOLOGIN_SESSION) })
    );
}

#[tokio::test]
async fn the_agent_picker_is_refused_and_an_unknown_dialog_is_not_found() {
    let chat = start("claude_init.jsonl").await;
    let session = chat.session();

    for action in [
        AgentAction::GetAvailableAgents,
        AgentAction::SetAgent {
            agent: "plan".to_owned(),
        },
    ] {
        let refused = session.send(action.clone()).await.expect_err("refused");
        assert_eq!(refused.code(), IpcErrorCode::Unsupported, "{action:?}");
    }

    let response = UiResponse {
        id: "nothing-open".to_owned(),
        outcome: UiOutcome::Cancelled,
    };
    let refused = session.respond(response).await.expect_err("no dialog is open");
    assert_eq!(refused.code(), IpcErrorCode::NotFound);

    chat.stop().await;
}

#[tokio::test]
async fn stop_closes_stdin_and_emits_exited_last() {
    let chat = start("claude_init.jsonl").await;

    let events = chat.stop().await;
    assert_eq!(
        events.last(),
        Some(&AgentEvent::Exited {
            code: Some(0),
            signal: None
        })
    );
    assert!(!chat.session().is_running());
}
