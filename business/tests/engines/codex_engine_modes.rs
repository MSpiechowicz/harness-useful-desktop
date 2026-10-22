//! Approval modes of Codex chats: each mode starts (or, after a mode change, resumes) the thread
//! with its approval policy and sandbox, no override sends neither and takes the mode Codex
//! answers with, Auto is refused before anything spawns, `!command`s are refused in Always ask,
//! and routines take the modes Codex offers.

use crate::codex_engine_support;

use codex_engine_support::{CodexChat, Launched, Setup, THREAD, launch_with, start_with};
use harness_business::agent::{AgentSession, CodexSession, codex_bash_allowed};
use harness_business::harness::Harness;
use harness_business::harnesses::Codex;
use harness_business::model::{AgentEvent, ApprovalMode, HarnessId, HookScope, IpcErrorCode, RoutineAction};
use harness_business::routines::format::check_action;
use pretty_assertions::assert_eq;

fn mode(approval_override: ApprovalMode) -> Setup {
    Setup {
        approval_override: Some(approval_override),
        ..Setup::default()
    }
}

fn codex(session: &AgentSession) -> &CodexSession {
    match session {
        AgentSession::Codex(session) => session,
        _ => panic!("not the Codex engine"),
    }
}

/// Starts `scenario` in `setup`; the chat's mode once it opened.
async fn started_mode(scenario: &str, setup: Setup) -> (CodexChat, Option<ApprovalMode>) {
    let chat = start_with(scenario, setup, |_| {}).await;
    let mode = codex(&chat.session).approval_mode();

    (chat, mode)
}

#[tokio::test]
async fn each_mode_starts_the_thread_with_its_approval_policy_and_sandbox() {
    let cases = [
        ("codex_engine_mode_ask.jsonl", ApprovalMode::AlwaysAsk),
        ("codex_engine_mode_write.jsonl", ApprovalMode::Write),
        ("codex_engine_mode_yolo.jsonl", ApprovalMode::Yolo),
    ];

    for (scenario, approval) in cases {
        let (chat, started) = started_mode(scenario, mode(approval)).await;

        assert_eq!(started, Some(approval), "{scenario}");
        chat.finish().await;
    }
}

#[tokio::test]
async fn without_an_override_nothing_is_sent_and_the_mode_is_the_one_codex_answers() {
    let (chat, started) = started_mode("codex_engine_mode_none.jsonl", Setup::default()).await;

    assert_eq!(started, Some(ApprovalMode::Write), "Codex answered on-request");
    chat.finish().await;
}

#[tokio::test]
async fn a_mode_change_resumes_the_thread_with_the_new_mode() {
    let Launched { result, sink, .. } = launch_with(
        "codex_engine_mode_resume.jsonl",
        Some(THREAD),
        mode(ApprovalMode::Yolo),
        |_| {},
    )
    .await;
    let session = result.expect("the restarted chat starts");

    assert_eq!(codex(&session).approval_mode(), Some(ApprovalMode::Yolo));

    session.stop().await;
    let exit = AgentEvent::Exited {
        code: Some(0),
        signal: None,
    };
    assert_eq!(
        sink.events().last(),
        Some(&exit),
        "the scripted Codex saw the Yolo resume"
    );
}

#[tokio::test]
async fn auto_is_refused_before_anything_spawns() {
    let launched = launch_with("codex_engine_open.jsonl", None, mode(ApprovalMode::Auto), |_| {}).await;

    let refused = launched.result.as_ref().err().expect("no Auto mode");
    assert_eq!(refused.code(), IpcErrorCode::Unsupported);
    assert!(launched.spawner.requested().is_empty());
}

#[test]
fn codex_offers_always_ask_write_and_yolo_and_trust_once_without_requiring_trust() {
    let capabilities = Codex.capabilities();

    assert!(capabilities.approval_modes);
    assert!(capabilities.yolo_mode);
    assert!(!capabilities.auto_mode);
    assert!(capabilities.project_trust);
    assert!(!capabilities.project_trust_required);
}

#[test]
fn a_command_is_refused_in_always_ask_only() {
    let refused = codex_bash_allowed(Some(ApprovalMode::AlwaysAsk)).unwrap_err();
    assert_eq!(refused.code(), IpcErrorCode::Forbidden);

    for allowed in [None, Some(ApprovalMode::Write), Some(ApprovalMode::Yolo)] {
        assert!(codex_bash_allowed(allowed).is_ok(), "{allowed:?}");
    }
}

#[tokio::test]
async fn a_chat_in_always_ask_refuses_commands() {
    let (chat, started) = started_mode("codex_engine_mode_ask.jsonl", mode(ApprovalMode::AlwaysAsk)).await;

    assert!(codex_bash_allowed(started).is_err());
    chat.finish().await;
}

/// An agent routine of `harness`; `None` if [`check_action`] takes it, else its refusal.
fn refusal(harness: HarnessId, approval_mode: Option<ApprovalMode>, trust_once: bool) -> Option<String> {
    let action = RoutineAction::Agent {
        harness,
        model: None,
        prompt: "Tidy up.".to_owned(),
        approval_mode,
        trust_once,
        max_minutes: None,
    };

    check_action(&action, HookScope::Global).err()
}

#[test]
#[ignore = "Codex chat turns on in S10: until then every Codex agent routine is refused"]
fn routines_take_the_modes_codex_offers() {
    for approval in [ApprovalMode::AlwaysAsk, ApprovalMode::Write, ApprovalMode::Yolo] {
        assert_eq!(refusal(HarnessId::Codex, Some(approval), false), None, "{approval:?}");
    }

    assert_eq!(refusal(HarnessId::Codex, None, true), None);

    let auto = refusal(HarnessId::Codex, Some(ApprovalMode::Auto), false).expect("no Auto");
    assert!(auto.contains("\"auto\" is for"), "{auto}");

    let yolo = refusal(HarnessId::Claude, Some(ApprovalMode::Yolo), false).expect("no Yolo on Claude Code");
    assert!(yolo.ends_with("is for omp, opencode or codex only"), "{yolo}");
}
