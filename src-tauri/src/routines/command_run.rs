//! Command routines (see "Command runs" in `docs/routines-format.md`): the shell of hook commands
//! ([`shell_exec_spec`]) in the project root or `<root>/workspace`, stdin closed, the
//! `HARNESS_ROUTINE_*` variables set, killed at its timeout (with its process group, also when the
//! run is stopped). Exit code 0 is `ok`; a non-zero code, a timeout or a spawn failure is
//! `failed`. Of each output stream the last [`MAX_OUTPUT_BYTES`] are kept (without a word cut off
//! at their start), redacted. Neither the command nor its output is logged.

use std::time::Duration;

use harness_business::env::Env;
use harness_business::hooks::command::shell_exec_spec;
use harness_business::process::{ProcessRunner, TokioRunner};

use super::job::{RunJob, RunOutcome};

/// Output kept from each stream (its end), in bytes.
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024;

/// The timeout when the routine sets none.
pub const DEFAULT_TIMEOUT_SECS: u32 = 300;

/// The environment of a command run of `job`: the routine id, the run id, the trigger and the
/// project root (empty for a global routine).
pub fn routine_vars(job: &RunJob) -> Vec<(String, String)> {
    let record = &job.run.record;

    [
        ("HARNESS_ROUTINE_ID", job.run.routine_id.clone()),
        ("HARNESS_ROUTINE_RUN", record.id.clone()),
        ("HARNESS_ROUTINE_TRIGGER", record.trigger.as_str().to_owned()),
        ("HARNESS_ROUTINE_PROJECT", record.project.clone().unwrap_or_default()),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value))
    .collect()
}

/// Runs `command` of `job` with `timeout_secs` (`None`: [`DEFAULT_TIMEOUT_SECS`]).
pub async fn run(env: &Env, job: &RunJob, command: &str, timeout_secs: Option<u32>) -> RunOutcome {
    let Some(cwd) = job.folder.as_deref() else {
        return RunOutcome::failed("The command has no folder to run in");
    };

    let timeout = Duration::from_secs(u64::from(timeout_secs.unwrap_or(DEFAULT_TIMEOUT_SECS)));
    let spec = shell_exec_spec(env, command, cwd, &routine_vars(job), timeout, MAX_OUTPUT_BYTES).keep_output_tail();

    match TokioRunner.run(&spec).await {
        Ok(output) => RunOutcome::exited(
            "The command",
            output.status,
            output.timed_out,
            &output.stdout,
            &output.stderr,
        ),
        Err(error) => {
            tracing::warn!(run = job.run.record.id, code = ?error.code(), "routine command could not start");
            RunOutcome::failed("The command could not start")
        }
    }
}
