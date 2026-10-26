//! Claude Code agent routine runs against the scripted `claude` (`fake_agent --dialect claude`,
//! `claude_routine_*.jsonl`), as for the other harnesses: an unattended run with approval mode
//! write and a model takes its turn and ends with the reply and the session tag; a permission
//! prompt nobody answers (Always ask or Write) is denied, a question nobody answers is dismissed,
//! and the run needs the user. A Claude Code routine in Yolo is never saved. The routines folder is
//! one Claude Code does not trust: a run there is refused before anything starts, unless the
//! routine trusts the project once, as the runs above do.

use harness_business::model::{ApprovalMode, HarnessId, RoutineAction, RoutineModel, RunSession, RunStatus};

use super::agent_run::{AgentTiming, UNTRUSTED_MESSAGE};
use super::tests_agent::{Fixture, timing};
use super::tests_agent_cli::{
    assert_run_ends_tagged, assert_run_takes_its_turn, assert_save_refused, assert_unanswered_ask_is_closed,
};

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

/// Runs `prompt` in `approval_mode`, which the CLI stalls on with an ask nobody answers, and checks
/// that the CLI got the scenario's answer.
fn assert_unanswered_claude_ask_is_closed(scenario: &str, prompt: &str, approval_mode: Option<ApprovalMode>) {
    assert_unanswered_ask_is_closed(HarnessId::Claude, scenario, claude(prompt, approval_mode, None));
}

#[test]
fn an_unattended_claude_run_in_approval_mode_write_takes_its_turn_and_finishes() {
    let Some(fixture) = fixture("claude_routine_write.jsonl", timing()) else {
        return;
    };
    let model = RoutineModel {
        provider: "anthropic".to_owned(),
        id: "sonnet".to_owned(),
        thinking: None,
    };
    let session = RunSession {
        harness: HarnessId::Claude,
        id: SESSION.to_owned(),
    };

    // The CLI refuses a start with the Always ask hook and a prompt before the model.
    let action = claude("Check the build", Some(ApprovalMode::Write), Some(model));
    assert_run_ends_tagged(&fixture, action, &session, "Checked the build: all green.");
}

#[test]
fn a_claude_permission_prompt_nobody_answers_in_always_ask_is_denied_and_the_run_needs_attention() {
    assert_unanswered_claude_ask_is_closed(
        "claude_routine_ask.jsonl",
        "Run the tests",
        Some(ApprovalMode::AlwaysAsk),
    );
}

#[test]
fn a_claude_permission_prompt_nobody_answers_in_write_is_denied_and_the_run_needs_attention() {
    assert_unanswered_claude_ask_is_closed(
        "claude_routine_ask_write.jsonl",
        "Write the report",
        Some(ApprovalMode::Write),
    );
}

#[test]
fn a_claude_question_nobody_answers_is_dismissed_and_the_run_needs_attention() {
    assert_unanswered_claude_ask_is_closed("claude_routine_question.jsonl", "Pick a colour", None);
}

#[test]
fn a_claude_routine_in_yolo_is_refused() {
    let Some(fixture) = fixture("claude_routine_write.jsonl", timing()) else {
        return;
    };
    let action = claude("Check the build", Some(ApprovalMode::Yolo), None);

    assert_save_refused(&fixture, action, "\"yolo\" is for omp, opencode or codex only");
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

    assert_run_takes_its_turn(&fixture, pong(true), "pong");
}
