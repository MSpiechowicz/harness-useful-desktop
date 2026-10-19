//! Runs the command hooks of one firing (see "Commands" in the format doc).
//!
//! - Shell: `sh -c <command>` on Linux and macOS, `cmd.exe /D /S /C "<command>"` on
//!   Windows. Stdin is closed, the working directory and the `HARNESS_HOOK_*` variables come from
//!   the [`CommandRequest`].
//! - The commands of a firing run one after another and share [`FIRING_BUDGET`]; each keeps its
//!   own timeout, cut short to the budget left, and the ones left when it is spent are skipped.
//! - Stdout is capped at [`MAX_OUTPUT_BYTES`], redacted for secrets and trimmed; stderr is
//!   dropped. A non-zero exit, a timeout (the process group is killed) or a spawn failure gives no
//!   output. Neither the command nor its output is logged.
//!
//! This is the only module that names a shell (`scripts/check-forbidden.ts`): anything else that
//! runs a command text the user wrote or approved builds it with [`shell_exec_spec`].

use std::path::Path;
use std::time::{Duration, Instant};

use super::compose::ResolvedPart;
use super::plan::{CommandRequest, Firing, PlannedPart};
use crate::env::Env;
use crate::process::{ExecSpec, ProcessRunner};
use crate::security::redact_text;

/// Total time the commands of one firing may take.
pub const FIRING_BUDGET: Duration = Duration::from_secs(30);

/// Stdout kept from one command, in bytes.
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024;

/// Why a command hook gave no output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandFailureKind {
    /// The shell could not be started.
    Spawn,
    /// It exited with a non-zero code (`None`: killed by a signal).
    Exit(Option<i32>),
    /// It ran past its timeout and was killed.
    Timeout,
    /// The firing's budget was spent before it could run.
    Skipped,
}

impl CommandFailureKind {
    /// A short stable name, for logs and notices.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spawn => "spawn",
            Self::Exit(_) => "exit",
            Self::Timeout => "timeout",
            Self::Skipped => "skipped",
        }
    }
}

/// A command hook that gave no output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandFailure {
    /// The hook's id.
    pub hook_id: String,
    /// The hook's name.
    pub hook_name: String,
    /// What went wrong.
    pub kind: CommandFailureKind,
}

/// A carried-out firing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FiringResult {
    /// What each hook contributes, in run order: the fixed texts, and the output of each command
    /// that succeeded with non-empty output.
    pub parts: Vec<ResolvedPart>,
    /// The commands that gave no output because they failed or were skipped.
    pub failures: Vec<CommandFailure>,
}

/// Carries out `firing`: fixed texts pass through, commands run one after another within
/// [`FIRING_BUDGET`].
pub async fn run_firing(runner: &dyn ProcessRunner, env: &Env, firing: &Firing) -> FiringResult {
    let started = Instant::now();
    let mut result = FiringResult::default();

    for part in &firing.parts {
        let request = match part {
            PlannedPart::Text(text) => {
                result.parts.push(text.clone());
                continue;
            }
            PlannedPart::Command(request) => request,
        };

        let left = FIRING_BUDGET.saturating_sub(started.elapsed());

        let outcome = if left.is_zero() {
            tracing::warn!(hook = request.hook_id, "hook command skipped: the 30 s budget is spent");
            Err(CommandFailureKind::Skipped)
        } else {
            run_command(runner, env, request, request.timeout.min(left)).await
        };

        match outcome {
            Ok(Some(text)) => result.parts.push(ResolvedPart {
                hook_id: request.hook_id.clone(),
                hook_name: request.hook_name.clone(),
                text,
                order: request.order.clone(),
            }),
            Ok(None) => {}
            Err(kind) => result.failures.push(CommandFailure {
                hook_id: request.hook_id.clone(),
                hook_name: request.hook_name.clone(),
                kind,
            }),
        }
    }

    result
}

/// Carries out the firings of a [`super::SendPlan`] (`sessionStart`, then `promptSubmit`), each
/// with its own budget, and merges their parts in run order for [`super::compose_prompt`].
pub async fn run_firings(runner: &dyn ProcessRunner, env: &Env, firings: &[Firing]) -> FiringResult {
    let mut merged = FiringResult::default();

    for firing in firings {
        let result = run_firing(runner, env, firing).await;
        merged.parts.extend(result.parts);
        merged.failures.extend(result.failures);
    }

    merged.parts.sort_by(|left, right| left.order.cmp(&right.order));
    merged
}

/// Runs one command with `timeout`. `Ok(None)` for a success with blank output.
pub async fn run_command(
    runner: &dyn ProcessRunner,
    env: &Env,
    request: &CommandRequest,
    timeout: Duration,
) -> Result<Option<String>, CommandFailureKind> {
    let spec = command_spec(env, request, timeout);

    let output = match runner.run(&spec).await {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!(hook = request.hook_id, %error, "hook command could not start");
            return Err(CommandFailureKind::Spawn);
        }
    };

    if output.timed_out {
        tracing::warn!(hook = request.hook_id, "hook command timed out");
        return Err(CommandFailureKind::Timeout);
    }

    if output.status != Some(0) {
        tracing::info!(hook = request.hook_id, code = ?output.status, "hook command failed");
        return Err(CommandFailureKind::Exit(output.status));
    }

    let text = redact_text(output.stdout.trim_end());

    Ok((!text.trim().is_empty()).then_some(text))
}

/// The shell invocation of `request`.
pub fn command_spec(env: &Env, request: &CommandRequest, timeout: Duration) -> ExecSpec {
    shell_exec_spec(
        env,
        &request.command,
        &request.cwd,
        &request.env,
        timeout,
        MAX_OUTPUT_BYTES,
    )
}

/// The shell invocation of the command text `command` (`sh -c` on Linux and macOS,
/// `cmd.exe /D /S /C` on Windows): run in `cwd` with `vars` added to its environment and stdin
/// closed, killed after `timeout`, each output stream capped at `max_output_bytes`. Only for a
/// command the user wrote or approved.
pub fn shell_exec_spec(
    env: &Env,
    command: &str,
    cwd: &Path,
    vars: &[(String, String)],
    timeout: Duration,
    max_output_bytes: usize,
) -> ExecSpec {
    shell_spec(env, command)
        .cwd(cwd)
        .envs(vars.iter().map(|(name, value)| (name.clone(), value.clone())))
        .timeout(timeout)
        .max_output_bytes(max_output_bytes)
}

#[cfg(not(windows))]
fn shell_spec(_env: &Env, command: &str) -> ExecSpec {
    ExecSpec::new("/bin/sh").args(["-c", command]) // guard-allow: installer-shell
}

/// `cmd.exe /D /S /C "<command>"`: with `/S` cmd strips the outer quotes and runs the rest as
/// typed, so the command is passed unquoted ([`ExecSpec::raw_arg`]).
#[cfg(windows)]
fn shell_spec(env: &Env, command: &str) -> ExecSpec {
    let root = env.var("SystemRoot").unwrap_or("C:\\Windows");
    let cmd = std::path::Path::new(root).join("System32").join("cmd.exe"); // guard-allow: installer-shell

    ExecSpec::new(cmd)
        .args(["/D", "/S", "/C"])
        .raw_arg(format!("\"{command}\""))
}
