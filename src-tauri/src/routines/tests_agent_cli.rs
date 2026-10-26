//! What the agent routine tests of Claude Code and Codex share (`tests_agent_claude`,
//! `tests_agent_codex`): a run that takes its turn (and tags its session), an ask nobody answers,
//! and an action the store refuses.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use harness_business::model::{HarnessId, HookAuthor, RoutineAction, RoutineRun, RoutineWhen, RunSession, RunStatus};
use serde_json::Value;

use super::agent_run::AgentTiming;
use super::relay::RelaySink;
use super::tests::draft;
use super::tests_agent::{Fixture, timing};
use crate::test_support::{collecting_channel, wait_until};

/// Runs `action` and checks that it ended well with `reply` in `session`, which is tagged with the
/// routine and the run, and that its one agent stopped.
pub(super) fn assert_run_ends_tagged(fixture: &Fixture, action: RoutineAction, session: &RunSession, reply: &str) {
    let (id, run_id, run) = assert_run_takes_its_turn(fixture, action, reply);
    assert_eq!(run.record.session.as_ref(), Some(session));

    let tags = fixture.state.routines.session_tags();
    let tag = tags.get(session.harness, &session.id).expect("the session is tagged");
    assert_eq!(
        (tag.routine_id.as_str(), tag.run_id.as_str()),
        (id.as_str(), run_id.as_str())
    );
}

/// Runs `action` of `harness`, which the agent stalls on with an ask nobody answers, and checks
/// that the agent got the scenario's answer. The chat is stopped before the fake could exit with
/// its mismatch code, so the check is the fake's report of any other answer on stderr
/// (`fake_agent: …`).
pub(super) fn assert_unanswered_ask_is_closed(harness: HarnessId, scenario: &str, action: RoutineAction) {
    let quick = AgentTiming {
        ui_wait: Duration::from_millis(300),
        ..timing()
    };
    let Some(fixture) = Fixture::of(harness, scenario, quick) else {
        return;
    };
    let relay = fixture.run_unanswered(action);

    wait_until("the agent's exit", || {
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

/// Checks that the routine store refuses to save `action` with a reason that names `reason`, and
/// that nothing started.
pub(super) fn assert_save_refused(fixture: &Fixture, action: RoutineAction, reason: &str) {
    let store = fixture.state.routines.store().expect("a routines folder");

    let refused = store
        .save(
            &draft("Nightly check", RoutineWhen::AppStart, action),
            HookAuthor::Desktop,
            SystemTime::now(),
        )
        .expect_err("the action is refused");

    assert!(refused.to_string().contains(reason), "{refused}");
    assert_eq!(fixture.setup.spawner.spawned(), 0, "nothing started");
}

/// Runs `action` and checks that it ended well with `reply`, its one agent stopped; answers the
/// routine's id, the run's and the run.
pub(super) fn assert_run_takes_its_turn(
    fixture: &Fixture,
    action: RoutineAction,
    reply: &str,
) -> (String, String, RoutineRun) {
    let (id, run_id) = fixture.run(action);
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(run.record.excerpt.as_deref(), Some(reply));
    wait_until("the agent to stop", || fixture.chats().is_empty());
    assert_eq!(fixture.setup.spawner.spawned(), 1, "one agent process");

    (id, run_id, run)
}

/// The events of the run's chat so far.
fn events(relay: &Arc<RelaySink>) -> Vec<Value> {
    let (window, received) = collecting_channel();
    relay.attach(window).expect("the chat's events");
    let messages = received.lock().unwrap();

    messages.iter().map(|message| message["event"].clone()).collect()
}
