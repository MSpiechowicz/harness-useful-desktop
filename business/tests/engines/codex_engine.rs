//! The Codex chat engine started from the harness's launch spec against the scripted app server:
//! the command line and handshake, a new and a resumed thread, a prompt with its streamed reply,
//! statistics and the last reply, an image as a file, and a Codex that dies before or during a
//! turn.

use crate::codex_engine_support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_engine_support::{THREAD, launch, prompt, replies, start};
use harness_business::BusinessError;
use harness_business::agent::AgentSession;
use harness_business::harnesses::codex::APP_SERVER_ARGS;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ImageAttachment, PromptStatus};
use pretty_assertions::assert_eq;

/// The tags of the first events of a chat.
fn opening(events: &[AgentEvent]) -> Vec<String> {
    crate::agent_support::kinds(&events[..3.min(events.len())])
}

#[tokio::test]
async fn a_new_chat_starts_app_server_in_the_project_and_names_its_thread() {
    let chat = start("codex_engine_open.jsonl").await;

    let spec = chat.spawner.requested().remove(0);
    let args: Vec<String> = spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
    assert_eq!(args, APP_SERVER_ARGS);
    assert_eq!(spec.cwd.as_deref(), Some(chat.project.as_path()));

    chat.wait("the commands", |event| {
        matches!(event, AgentEvent::CommandsUpdated { .. })
    })
    .await;
    assert_eq!(opening(&chat.events()), ["ready", "state", "commandsUpdated"]);

    let AgentReply::State { state } = chat.act(AgentAction::GetState).await else {
        panic!("not a state");
    };
    assert_eq!(state.session_id, THREAD);
    assert_eq!(
        state.model.as_ref().map(|model| model.id.as_str()),
        Some("gpt-5.6-terra")
    );
    assert_eq!(state.thinking_level.as_deref(), Some("medium"));
    assert!(!state.is_streaming);

    let AgentSession::Codex(session) = &chat.session else {
        panic!("not the Codex engine");
    };
    assert_eq!(session.codex_version().as_deref(), Some("0.154.0"));

    chat.finish().await;
}

#[tokio::test]
async fn a_resumed_chat_resumes_its_thread_without_turns() {
    let launched = launch("codex_engine_resume.jsonl", Some(THREAD)).await;
    let session = launched.result.expect("the resumed chat starts");

    let AgentReply::State { state } = session.send(AgentAction::GetState).await.expect("a state") else {
        panic!("not a state");
    };
    assert_eq!(state.session_id, THREAD);
    assert_eq!(state.session_name.as_deref(), Some("Recorded thread"));
    assert_eq!(state.thinking_level.as_deref(), Some("high"));

    session.stop().await;
    assert_eq!(
        launched.sink.events().last(),
        Some(&AgentEvent::Exited {
            code: Some(0),
            signal: None
        })
    );
}

#[tokio::test]
async fn a_thread_id_that_is_not_a_uuid_is_refused_before_spawning() {
    let launched = launch("codex_engine_resume.jsonl", Some("../thread")).await;

    assert!(matches!(launched.result, Err(BusinessError::InvalidInput(_))));
    assert!(launched.spawner.requested().is_empty());
}

#[tokio::test]
async fn a_prompt_streams_its_reply_and_settles() {
    let chat = start("codex_engine_basic.jsonl").await;
    let taken = Arc::new(AtomicBool::new(false));
    let flag = taken.clone();
    chat.session
        .on_prompt_taken(Box::new(move || flag.store(true, Ordering::SeqCst)));

    let reply = chat.act(prompt("Ask me about colours.")).await;
    assert_eq!(
        reply,
        AgentReply::Prompt {
            agent_invoked: Some(true),
            disposition: Some("started".to_owned())
        }
    );

    let events = chat.settled(1).await;
    assert!(taken.load(Ordering::SeqCst), "the prompt-taken call ran");
    assert_eq!(
        replies(&events),
        ["I’ll ask one quick multiple-choice question.", "NO_TOOL"]
    );
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::PromptResult {
            status: PromptStatus::Completed,
            ..
        }
    )));
    assert_eq!(events.last(), Some(&AgentEvent::Settled { aborted: Some(false) }));

    let AgentReply::Text { text } = chat.act(AgentAction::GetLastAssistantText).await else {
        panic!("not a text");
    };
    assert_eq!(text.as_deref(), Some("NO_TOOL"));

    let AgentReply::Stats { stats } = chat.act(AgentAction::GetSessionStats).await else {
        panic!("not stats");
    };
    assert_eq!(stats.assistant_messages, 2);
    assert_eq!(stats.user_messages, 1);
    assert!(stats.tokens.total > 0);
    let context = stats.context_usage.expect("a context meter");
    assert_eq!(context.context_window, 258_400);

    chat.finish().await;
}

#[tokio::test]
async fn an_image_goes_to_codex_as_a_saved_file() {
    let chat = start("codex_engine_image.jsonl").await;
    let pixel = STANDARD.encode(b"not really a png");

    chat.act(AgentAction::Prompt {
        message: "What is this?".to_owned(),
        images: vec![ImageAttachment {
            data: pixel,
            mime_type: "image/png".to_owned(),
        }],
        streaming_behavior: None,
    })
    .await;
    chat.settled(1).await;

    let saved: Vec<_> = walk(chat.attachments.path());
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert!(saved[0].ends_with("image.png"));
    assert_eq!(std::fs::read(&saved[0]).unwrap(), b"not really a png");

    chat.finish().await;
}

/// Every file below `dir`.
fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();

        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }

    files
}

#[tokio::test]
async fn codex_dying_in_a_turn_ends_the_chat() {
    let chat = start("codex_engine_crash.jsonl").await;

    chat.act(prompt("Crash, please.")).await;

    let exited = chat
        .wait("exited", |event| matches!(event, AgentEvent::Exited { .. }))
        .await;
    assert_eq!(
        exited,
        AgentEvent::Exited {
            code: Some(101),
            signal: None
        }
    );
    assert!(chat.events().iter().any(|event| matches!(
        event,
        AgentEvent::Stderr { line } if line.contains("panicked")
    )));

    let error = chat.session.send(prompt("Again?")).await.unwrap_err();
    assert!(matches!(error, BusinessError::Process(_)), "{error:?}");
}

#[tokio::test]
async fn codex_exiting_before_the_handshake_fails_the_start_with_its_stderr() {
    let launched = launch("codex_engine_crash_start.jsonl", None).await;

    let Err(error) = launched.result else {
        panic!("the start fails");
    };
    assert!(error.to_string().contains("unexpected argument"), "{error}");
    assert!(launched.sink.events().is_empty(), "a failed start emits nothing");
}
