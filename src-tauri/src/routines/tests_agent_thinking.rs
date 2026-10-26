//! The thinking level of an agent routine's model: set after the model and before the prompt, and a
//! level the agent refuses fails the run, naming it.

use harness_business::model::{RoutineAction, RoutineModel, RunStatus};

use super::tests_agent::{Fixture, REPLY, agent_with, timing};
use crate::test_support::wait_until;

/// An OMP agent action on `openai/gpt-5` with the thinking level `level`.
fn agent_thinking(level: &str) -> RoutineAction {
    let RoutineAction::Agent {
        harness,
        prompt,
        approval_mode,
        trust_once,
        max_minutes,
        ..
    } = agent_with("Check the build", Some(("openai", "gpt-5")), None)
    else {
        unreachable!("an agent action");
    };

    RoutineAction::Agent {
        harness,
        model: Some(RoutineModel {
            provider: "openai".to_owned(),
            id: "gpt-5".to_owned(),
            thinking: Some(level.to_owned()),
        }),
        prompt,
        approval_mode,
        trust_once,
        max_minutes,
    }
}

#[test]
fn a_run_sets_its_thinking_level_after_its_model_and_fails_when_the_agent_refuses_it() {
    let Some(fixture) = Fixture::new("omp_routine_thinking.jsonl", timing()) else {
        return;
    };

    // The scenario only answers the thinking level after the model and before the prompt.
    let (id, _) = fixture.run(agent_thinking("high"));
    let run = fixture.ended(&id);
    assert_eq!(run.record.status, RunStatus::Ok, "{:?}", run.record.message);
    assert_eq!(run.record.excerpt.as_deref(), Some(REPLY));

    let Some(refused) = Fixture::new("omp_routine_thinking_refused.jsonl", timing()) else {
        return;
    };

    let (id, _) = refused.run(agent_thinking("ultra"));
    let run = refused.ended(&id);
    assert_eq!(run.record.status, RunStatus::Failed);
    assert_eq!(
        run.record.message.as_deref(),
        Some("The thinking level ultra is not available for openai/gpt-5")
    );
    wait_until("the agent to stop", || refused.chats().is_empty());
}
