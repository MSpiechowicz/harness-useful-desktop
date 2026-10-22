//! Claude Code agent routine runs against the scripted `claude` (`claude_basic.jsonl`): in a folder
//! Claude Code does not trust yet a run is refused before anything starts, unless the routine
//! trusts the project once.

use harness_business::model::{HarnessId, RoutineAction, RunStatus};

use super::agent_run::UNTRUSTED_MESSAGE;
use super::tests_agent::{Fixture, timing};
use crate::test_support::wait_until;

/// A Claude Code agent action that sends the prompt of `claude_basic.jsonl`.
fn claude(trust_once: bool) -> RoutineAction {
    RoutineAction::Agent {
        harness: HarnessId::Claude,
        model: None,
        prompt: "Reply with exactly the word: pong".to_owned(),
        approval_mode: None,
        trust_once,
        max_minutes: None,
    }
}

#[test]
fn a_claude_run_in_a_folder_claude_does_not_trust_fails_before_it_starts() {
    let Some(fixture) = Fixture::of(HarnessId::Claude, "claude_basic.jsonl", timing()) else {
        return;
    };

    let (id, _) = fixture.run(claude(false));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Failed);
    assert_eq!(run.record.message.as_deref(), Some(UNTRUSTED_MESSAGE));
    assert_eq!(fixture.setup.spawner.spawned(), 0, "nothing ran");
}

#[test]
fn a_claude_run_that_trusts_the_project_once_takes_its_turn() {
    let Some(fixture) = Fixture::of(HarnessId::Claude, "claude_basic.jsonl", timing()) else {
        return;
    };

    let (id, _) = fixture.run(claude(true));
    let run = fixture.ended(&id);

    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(run.record.excerpt.as_deref(), Some("pong"));
    wait_until("the agent to stop", || fixture.chats().is_empty());
    assert_eq!(fixture.setup.spawner.spawned(), 1);
}
