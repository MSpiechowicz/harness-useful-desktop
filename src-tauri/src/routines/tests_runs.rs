//! What one run does: the run limits, command runs (exit codes, timeout, environment, output cap,
//! redaction), script runs against the `harness-memory` stand-in (`fake_agent`), and a run that is
//! no longer going on by the time the view hears it ended.

use std::sync::{Arc, Mutex};

use harness_business::model::{
    HookAuthor, HookOrigin, HookScope, RoutineAction, RoutineDraft, RoutineRunsQuery, RoutineWhen, RoutinesEvent,
    RunStatus, RunTrigger, ScriptScope,
};
use harness_business::routines::runs::started_run;
use jiff::Timestamp;
use tauri::async_runtime::block_on;
use tauri::ipc::{Channel, InvokeResponseBody};

use super::command_run;
use super::job::{RunJob, RunOutcome};
use super::queue::{Reserved, RunClass, Runs};
use super::script_run::TRUST_MESSAGE;
use super::tests::{command, draft, ended, save};
use crate::test_support::{ProjectHome, place_memory_server};

#[test]
fn at_most_four_command_and_two_agent_runs_go_at_once_and_the_rest_wait_in_order() {
    let mut runs = Runs::default();
    let mut reserve = |id: &str, class| runs.reserve(id, class, RunTrigger::Schedule);

    let shell: Vec<Reserved> = (0..5).map(|n| reserve(&format!("s{n}"), RunClass::Shell)).collect();
    assert_eq!(
        shell,
        [
            Reserved::Now,
            Reserved::Now,
            Reserved::Now,
            Reserved::Now,
            Reserved::Queued
        ]
    );

    let agent: Vec<Reserved> = (0..3).map(|n| reserve(&format!("a{n}"), RunClass::Agent)).collect();
    assert_eq!(agent, [Reserved::Now, Reserved::Now, Reserved::Queued]);

    assert_eq!(reserve("s0", RunClass::Shell), Reserved::Busy, "one run per routine");
    assert_eq!(reserve("s4", RunClass::Shell), Reserved::Busy, "waiting counts as busy");

    assert_eq!(runs.busy().len(), 8);
    assert_eq!(runs.running().len(), 6);
    assert!(runs.next_ready().is_empty(), "no place is free");

    runs.release("a1");
    assert_eq!(runs.next_ready(), [("a2".to_owned(), RunTrigger::Schedule)]);

    runs.release("s4");
    runs.release("s2");
    assert!(
        runs.next_ready().is_empty(),
        "a routine that left the queue never starts"
    );
    assert_eq!(runs.running().len(), 5);
}

#[test]
fn a_run_is_no_longer_going_on_when_the_view_hears_it_ended() {
    let setup = ProjectHome::new("routines-end-live");
    let runtime = Arc::downgrade(&setup.state.routines);
    let going_on = Arc::new(Mutex::new(Vec::new()));
    let seen = going_on.clone();

    // The view reads the runs going on the moment it hears a run ended.
    let listener: Channel<RoutinesEvent> = Channel::new(move |body| {
        let (Some(runtime), InvokeResponseBody::Json(json)) = (runtime.upgrade(), body) else {
            return Ok(());
        };

        if json.contains("\"runFinished\"") {
            let query = RoutineRunsQuery {
                routine_id: None,
                status: Some(RunStatus::Running),
                offset: 0,
                limit: 10,
            };
            seen.lock().unwrap().push(runtime.live_page(&query).total);
        }

        Ok(())
    });
    setup.state.routines.listen(listener);

    let view = save(
        &setup.state,
        &draft("Quick", RoutineWhen::AppStart, command("echo done")),
        HookAuthor::Desktop,
    );
    block_on(setup.state.routines.run_now(&view.id)).expect("ran");
    ended(&setup.state, &view.id, 1);

    assert_eq!(*going_on.lock().unwrap(), [0]);
}

/// A recorded run of a routine that runs `action` in the project folder.
fn job(setup: &ProjectHome, action: RoutineAction) -> RunJob {
    let view = save(
        &setup.state,
        &draft("Job", RoutineWhen::AppStart, action),
        HookAuthor::Desktop,
    );
    let routine = view.routine.expect("valid");
    let project = Some(setup.project.to_string_lossy().into_owned());
    let run = started_run(
        &routine,
        HookOrigin::Local,
        RunTrigger::Manual,
        project,
        Timestamp::now(),
    );

    RunJob {
        routine,
        run,
        folder: Some(setup.project.clone()),
    }
}

fn run_command(setup: &ProjectHome, text: &str, timeout_secs: Option<u32>) -> RunOutcome {
    let job = job(setup, command(text));

    block_on(command_run::run(&setup.state.env, &job, text, timeout_secs))
}

#[test]
fn a_command_that_exits_with_zero_is_ok_and_keeps_its_output() {
    let setup = ProjectHome::new("routine-command-ok");

    let outcome = run_command(&setup, "echo hello", None);

    assert_eq!(outcome.status, RunStatus::Ok);
    assert_eq!(outcome.exit_code, Some(0));
    assert_eq!(outcome.excerpt.as_deref(), Some("hello"));
    assert_eq!(outcome.error_excerpt, None);
    assert_eq!(outcome.message, None);
}

#[test]
fn a_non_zero_exit_fails_with_its_code() {
    let setup = ProjectHome::new("routine-command-exit");

    let outcome = run_command(&setup, "exit 3", None);

    assert_eq!(outcome.status, RunStatus::Failed);
    assert_eq!(outcome.exit_code, Some(3));
    assert_eq!(outcome.message.as_deref(), Some("The command exited with code 3"));
}

