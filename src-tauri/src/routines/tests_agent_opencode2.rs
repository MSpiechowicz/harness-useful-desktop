//! OpenCode 2 agent routine runs against the scripted OpenCode 2 `opencode serve`
//! (`opencode2_routine_*.jsonl`), as on OpenCode 1: an unattended run with approval mode write and a
//! model takes its turn, writes a file without an ask and ends with the reply and the session tag;
//! a permission ask nobody answers is rejected, a question nobody answers is dismissed, and the run
//! needs the user.

use std::sync::Arc;
use std::time::Duration;

use harness_business::model::{ApprovalMode, HarnessId, RoutineAction, RoutineModel, RunSession, RunStatus};
use serde_json::Value;

use super::agent_run::AgentTiming;
use super::relay::RelaySink;
use super::tests_agent::{Fixture, timing};
use crate::test_support::{collecting_channel, wait_until};

/// The OpenCode 2 session every scenario creates (`opencode2_startup.jsonl`).
const SESSION: &str = "ses_ea2843d6fffeeHJKZwBKUXqULv";

/// An OpenCode agent action that sends `prompt` with `approval_mode` and `model`.
fn opencode(prompt: &str, approval_mode: Option<ApprovalMode>, model: Option<RoutineModel>) -> RoutineAction {
    RoutineAction::Agent {
        harness: HarnessId::OpenCode,
        model,
        prompt: prompt.to_owned(),
        approval_mode,
        trust_once: false,
        max_minutes: None,
    }
}

/// Runs `prompt`, which the agent stalls on with a request nobody answers, and checks that the
/// server got the scenario's answer. The chat is stopped before the fake could exit with its mismatch
/// code, so the check is the fake's report of any other request on stderr (`fake_agent: …`).
fn assert_unanswered_request_is_closed(scenario: &str, prompt: &str) {
    let quick = AgentTiming {
        ui_wait: Duration::from_millis(300),
        ..timing()
    };
    let Some(fixture) = Fixture::opencode2(scenario, quick) else {
        return;
    };
    let relay = fixture.run_unanswered(opencode(prompt, None, None));

    wait_until("the server's exit", || {
        events(&relay).iter().any(|event| event["type"] == "exited")
    });

    let mismatches: Vec<Value> = events(&relay)
        .into_iter()
        .filter(|event| event["type"] == "stderr")
        .filter(|event| {
            event["line"]
                .as_str()
                .is_some_and(|line| line.starts_with("fake_agent:"))
        })
        .collect();
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

/// The events of the run's chat so far.
fn events(relay: &Arc<RelaySink>) -> Vec<Value> {
    let (window, received) = collecting_channel();
    relay.attach(window).expect("the chat's events");
    let messages = received.lock().unwrap();

    messages.iter().map(|message| message["event"].clone()).collect()
}

#[test]
fn an_unattended_opencode2_run_in_approval_mode_write_writes_a_file_and_finishes() {
    let Some(fixture) = Fixture::opencode2("opencode2_routine_write.jsonl", timing()) else {
        return;
    };
    let model = RoutineModel {
        provider: "opencode".to_owned(),
        id: "longcat-2.5-preview-free".to_owned(),
    };

    // The start is refused unless the server's agents show the write rules; the model goes to the
    // session before the prompt.
    let (id, run_id) = fixture.run(opencode("Check the build", Some(ApprovalMode::Write), Some(model)));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(run.record.excerpt.as_deref(), Some("Checked the build: all green."));
    assert_eq!(
        run.record.session,
        Some(RunSession {
            harness: HarnessId::OpenCode,
            id: SESSION.to_owned(),
        })
    );
    wait_until("the server to stop", || fixture.chats().is_empty());
    assert_eq!(fixture.setup.spawner.spawned(), 2, "the probe, then the chat's server");

    let tags = fixture.state.routines.session_tags();
    let tag = tags.get(HarnessId::OpenCode, SESSION).expect("the session is tagged");
    assert_eq!(
        (tag.routine_id.as_str(), tag.run_id.as_str()),
        (id.as_str(), run_id.as_str())
    );
}

#[test]
fn an_opencode2_permission_ask_nobody_answers_is_rejected_and_the_run_needs_attention() {
    assert_unanswered_request_is_closed("opencode2_routine_ask.jsonl", "Run the tests");
}

#[test]
fn an_opencode2_question_nobody_answers_is_dismissed_and_the_run_needs_attention() {
    assert_unanswered_request_is_closed("opencode2_routine_ask_question.jsonl", "Pick a colour");
}
