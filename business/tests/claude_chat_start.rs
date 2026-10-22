//! A Claude Code chat through the agent manager, now that the harness offers chat (a preview): a
//! new chat starts and binds to the session it names, a helper chat (AI naming) runs without
//! tools, MCP servers, thinking or a saved session, answers its one prompt and denies nothing it is
//! not asked, and a folder Claude Code does not trust starts only when the launch trusts it once.

mod agent_support;

use std::path::Path;

use agent_support::{options, start_in, start_with};
use harness_business::env::Env;
use harness_business::harness::Harness;
use harness_business::harnesses::Claude;
use harness_business::harnesses::claude::global_config_file;
use harness_business::model::{AgentAction, AgentEvent, AgentReply, ApprovalMode, HarnessId, IpcErrorCode};
use pretty_assertions::assert_eq;
use serde_json::json;

/// What a chat in a folder Claude Code does not trust is refused with.
const UNTRUSTED: &str =
    "Claude Code does not trust this folder yet. Run claude in it once from a terminal, or trust it once here.";

/// `env` with a Claude global file that names no project.
fn no_trusted_folder(env: Env) -> Env {
    std::fs::write(global_config_file(&env), json!({ "projects": {} }).to_string()).expect("write the global file");
    env
}

/// The test home with `global` as its Claude global file (`None`: no file).
fn home_with(global: Option<&str>) -> (tempfile::TempDir, Env) {
    let home = tempfile::tempdir().expect("temp home");
    let env = Env::for_tests(home.path());

    if let Some(text) = global {
        std::fs::write(global_config_file(&env), text).expect("write the global file");
    }

    (home, env)
}

fn trust_entry(project: &Path, answer: serde_json::Value) -> String {
    json!({ "projects": { project.to_string_lossy(): { "hasTrustDialogAccepted": answer } } }).to_string()
}

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

#[test]
fn only_a_folder_whose_trust_dialog_was_accepted_counts_as_trusted() {
    let project = tempfile::tempdir().expect("project");
    let project = project.path();
    let refused = |global: Option<String>| {
        let (_home, env) = home_with(global.as_deref());
        Claude.check_project_trust(&env, project)
    };

    assert!(Claude.capabilities().project_trust && Claude.capabilities().project_trust_required);
    assert!(refused(Some(trust_entry(project, json!(true)))).is_ok());

    for global in [
        None,
        Some("{ not json".to_owned()),
        Some(json!({ "hasTrustDialogAccepted": true }).to_string()),
        Some(trust_entry(project, json!(false))),
        Some(trust_entry(project, json!("true"))),
        Some(trust_entry(&project.join("sub"), json!(true))),
    ] {
        let error = refused(global.clone()).expect_err("not trusted");
        assert_eq!(error.code(), IpcErrorCode::Untrusted, "{global:?}");
        assert_eq!(error.to_string(), UNTRUSTED);
    }
}

#[tokio::test]
async fn a_chat_or_helper_in_a_folder_claude_does_not_trust_is_refused_unless_trusted_once() {
    for utility in [false, true] {
        let refused = start_in(
            HarnessId::Claude,
            "claude",
            "claude_init.jsonl",
            options(),
            no_trusted_folder,
            |request| request.utility = utility,
        )
        .await
        .err()
        .expect("refused before spawning");
        assert_eq!(refused.code(), IpcErrorCode::Untrusted, "utility {utility}");
        assert_eq!(refused.to_string(), UNTRUSTED);
    }

    let run = start_in(
        HarnessId::Claude,
        "claude",
        "claude_init.jsonl",
        options(),
        no_trusted_folder,
        |request| request.trust_once = true,
    )
    .await
    .expect("trusted once, the chat starts");
    assert!(run.info.trusted_once);
    assert_eq!(run.spawner.requested().len(), 1);

    run.manager.begin_stop_all().finish().await;
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
async fn a_helper_chat_answers_one_prompt_without_tools_mcp_servers_thinking_or_a_saved_session() {
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
            "--strict-mcp-config".to_owned(),
            "--thinking=disabled".to_owned(),
            "--no-session-persistence".to_owned(),
        ]),
        "{args:?}"
    );
    assert!(
        !args
            .iter()
            .any(|arg| arg == "--session-id" || arg.starts_with("--model"))
    );

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