#[test]
fn secrets_in_the_output_are_redacted_before_they_are_kept() {
    let setup = ProjectHome::new("routine-command-redact");
    // Built at run time, so no credential-shaped literal is committed.
    let token = ["gh", "p_", &"Zx9Qw3Er7Ty1".repeat(3)].concat();

    let outcome = run_command(&setup, &format!("echo deploy {token}"), None);

    let excerpt = outcome.excerpt.expect("output");
    assert!(excerpt.starts_with("deploy"), "{excerpt}");
    assert!(!excerpt.contains(&token), "{excerpt}");
}

#[test]
fn a_command_without_a_folder_does_not_run() {
    let setup = ProjectHome::new("routine-command-no-folder");
    let job = RunJob {
        folder: None,
        ..job(&setup, command("echo never"))
    };

    let outcome = block_on(command_run::run(&setup.state.env, &job, "echo never", None));

    assert_eq!(outcome.status, RunStatus::Failed);
    assert_eq!(outcome.excerpt, None);
}

#[cfg(unix)]
#[test]
fn a_command_runs_in_its_folder_with_the_routine_variables_and_its_stderr_kept() {
    let setup = ProjectHome::new("routine-command-env");
    let job = job(&setup, command("env"));
    // The ids are random hex, which the redaction masks in output: they are compared in the shell.
    let text = format!(
        r#"test "$HARNESS_ROUTINE_ID" = {} && test "$HARNESS_ROUTINE_RUN" = {} && test "$HARNESS_ROUTINE_TRIGGER" = manual && test "$HARNESS_ROUTINE_PROJECT" = "$(pwd)" && echo vars-ok; pwd; echo oops >&2; exit 4"#,
        job.run.routine_id, job.run.record.id
    );

    let outcome = block_on(command_run::run(&setup.state.env, &job, &text, None));

    let expected = format!("vars-ok\n{}", setup.project.to_string_lossy());
    assert_eq!(outcome.excerpt.as_deref(), Some(expected.as_str()));
    assert_eq!(outcome.error_excerpt.as_deref(), Some("oops"));
    assert_eq!(outcome.status, RunStatus::Failed);
    assert_eq!(outcome.exit_code, Some(4));
}

#[cfg(unix)]
#[test]
fn a_command_past_its_timeout_is_killed_and_fails() {
    let setup = ProjectHome::new("routine-command-timeout");

    let outcome = run_command(&setup, "sleep 20", Some(1));

    assert_eq!(outcome.status, RunStatus::Failed);
    assert_eq!(outcome.message.as_deref(), Some("The command timed out"));
    assert_eq!(outcome.exit_code, None);
}

#[cfg(unix)]
#[test]
fn stdout_is_capped_and_its_real_tail_kept() {
    let setup = ProjectHome::new("routine-command-cap");

    // About 23 KiB of numbered lines: more than the 16 KiB kept.
    let outcome = run_command(&setup, "seq 1 5000", None);

    let excerpt = outcome.excerpt.expect("output");
    assert_eq!(outcome.status, RunStatus::Ok);
    assert!(excerpt.len() <= 4096, "{}", excerpt.len());
    assert!(excerpt.ends_with("\n4999\n5000"), "the end of the output is kept");
}

/// A script routine (`scope` the routine's, `script_scope` Harness Memory's) run with Run now.
fn run_script(setup: &ProjectHome, name: &str, scope: HookScope, script_scope: ScriptScope) -> RunOutcomeView {
    let action = RoutineAction::Script {
        name: name.to_owned(),
        scope: script_scope,
    };
    let project = (scope == HookScope::Project).then(|| setup.project.to_string_lossy().into_owned());
    let draft = RoutineDraft {
        scope,
        project,
        ..draft(name, RoutineWhen::AppStart, action)
    };
    let routine = save(&setup.state, &draft, HookAuthor::Desktop);

    block_on(setup.state.routines.run_now(&routine.id)).expect("ran");
    let run = ended(&setup.state, &routine.id, 1).remove(0).record;

    RunOutcomeView {
        status: run.status,
        message: run.message,
        excerpt: run.excerpt,
    }
}

/// What a script run's record says.
#[derive(Debug, PartialEq, Eq)]
struct RunOutcomeView {
    status: RunStatus,
    message: Option<String>,
    excerpt: Option<String>,
}

#[test]
fn a_script_routine_without_harness_memory_fails() {
    let setup = ProjectHome::new("routine-script-missing");

    let run = run_script(&setup, "release-notes", HookScope::Global, ScriptScope::Global);

    assert_eq!(run.status, RunStatus::Failed);
    assert_eq!(run.message.as_deref(), Some("Harness Memory is not available"));
}

#[test]
fn a_script_runs_unless_this_machine_must_trust_it_first() {
    let setup = ProjectHome::new("routine-script-trust");

    if place_memory_server(&setup.home).is_none() {
        return;
    }

    let untrusted = run_script(&setup, "release-notes", HookScope::Global, ScriptScope::Global);
    assert_eq!(
        untrusted,
        RunOutcomeView {
            status: RunStatus::NeedsAttention,
            message: Some(TRUST_MESSAGE.to_owned()),
            excerpt: None,
        },
        "an unreviewed script is not run"
    );

    let trusted = run_script(&setup, "open-prs", HookScope::Project, ScriptScope::Repo);
    assert_eq!(
        trusted,
        RunOutcomeView {
            status: RunStatus::Ok,
            message: None,
            excerpt: Some("ran gh".to_owned()),
        }
    );

    let missing = run_script(&setup, "no-such-script", HookScope::Global, ScriptScope::Global);
    assert_eq!(missing.status, RunStatus::Failed);
    assert_eq!(
        missing.message.as_deref(),
        Some("The script was not found in Harness Memory")
    );
}
