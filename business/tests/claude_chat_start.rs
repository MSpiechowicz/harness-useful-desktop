//! A Claude Code chat through the agent manager, now that the harness offers chat (a preview): a
//! new chat starts and binds to the session it names, and a helper chat (AI naming) runs without
//! tools, thinking or a saved session, answers its one prompt and denies nothing it is not asked.

mod agent_support;

use agent_support::{options, start_with};
use harness_business::harness::Harness;
use harness_business::harnesses::Claude;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ApprovalMode, HarnessId, IpcErrorCode};
use pretty_assertions::assert_eq;

fn args(run: &agent_support::Run) -> Vec<String> {
    let spec = run.spawner.requested().pop().expect("spawned");

    spec.args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
}

/// Stops every chat; the scripted CLI exits 0 only when every expected frame matched.
async fn stop_cleanly(run: &agent_support::Run) {
    run.manager.begin_stop_all().finish().await;

    let exited = run
        .events()
        .into_iter()
        .rev()
        .find(|event| matches!(event, AgentEvent::Exited { .. }));
    assert_eq!(
        exited,
        Some(AgentEvent::Exited {
            code: Some(0),
            signal: None
        })
    );
}

#[test]
fn the_harness_offers_chat_as_a_preview() {
    let capabilities = Claude.capabilities();

    assert!(capabilities.chat && capabilities.chat_preview);
    assert!(capabilities.compact_instructions && capabilities.memory_chat_guards);
    assert!(capabilities.slash_commands_via_prompt);
    assert!(!capabilities.yolo_mode && !capabilities.session_tree);
}

#[tokio::test]
async fn a_claude_chat_starts_through_the_manager_on_the_session_it_names() {
    let run = start_with(HarnessId::Claude, "claude", "claude_init.jsonl", options(), |_| {})
        .await
        .expect("the chat starts");
    assert_eq!(run.info.harness, HarnessId::Claude);

    let args = args(&run);
    let named = args
        .iter()
        .position(|arg| arg == "--session-id")
        .map(|at| args[at + 1].clone())
        .expect("a new chat names its session");

    let reply = run
        .manager
        .send(run.key(), AgentAction::GetState)
        .await
        .expect("a state");
    let AgentReply::State { state } = reply else {
        panic!("a state")
    };
    assert_eq!(state.session_id, named);
    assert!(run.manager.holds_session(&named), "the manager binds the named session");

    run.manager.begin_stop_all().finish().await;
}

#[tokio::test]
async fn a_helper_chat_answers_one_prompt_without_tools_thinking_or_a_saved_session() {
    let run = start_with(
        HarnessId::Claude,
        "claude",
        "claude_utility.jsonl",
        options(),
        |request| {
            request.utility = true;
            request.approval_override = Some(ApprovalMode::AlwaysAsk);
        },
    )
    .await
    .expect("the helper starts");

    let args = args(&run);
    assert!(
        args.ends_with(&[
            "--permission-mode".to_owned(),
            "default".to_owned(),
            "--tools".to_owned(),
            String::new(),
            "--thinking=disabled".to_owned(),
            "--no-session-persistence".to_owned(),
        ]),
        "{args:?}"
    );
    assert!(!args.iter().any(|arg| arg == "--session-id" || arg == "--model"));

    let prompt = AgentAction::Prompt {
        message: "Give a 3 word title for a chat about fixing a login bug. Reply with the title only.".to_owned(),
        images: Vec::new(),
        streaming_behavior: None,
    };
    run.manager.send(run.key(), prompt).await.expect("sent");
    run.wait("settled", |event| matches!(event, AgentEvent::Settled { .. }))
        .await;

    let reply = run
        .manager
        .send(run.key(), AgentAction::GetLastAssistantText)
        .await
        .expect("the reply");
    assert_eq!(
        reply,
        AgentReply::Text {
            text: Some("Login Bug Fix".to_owned())
        }
    );

    let refused = run
        .manager
        .send(run.key(), AgentAction::NewSession)
        .await
        .expect_err("nothing saved");
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);

    stop_cleanly(&run).await;
}

#[tokio::test]
async fn a_helper_chat_denies_a_permission_prompt_at_once_without_a_dialog() {
    let run = start_with(
        HarnessId::Claude,
        "claude",
        "claude_utility_ask.jsonl",
        options(),
        |request| {
            request.utility = true;
        },
    )
    .await
    .expect("the helper starts");

    run.wait(
        "the marker after the answer",
        |event| matches!(event, AgentEvent::State { state } if state.session_name.as_deref() == Some("Marker")),
    )
    .await;
    assert!(
        !run.events()
            .iter()
            .any(|event| matches!(event, AgentEvent::UiRequest { .. })),
        "nobody sees a helper's dialogs"
    );

    stop_cleanly(&run).await;
}
