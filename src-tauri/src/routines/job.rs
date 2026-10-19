//! What a run carries out and how it ends: the [`RunJob`] a runner gets, the [`RunOutcome`] it
//! returns, the [`StopSignal`] that ends it early, and the [`AgentRunner`] seam agent routines run
//! through.

use std::path::PathBuf;

use harness_business::model::{Routine, RoutineRun, RunStatus};
use harness_business::process::BoxFuture;
use harness_business::routines::runs::{CLOSED_MESSAGE, RunEnd, excerpt_tail};
use harness_business::security::redact_text;
use jiff::Timestamp;
use tokio::sync::watch;

use super::progress::RunProgress;

/// The message of a run the user stopped.
pub const STOPPED_MESSAGE: &str = "Stopped in Harness Desktop";

/// The message of a team run stopped because its routine may no longer run here.
pub const WITHDRAWN_MESSAGE: &str = "The remote routine is off or no longer allowed here";

/// The message of an agent run while agent routines cannot run.
pub const AGENTS_UNAVAILABLE: &str = "Agent routines are not available in this version";

/// One run to carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunJob {
    /// The routine as it was when the run started.
    pub routine: Routine,
    /// The run as recorded at its start (`run.record.project` is the project root it runs in).
    pub run: RoutineRun,
    /// The folder it runs in: the project root (checked again at the start), `<root>/workspace`
    /// for a global agent or command routine, `None` for a global script routine.
    pub folder: Option<PathBuf>,
}

/// How a run ended, before its end time is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutcome {
    /// `ok`, `failed`, `needsAttention` or `stopped`.
    pub status: RunStatus,
    /// A short secret-free reason.
    pub message: Option<String>,
    /// The last assistant text, or the stdout tail (redacted).
    pub excerpt: Option<String>,
    /// The stderr tail (redacted).
    pub error_excerpt: Option<String>,
    /// The exit code of a command or script.
    pub exit_code: Option<i32>,
}

impl RunOutcome {
    fn with(status: RunStatus, message: Option<&str>) -> Self {
        Self {
            status,
            message: message.map(str::to_owned),
            excerpt: None,
            error_excerpt: None,
            exit_code: None,
        }
    }

    /// It went well.
    pub fn ok() -> Self {
        Self::with(RunStatus::Ok, None)
    }

    /// It failed because of `message`.
    pub fn failed(message: &str) -> Self {
        Self::with(RunStatus::Failed, Some(message))
    }

    /// It cannot go on without the user, because of `message`.
    pub fn needs_attention(message: &str) -> Self {
        Self::with(RunStatus::NeedsAttention, Some(message))
    }

    /// It was stopped for `reason`.
    pub fn stopped(reason: StopReason) -> Self {
        Self::with(RunStatus::Stopped, Some(reason.message()))
    }

    /// The end of a process `what` ran ("The command", "The script"): `ok` for exit code 0, else
    /// `failed` with why, the redacted tails of its output kept.
    pub fn exited(what: &str, exit_code: Option<i32>, timed_out: bool, stdout: &str, stderr: &str) -> Self {
        let mut outcome = match (timed_out, exit_code) {
            (true, _) => Self::failed(&format!("{what} timed out")),
            (false, Some(0)) => Self::ok(),
            (false, Some(code)) => Self::failed(&format!("{what} exited with code {code}")),
            (false, None) => Self::failed(&format!("{what} was stopped before it finished")),
        };

        outcome.exit_code = exit_code;
        outcome.excerpt = excerpt(stdout);
        outcome.error_excerpt = excerpt(stderr);
        outcome
    }

    /// The record's end at `ended_at`.
    pub fn end(self, ended_at: Timestamp) -> RunEnd {
        RunEnd {
            status: self.status,
            ended_at,
            message: self.message,
            excerpt: self.excerpt,
            error_excerpt: self.error_excerpt,
            exit_code: self.exit_code,
        }
    }
}

/// The tail of `output` a record keeps: redacted for secrets, trailing whitespace dropped, at
/// most 4 KiB; `None` when blank.
pub fn excerpt(output: &str) -> Option<String> {
    let redacted = redact_text(output.trim_end());

    (!redacted.trim().is_empty()).then(|| excerpt_tail(&redacted))
}

/// Why a run was stopped before it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// The user stopped it (or deleted its routine).
    User,
    /// Harness Desktop is closing.
    Closed,
    /// A team routine was turned off here, left the team, or is no longer trusted or approved.
    Withdrawn,
}

impl StopReason {
    /// The record's message.
    pub fn message(self) -> &'static str {
        match self {
            Self::User => STOPPED_MESSAGE,
            Self::Closed => CLOSED_MESSAGE,
            Self::Withdrawn => WITHDRAWN_MESSAGE,
        }
    }
}

/// Tells a running job to stop.
pub struct StopSignal(pub(super) watch::Receiver<Option<StopReason>>);

impl StopSignal {
    /// Why the run is to stop, when it is already.
    pub fn fired(&self) -> Option<StopReason> {
        *self.0.borrow()
    }

    /// Waits until the run is to stop and says why. Never returns while it may go on.
    pub async fn stopped(&mut self) -> StopReason {
        let reason = self.0.wait_for(Option::is_some).await.map(|reason| *reason);

        match reason {
            Ok(reason) => reason.unwrap_or(StopReason::User),
            // The runner is gone without a stop: nothing will ask any more.
            Err(_) => std::future::pending().await,
        }
    }
}

/// Carries out agent routines (P6: the chat, its relay and the "needs you" timer). The runner
/// calls [`AgentRunner::run`] once per run with the folder already authorized, records what it
/// returns, and sends the notifications. The implementation:
///
/// - reports the harness session as soon as it is known ([`RunProgress::session`]) and every
///   change of "waits for an answer" ([`RunProgress::needs_you`]);
/// - returns soon after `stop` fires (with [`RunOutcome::stopped`] and the reason), stopping its
///   agent unless a window is attached to it;
/// - enforces `maxMinutes`, the model and the session-limit retries itself.
pub trait AgentRunner: Send + Sync {
    /// Runs the agent action of `job.routine`.
    fn run(&self, job: RunJob, progress: RunProgress, stop: StopSignal) -> BoxFuture<'static, RunOutcome>;
}

/// The agent runner until agent routines are wired: every run fails with [`AGENTS_UNAVAILABLE`].
pub struct AgentsUnavailable;

impl AgentRunner for AgentsUnavailable {
    fn run(&self, _job: RunJob, _progress: RunProgress, _stop: StopSignal) -> BoxFuture<'static, RunOutcome> {
        Box::pin(async { RunOutcome::failed(AGENTS_UNAVAILABLE) })
    }
}
