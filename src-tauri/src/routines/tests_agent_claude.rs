//! Claude Code agent routine runs against the scripted `claude` (`fake_agent --dialect claude`,
//! `claude_routine_*.jsonl`), as for the other harnesses: an unattended run with approval mode
//! write and a model takes its turn and ends with the reply and the session tag; a permission
//! prompt nobody answers (Always ask or Write) is denied, a question nobody answers is dismissed,
//! and the run needs the user. A Claude Code routine in Yolo is never saved. The routines folder is
//! one Claude Code does not trust: a run there is refused before anything starts, unless the
//! routine trusts the project once, as the runs above do.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use harness_business::model::{
    ApprovalMode, HarnessId, HookAuthor, RoutineAction, RoutineModel, RoutineWhen, RunSession, RunStatus,
};
use serde_json::Value;

use super::agent_run::{AgentTiming, UNTRUSTED_MESSAGE};
use super::relay::RelaySink;
use super::tests::draft;
use super::tests_agent::{Fixture, timing};
use crate::test_support::{collecting_channel, wait_until};

/// The session of the recorded turn `claude_routine_write.jsonl` replays (basic.jsonl).
const SESSION: &str = "c1262da3-a66b-42b5-a9ee-32bb98f2045d";

/// A Claude Code agent action that sends `prompt` with `approval_mode` and `model`, trusting the
/// routines folder once.
fn claude(prompt: &str, approval_mode: Option<ApprovalMode>, model: Option<RoutineModel>) -> RoutineAction {
    RoutineAction::Agent {
        harness: HarnessId::Claude,
        model,
        prompt: prompt.to_owned(),
        approval_mode,
        trust_once: true,
        max_minutes: None,
    }
}

/// The prompt of `claude_basic.jsonl`, trusting the folder once when `trust_once`.
fn pong(trust_once: bool) -> RoutineAction {
    RoutineAction::Agent {
        harness: HarnessId::Claude,
        model: None,
        prompt: "Reply with exactly the word: pong".to_owned(),
        approval_mode: None,
        trust_once,
        max_minutes: None,
    }
}

/// Claude Code chats that play `scenario`; `None` (the test skips) when `fake_agent` is not built.
fn fixture(scenario: &str, timing: AgentTiming) -> Option<Fixture> {
    Fixture::of(HarnessId::Claude, scenario, timing)
}

/// Runs `prompt` in `approval_mode`, which the agent stalls on with an ask nobody answers, and
/// checks that the CLI got the scenario's answer. The chat is stopped before the fake could exit
/// with its mismatch code, so the check is the fake's report of any other answer on stderr
/// (`fake_agent: …`).
fn assert_unanswered_ask_is_closed(scenario: &str, prompt: &str, approval_mode: Option<ApprovalMode>) {
    let quick = AgentTiming {
        ui_wait: Duration::from_millis(300),
        ..timing()
    };
    let Some(fixture) = fixture(scenario, quick) else {
        return;
    };
    let relay = fixture.run_unanswered(claude(prompt, approval_mode, None));

    wait_until("the CLI's exit", || {
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
fn an_unattended_claude_run_in_approval_mode_write_takes_its_turn_and_finishes() {
    let Some(fixture) = fixture("claude_routine_write.jsonl", timing()) else {
        return;
    };
    let model = RoutineModel {
        provider: "anthropic".to_owned(),
        id: "sonnet".to_owned(),
    };

    // The CLI refuses a start with the Always ask hook and a prompt before the model.
    let (id, run_id) = fixture.run(claude("Check the build", Some(ApprovalMode::Write), Some(model)));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(run.record.excerpt.as_deref(), Some("Checked the build: all green."));
    assert_eq!(
        run.record.session,
        Some(RunSession {
            harness: HarnessId::Claude,
            id: SESSION.to_owned(),
        })
    );
    wait_until("the CLI to stop", || fixture.chats().is_empty());
    assert_eq!(fixture.setup.spawner.spawned(), 1, "one CLI process");

    let tags = fixture.state.routines.session_tags();
    let tag = tags.get(HarnessId::Claude, SESSION).expect("the session is tagged");
    assert_eq!(
        (tag.routine_id.as_str(), tag.run_id.as_str()),
        (id.as_str(), run_id.as_str())
    );
}

#[test]
fn a_claude_permission_prompt_nobody_answers_in_always_ask_is_denied_and_the_run_needs_attention() {
    assert_unanswered_ask_is_closed(
        "claude_routine_ask.jsonl",
        "Run the tests",
        Some(ApprovalMode::AlwaysAsk),
    );
}

#[test]
fn a_claude_permission_prompt_nobody_answers_in_write_is_denied_and_the_run_needs_attention() {
    assert_unanswered_ask_is_closed(
        "claude_routine_ask_write.jsonl",
        "Write the report",
        Some(ApprovalMode::Write),
    );
}

#[test]
fn a_claude_question_nobody_answers_is_dismissed_and_the_run_needs_attention() {
    assert_unanswered_ask_is_closed("claude_routine_question.jsonl", "Pick a colour", None);
}

#[test]
fn a_claude_routine_in_yolo_is_refused() {
    let Some(fixture) = fixture("claude_routine_write.jsonl", timing()) else {
        return;
    };
    let store = fixture.state.routines.store().expect("a routines folder");
    let action = claude("Check the build", Some(ApprovalMode::Yolo), None);

    let refused = store
        .save(
            &draft("Nightly check", RoutineWhen::AppStart, action),
            HookAuthor::Desktop,
            SystemTime::now(),
        )
        .expect_err("no Yolo for Claude Code");

    assert!(
        refused.to_string().contains("\"yolo\" is for omp or opencode only"),
        "{refused}"
    );
    assert_eq!(fixture.setup.spawner.spawned(), 0, "nothing started");
}

#[test]
fn a_claude_run_in_a_folder_claude_does_not_trust_fails_before_it_starts() {
    let Some(fixture) = fixture("claude_basic.jsonl", timing()) else {
        return;
    };

    let (id, _) = fixture.run(pong(false));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Failed);
    assert_eq!(run.record.message.as_deref(), Some(UNTRUSTED_MESSAGE));
    assert_eq!(fixture.setup.spawner.spawned(), 0, "nothing ran");
}

#[test]
fn a_claude_run_that_trusts_the_project_once_takes_its_turn() {
    let Some(fixture) = fixture("claude_basic.jsonl", timing()) else {
        return;
    };

    let (id, _) = fixture.run(pong(true));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(run.record.excerpt.as_deref(), Some("pong"));
    wait_until("the agent to stop", || fixture.chats().is_empty());
    assert_eq!(fixture.setup.spawner.spawned(), 1);
}
