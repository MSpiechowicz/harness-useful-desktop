//! What an agent run waits for: an answer to its question (the view hears when it no longer
//! waits), the settle that follows its prompt (not one from before it, and none for a prompt the
//! harness handled itself), and how a failed prompt ends the agent even while a window shows the
//! chat.

use std::time::Duration;

use harness_business::model::{RunStatus, UiOutcome, UiResponse};
use tauri::async_runtime::block_on;

use super::agent_run::{AGENT_FAILED, AgentTiming};
use super::tests::runs_of;
use super::tests_agent::{Fixture, REPLY, agent, agent_with, timing};
use crate::commands::agent;
use crate::test_support::{collecting_channel, wait_until};

#[test]
fn an_answered_question_no_longer_needs_the_user() {
    let Some(fixture) = Fixture::new("omp_routine_answered.jsonl", timing()) else {
        return;
    };
    let (listener, events) = collecting_channel();
    fixture.state.routines.listen(listener);

    let (id, run_id) = fixture.run(agent("Deploy"));
    let key = fixture.relay(&run_id).key().expect("the chat's key");
    wait_until("the question", || fixture.needs_you(&id));
    let heard = |kind: &str| {
        events.lock().unwrap().iter().any(|event| {
            event["kind"] == kind && event["runId"] == run_id.as_str() && event["routineId"] == id.as_str()
        })
    };
    wait_until("the view to hear it", || heard("needsYou"));
    assert!(!heard("needsYouCleared"));

    let answer = UiResponse {
        id: "q-1".to_owned(),
        outcome: UiOutcome::Confirmed { confirmed: true },
    };
    block_on(agent::respond(&fixture.state, &key, answer)).expect("answered");

    wait_until("the run to go on", || {
        !fixture.needs_you(&id) && runs_of(&fixture.state, &id)[0].record.status == RunStatus::Running
    });
    wait_until("the view to hear the run no longer waits", || heard("needsYouCleared"));

    let run = fixture.ended(&id);
    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(run.record.excerpt.as_deref(), Some(REPLY));
}

#[test]
fn a_settle_before_the_prompt_does_not_end_the_run() {
    let Some(fixture) = Fixture::new("omp_routine_early_settle.jsonl", timing()) else {
        return;
    };

    let (id, _) = fixture.run(agent("Check the build"));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(
        run.record.excerpt.as_deref(),
        Some(REPLY),
        "the run waited for the reply to its prompt"
    );
}

#[test]
fn a_prompt_the_harness_handled_itself_ends_the_run_at_once() {
    // Without the end of the prompt, the run would wait out its one (short) minute and fail.
    let short = AgentTiming {
        minute: Duration::from_secs(5),
        ..timing()
    };
    let Some(fixture) = Fixture::new("omp_routine_handled.jsonl", short) else {
        return;
    };

    let (id, _) = fixture.run(agent_with("/compact", None, Some(1)));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert!(
        run.record.duration_ms.is_some_and(|ms| ms < 4_000),
        "{:?}",
        run.record.duration_ms
    );
}

#[test]
fn a_failed_prompt_stops_the_agent_a_window_shows() {
    let Some(fixture) = Fixture::new("omp_routine_failed_slow.jsonl", timing()) else {
        return;
    };

    let (id, _, _, _open) = fixture.run_shown(agent("Check the build"));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Failed);
    assert_eq!(run.record.message.as_deref(), Some(AGENT_FAILED));
    wait_until("the agent to stop", || fixture.chats().is_empty());
}
