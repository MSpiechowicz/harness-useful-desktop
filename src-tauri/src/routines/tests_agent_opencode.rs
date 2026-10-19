//! OpenCode agent routine runs against the scripted `opencode serve` (`opencode_routine_*.jsonl`):
//! an unattended run with approval mode write and a model takes its turn and ends with the reply
//! and the session tag, and a permission ask nobody answers is rejected and the run needs the
//! user, as for OMP.

use std::time::Duration;

use harness_business::model::{ApprovalMode, HarnessId, RoutineAction, RoutineModel, RunSession, RunStatus};

use super::agent_run::AgentTiming;
use super::tests_agent::{Fixture, timing};
use crate::test_support::{collecting_channel, wait_until};

/// The OpenCode session every scenario creates (`opencode_startup.jsonl`).
const SESSION: &str = "ses_eaa561636ffezINnnn1ghd5370";

/// The exit code of the scripted server after a request it did not expect.
const FAKE_MISMATCH: i32 = 4;

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

#[test]
fn an_unattended_opencode_run_in_approval_mode_write_takes_its_turn_and_finishes() {
    let Some(fixture) = Fixture::of(HarnessId::OpenCode, "opencode_routine_write.jsonl", timing()) else {
        return;
    };
    let model = RoutineModel {
        provider: "opencode".to_owned(),
        id: "longcat-2.5-preview-free".to_owned(),
    };

    // The server refuses a start without the write rules and a prompt without the model.
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

    let tags = fixture.state.routines.session_tags();
    let tag = tags.get(HarnessId::OpenCode, SESSION).expect("the session is tagged");
    assert_eq!(
        (tag.routine_id.as_str(), tag.run_id.as_str()),
        (id.as_str(), run_id.as_str())
    );
}

#[test]
fn an_opencode_permission_ask_nobody_answers_is_rejected_and_the_run_needs_attention() {
    let quick = AgentTiming {
        ui_wait: Duration::from_millis(300),
        ..timing()
    };
    let Some(fixture) = Fixture::of(HarnessId::OpenCode, "opencode_routine_ask.jsonl", quick) else {
        return;
    };
    let relay = fixture.run_unanswered(opencode("Run the tests", None, None));

    // The server got "reject" for its ask: any other answer makes it exit with its mismatch code.
    let exited = || {
        let (window, received) = collecting_channel();
        relay.attach(window).expect("the chat's events");
        let messages = received.lock().unwrap();

        messages
            .iter()
            .find(|message| message["event"]["type"] == "exited")
            .map(|message| message["event"].clone())
    };
    wait_until("the server's exit", || exited().is_some());

    let exit = exited().expect("the server exited");
    assert_ne!(exit["code"], FAKE_MISMATCH, "{exit}");
}
