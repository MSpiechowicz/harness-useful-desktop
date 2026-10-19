//! Script routines (see "Script runs" in `docs/routines-format.md`): a saved Harness Memory script
//! run by name over its own `harness-memory mcp` connection
//! ([`MemoryScripts::run_for_routine`]). A script this machine must approve first is not run: the
//! run needs attention ("Trust it in the Scripts view"). A missing script or Harness Memory is
//! `failed`; otherwise the script's exit decides, as for a command. A stop cancels the script
//! gracefully: Harness Memory kills it with its process group before its server is shut down.

use std::path::Path;

use harness_business::ecosystem::MemoryScripts;
use harness_business::ecosystem::memory_scripts_run::{ScriptCall, ScriptRunResult};
use harness_business::env::Env;
use harness_business::model::ScriptScope;

use super::job::{RunJob, RunOutcome, StopReason, StopSignal};

/// The message of a script run that needs the user's approval of the script.
pub const TRUST_MESSAGE: &str = "Trust it in the Scripts view";

/// Runs script `name` in `scope` for `job` (in its project when it has one), until `stop` fires.
pub async fn run(
    scripts: &MemoryScripts,
    env: &Env,
    job: &RunJob,
    name: &str,
    scope: ScriptScope,
    mut stop: StopSignal,
) -> RunOutcome {
    let call = ScriptCall {
        name,
        scope,
        project: job.run.record.project.as_deref().map(Path::new),
    };

    let mut reason = None;
    let result = scripts
        .run_for_routine(env, &call, async {
            reason = Some(stop.stopped().await);
        })
        .await;

    match (result, reason) {
        (ScriptRunResult::Stopped, Some(reason)) => RunOutcome::stopped(reason),
        (result, _) => outcome_of(result),
    }
}

/// The run's outcome for what Harness Memory answered.
pub fn outcome_of(result: ScriptRunResult) -> RunOutcome {
    match result {
        ScriptRunResult::Unavailable => RunOutcome::failed("Harness Memory is not available"),
        ScriptRunResult::NotFound => RunOutcome::failed("The script was not found in Harness Memory"),
        ScriptRunResult::NeedsTrust => RunOutcome::needs_attention(TRUST_MESSAGE),
        ScriptRunResult::Failed(reason) => RunOutcome::failed(&reason),
        ScriptRunResult::TimedOut => RunOutcome::failed("The script did not finish in time"),
        ScriptRunResult::Stopped => RunOutcome::stopped(StopReason::User),
        ScriptRunResult::Finished {
            exit_code,
            timed_out,
            stdout,
            stderr,
        } => RunOutcome::exited("The script", exit_code, timed_out, &stdout, &stderr),
    }
}
