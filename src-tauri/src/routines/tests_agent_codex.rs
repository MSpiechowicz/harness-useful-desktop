//! Codex agent routine runs against the scripted `codex app-server` (`fake_agent --dialect codex`,
//! `codex_routine_*.jsonl`), as for the other harnesses: an unattended run with approval mode
//! write and a model takes its turn (the model goes with it) and ends with the reply and the
//! thread as its session tag; an approval nobody gives (a command in Always ask, a file change in
//! Write) is declined, a question nobody answers gets no answers, and the run needs the user. A
//! Codex routine in Auto is never saved. Codex starts in a folder it does not
//! trust (it ignores the folder's own `.codex` layer), so a run takes its turn with or without
//! trusting the project once.

use harness_business::model::{ApprovalMode, HarnessId, RoutineAction, RoutineModel, RunSession};

use super::agent_run::AgentTiming;
use super::tests_agent::{Fixture, timing};
use super::tests_agent_cli::{
    assert_run_ends_tagged, assert_run_takes_its_turn, assert_save_refused, assert_unanswered_ask_is_closed,
};

/// The thread every Codex scenario starts (`codex_engine_handshake.jsonl` and its siblings).
const THREAD: &str = "01a164a0-4c41-7613-9735-20de2a8016a8";

/// A Codex agent action that sends `prompt` with `approval_mode` and `model`, trusting the routines
/// folder once.
fn codex(prompt: &str, approval_mode: Option<ApprovalMode>, model: Option<RoutineModel>) -> RoutineAction {
    RoutineAction::Agent {
        harness: HarnessId::Codex,
        model,
        prompt: prompt.to_owned(),
        approval_mode,
        trust_once: true,
        max_minutes: None,
    }
}

/// The prompt of `codex_routine_pong.jsonl`, trusting the folder once when `trust_once`.
fn pong(trust_once: bool) -> RoutineAction {
    RoutineAction::Agent {
        harness: HarnessId::Codex,
        model: None,
        prompt: "Reply with exactly the word: pong".to_owned(),
        approval_mode: None,
        trust_once,
        max_minutes: None,
    }
}

/// Codex chats that play `scenario`; `None` (the test skips) when `fake_agent` is not built.
fn fixture(scenario: &str, timing: AgentTiming) -> Option<Fixture> {
    Fixture::of(HarnessId::Codex, scenario, timing)
}

/// Runs `prompt` in `approval_mode`, which Codex stalls on with an ask nobody answers, and checks
/// that Codex got the scenario's answer.
fn assert_unanswered_codex_ask_is_closed(scenario: &str, prompt: &str, approval_mode: Option<ApprovalMode>) {
    assert_unanswered_ask_is_closed(HarnessId::Codex, scenario, codex(prompt, approval_mode, None));
}

#[test]
fn an_unattended_codex_run_in_approval_mode_write_takes_its_turn_and_finishes() {
    let Some(fixture) = fixture("codex_routine_write.jsonl", timing()) else {
        return;
    };
    let model = RoutineModel {
        provider: "openai".to_owned(),
        id: "gpt-5.6-luna".to_owned(),
        thinking: None,
    };
    let session = RunSession {
        harness: HarnessId::Codex,
        id: THREAD.to_owned(),
    };

    // The scenario expects the thread to ask on request in a workspace-write sandbox, and the
    // routine's model on the turn.
    let action = codex("Check the build", Some(ApprovalMode::Write), Some(model));
    assert_run_ends_tagged(&fixture, action, &session, "Checked the build: all green.");
}

#[test]
fn a_codex_command_approval_nobody_gives_in_always_ask_is_declined_and_the_run_needs_attention() {
    assert_unanswered_codex_ask_is_closed(
        "codex_routine_ask.jsonl",
        "Run the tests",
        Some(ApprovalMode::AlwaysAsk),
    );
}

#[test]
fn a_codex_file_change_approval_nobody_gives_in_write_is_declined_and_the_run_needs_attention() {
    assert_unanswered_codex_ask_is_closed(
        "codex_routine_ask_write.jsonl",
        "Write the report",
        Some(ApprovalMode::Write),
    );
}

#[test]
fn a_codex_question_nobody_answers_gets_no_answers_and_the_run_needs_attention() {
    assert_unanswered_codex_ask_is_closed("codex_routine_question.jsonl", "Pick a colour", None);
}

#[test]
fn a_codex_routine_in_auto_is_refused() {
    let Some(fixture) = fixture("codex_routine_write.jsonl", timing()) else {
        return;
    };
    let action = codex("Check the build", Some(ApprovalMode::Auto), None);

    assert_save_refused(&fixture, action, "\"auto\" is for claude only");
}

#[test]
fn a_codex_run_in_a_folder_codex_does_not_trust_takes_its_turn() {
    let Some(fixture) = fixture("codex_routine_pong.jsonl", timing()) else {
        return;
    };

    assert_run_takes_its_turn(&fixture, pong(false), "pong");
}

#[test]
fn a_codex_run_that_trusts_the_project_once_takes_its_turn() {
    let Some(fixture) = fixture("codex_routine_pong.jsonl", timing()) else {
        return;
    };

    assert_run_takes_its_turn(&fixture, pong(true), "pong");
}
