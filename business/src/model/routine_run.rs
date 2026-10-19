//! Runs of routines: one record per execution, kept per routine by the app window (see "Run
//! records" in `docs/routines-format.md`, [`crate::routines::runs`]), and the events the Routines
//! view listens to.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{HarnessId, HookOrigin};

/// What started a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RunTrigger {
    /// A slot, on time.
    Schedule,
    /// A missed slot, run once later.
    CatchUp,
    /// `when.type: "appStart"` at launch.
    AppStart,
    /// "Run now".
    Manual,
}

impl RunTrigger {
    /// The name in records and in `HARNESS_ROUTINE_TRIGGER`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Schedule => "schedule",
            Self::CatchUp => "catchUp",
            Self::AppStart => "appStart",
            Self::Manual => "manual",
        }
    }
}

/// How a run went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RunStatus {
    /// Started and not finished (a run waiting for an answer included).
    Running,
    /// The agent settled, or the command or script exited with 0.
    Ok,
    /// It failed (see `message`).
    Failed,
    /// It cannot go on without the user (an unreviewed script, an unanswered question).
    NeedsAttention,
    /// The user stopped it, or Harness Desktop closed during the run.
    Stopped,
}

/// The harness session of an agent run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunSession {
    /// `omp` or `pi`.
    pub harness: HarnessId,
    /// The harness's session id.
    pub id: String,
}

/// One run as its routine's record file keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunRecord {
    /// 32 lowercase hex characters.
    pub id: String,
    /// The routine's name when the run started.
    pub routine_name: String,
    /// A local or a team routine.
    pub origin: HookOrigin,
    /// What started it.
    pub trigger: RunTrigger,
    /// RFC 3339 UTC.
    pub started_at: String,
    /// RFC 3339 UTC, `None` while running.
    pub ended_at: Option<String>,
    /// From start to end, `None` while running.
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    /// How it went.
    pub status: RunStatus,
    /// A short secret-free reason, at most 500 characters.
    pub message: Option<String>,
    /// The last assistant text of an agent run, or the stdout tail of a command or script
    /// (redacted, at most 4 KiB).
    pub excerpt: Option<String>,
    /// The stderr tail of a command or script (redacted, at most 4 KiB).
    pub error_excerpt: Option<String>,
    /// The exit code of a command or script.
    pub exit_code: Option<i32>,
    /// The harness session of an agent run.
    pub session: Option<RunSession>,
    /// The project root the run used, `None` for a global run.
    pub project: Option<String>,
}

/// One run as the Routines view shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutineRun {
    /// The routine it ran.
    pub routine_id: String,
    /// A running agent run waits for an answer ("Needs you"); never stored.
    pub needs_you: bool,
    /// The stored record.
    #[serde(flatten)]
    pub record: RunRecord,
}

/// What `routine_runs` asks for: the newest runs first, optionally of one routine or status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutineRunsQuery {
    /// Only this routine's runs.
    pub routine_id: Option<String>,
    /// Only runs with this status.
    pub status: Option<RunStatus>,
    /// How many matching runs to skip.
    pub offset: u32,
    /// At most this many runs (1 to 200).
    pub limit: u32,
}

/// One page of runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutineRunsPage {
    /// The page, newest first.
    pub runs: Vec<RoutineRun>,
    /// How many runs match in all.
    pub total: u32,
    /// The offset of the next page, `None` on the last one.
    pub next_offset: Option<u32>,
}

/// What `routines_listen` sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum RoutinesEvent {
    /// The routines folder, the approvals or the team routines changed: read the list again.
    Changed,
    /// A run started.
    RunStarted {
        /// The run as recorded.
        run: RoutineRun,
    },
    /// A run ended.
    RunFinished {
        /// The run as recorded.
        run: RoutineRun,
    },
    /// A run waits for an answer and no window is attached to it.
    NeedsYou {
        /// The run.
        run_id: String,
        /// Its routine.
        routine_id: String,
        /// The routine's name.
        name: String,
    },
    /// A run no longer waits for an answer (it was answered, the question was withdrawn, or the
    /// run ended): a "needs you" prompt about it can go.
    NeedsYouCleared {
        /// The run.
        run_id: String,
        /// Its routine.
        routine_id: String,
    },
    /// Open the chat of this run (the user clicked a "needs you" notification).
    Open {
        /// The run.
        run_id: String,
    },
}
