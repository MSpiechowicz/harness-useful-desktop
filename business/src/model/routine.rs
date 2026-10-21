//! Routines: small JSON files that say when something runs (a daily time, every N minutes, a cron
//! expression, app start) and what runs (an agent chat with a prompt, a shell command or a saved
//! Harness Memory script), and the team routines shared through Harness Memory.
//!
//! The file format is the contract in `docs/routines-format.md`; [`crate::routines`] reads,
//! validates and writes the files and plans when they run. Scopes, authors, approvals and origins
//! are the ones hooks use ([`HookScope`], [`HookAuthor`], [`HookApproval`], [`HookOrigin`]). These
//! are the shapes the webview sees.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ApprovalMode, HarnessId, HookApproval, HookAuthor, HookOrigin, HookScope, ScriptScope, ShareBlock};

/// When a routine runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum RoutineWhen {
    /// Every day at `at` (local time).
    Daily {
        /// `HH:MM` on a 24-hour clock.
        at: String,
    },
    /// Monday to Friday at `at` (local time).
    Weekdays {
        /// `HH:MM` on a 24-hour clock.
        at: String,
    },
    /// Every `every` minutes, counted from the start of each local day.
    EveryMinutes {
        /// 1 to 720.
        every: u32,
    },
    /// Every `every` hours, counted from the start of each local day.
    EveryHours {
        /// 1 to 24.
        every: u32,
    },
    /// At every local minute a five-field cron expression matches.
    Cron {
        /// At most 100 characters.
        expression: String,
    },
    /// Once each time Harness Desktop starts.
    AppStart,
}

impl RoutineWhen {
    /// Which kind of schedule this is (the "When" filter).
    pub fn kind(&self) -> RoutineWhenKind {
        match self {
            Self::Daily { .. } => RoutineWhenKind::Daily,
            Self::Weekdays { .. } => RoutineWhenKind::Weekdays,
            Self::EveryMinutes { .. } => RoutineWhenKind::EveryMinutes,
            Self::EveryHours { .. } => RoutineWhenKind::EveryHours,
            Self::Cron { .. } => RoutineWhenKind::Cron,
            Self::AppStart => RoutineWhenKind::AppStart,
        }
    }
}

/// The kinds of [`RoutineWhen`], without their fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RoutineWhenKind {
    /// `daily`.
    Daily,
    /// `weekdays`.
    Weekdays,
    /// `everyMinutes`.
    EveryMinutes,
    /// `everyHours`.
    EveryHours,
    /// `cron`.
    Cron,
    /// `appStart`.
    AppStart,
}

impl RoutineWhenKind {
    /// The `when.type` in a file.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekdays => "weekdays",
            Self::EveryMinutes => "everyMinutes",
            Self::EveryHours => "everyHours",
            Self::Cron => "cron",
            Self::AppStart => "appStart",
        }
    }
}

/// The model an agent routine switches to (`ModelOption.provider` and `ModelOption.id`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutineModel {
    /// 1 to 200 characters.
    pub provider: String,
    /// 1 to 200 characters.
    pub id: String,
}

/// What a routine does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum RoutineAction {
    /// Starts a chat with an agent and sends `prompt`.
    Agent {
        /// A harness with chat: `omp`, `pi` or `opencode`.
        harness: HarnessId,
        /// The model to switch to; `None` keeps the harness's default.
        model: Option<RoutineModel>,
        /// 1 to 8192 characters, sent as written.
        prompt: String,
        /// OMP and OpenCode only: the chat's approval mode; `None` keeps the harness's setting.
        approval_mode: Option<ApprovalMode>,
        /// PI only: trust the project's local resources for this launch.
        trust_once: bool,
        /// 1 to 240; `None` means 30. The run is stopped after that long.
        max_minutes: Option<u32>,
    },
    /// Runs `command` in a shell.
    Command {
        /// 1 to 2048 characters.
        command: String,
        /// 1 to 3600 seconds; `None` means 300.
        timeout_secs: Option<u32>,
    },
    /// Runs a script saved in Harness Memory.
    Script {
        /// The script name (`^[a-z0-9][a-z0-9_-]{0,63}$`).
        name: String,
        /// Harness Memory's script scope (`repo` only in a project routine).
        scope: ScriptScope,
    },
}

impl RoutineAction {
    /// True for an agent action that runs tools without asking: OMP or OpenCode
    /// `approvalMode: "yolo"`, or PI `trustOnce: true`.
    pub fn runs_unattended(&self) -> bool {
        match self {
            Self::Agent {
                approval_mode,
                trust_once,
                ..
            } => *approval_mode == Some(ApprovalMode::Yolo) || *trust_once,
            Self::Command { .. } | Self::Script { .. } => false,
        }
    }
}

/// A valid routine file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Routine {
    /// 32 lowercase hex characters, the file name stem.
    pub id: String,
    /// 1 to 80 characters.
    pub name: String,
    /// At most 500 characters.
    pub description: Option<String>,
    /// Global or one project.
    pub scope: HookScope,
    /// The canonical project root of a project routine (a `git:` repository key in a team body).
    pub project: Option<String>,
    /// When it runs.
    pub when: RoutineWhen,
    /// What it does.
    pub action: RoutineAction,
    /// Whether the schedule is on.
    pub enabled: bool,
    /// RFC 3339 UTC.
    pub created_at: String,
    /// Who created the file (decides who may remove it).
    pub created_by: HookAuthor,
    /// RFC 3339 UTC.
    pub updated_at: String,
    /// The last writer.
    pub updated_by: HookAuthor,
}

/// Whether a routine runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RoutineStatus {
    /// Valid, approved and on: it runs on its schedule.
    Active,
    /// A run of it is going on now.
    Running,
    /// Valid and approved, but its schedule is off (or a team routine not turned on here).
    Off,
    /// Valid, but this exact file was never approved (an agent wrote or changed it), or a team
    /// routine that needs approval on this machine.
    Pending,
    /// A project routine whose folder is missing or no longer that project (a team one: its chosen
    /// folder is missing or no longer the repository). It does not run, not even with "Run now",
    /// until the folder is back; `RoutineView.problem` says why for a local one.
    Paused,
    /// The file breaks the format (see `RoutineView.problem`); it never runs.
    Invalid,
    /// A team routine whose id a local routine also has: the local one wins.
    Duplicate,
}

/// One routine file, or one team routine (`origin: team`), as the Routines view lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutineView {
    /// The file name stem.
    pub id: String,
    /// The parsed routine; `None` when the file is invalid.
    pub routine: Option<Routine>,
    /// A readable name even for an invalid file (its `name` when that is a string).
    pub name: Option<String>,
    /// Whether it runs.
    pub status: RoutineStatus,
    /// Lowercase SHA-256 of the file bytes; pass it back to approve, edit or delete this version.
    pub sha256: String,
    /// Why the file is invalid, or why a paused local routine does not run.
    pub problem: Option<String>,
    /// Whether this or an earlier version was approved.
    pub approval: HookApproval,
    /// The approved earlier version, for a diff: set when `approval` is `changed` and the app kept
    /// a copy of that version.
    pub approved: Option<Routine>,
    /// A local file or a team routine.
    pub origin: HookOrigin,
    /// The agent action runs tools without asking (`approvalMode: "yolo"` or `trustOnce: true`).
    pub runs_unattended: bool,
    /// "Run now" may start it: valid and approved (a team routine: turned on here, trusted or
    /// approved, and its folder there).
    pub can_run_now: bool,
    /// A run of it waits for a free place (at most 2 agent and 4 command or script runs at once):
    /// "Run now" is refused until it started.
    pub queued: bool,
    /// The next scheduled run (RFC 3339 UTC) while it is active; `None` for app start routines.
    pub next_run: Option<String>,
    /// Team routines: who shared it (a free-text label).
    pub team_author: Option<String>,
    /// Team routines: when the team copy last changed (RFC 3339 UTC).
    pub team_updated_at: Option<String>,
    /// Team routines: the repository key of a project routine, `None` for a global one.
    pub team_repo: Option<String>,
    /// Team routines: the local routine with the same id changed since it was shared.
    pub team_changed: bool,
    /// Team routines: turned on on this machine.
    pub team_on: bool,
    /// Team project routines: the folder it runs in on this machine, chosen when turned on.
    pub team_folder: Option<String>,
    /// Local routines: why "Share with team" is not possible, `None` when it is.
    pub share_block: Option<ShareBlock>,
    /// The project root of a project routine: a local file's (also over its scope's limit), a
    /// team routine's only once a refresh of this session confirmed a root of its repository.
    /// `None` for a global routine or an unknown scope.
    pub root: Option<String>,
    /// The repository key of a project routine: a team routine's, or the one a refresh of this
    /// session confirmed for a local file's root. `None` for a global routine or while unknown.
    pub repo_key: Option<String>,
}

/// The team routines as this machine sees them, while sharing is on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeamRoutines {
    /// "Run team routines without asking" is on.
    pub trust: bool,
    /// Why the latest refresh failed (the copies of the one before are used).
    pub problem: Option<String>,
    /// When the copies were last refreshed (RFC 3339 UTC).
    pub refreshed_at: Option<String>,
}

/// How many listed routines fall in each filter value, before filtering.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutineCounts {
    /// Every listed routine.
    pub total: u32,
    /// By status ("Status" filter).
    pub status: BTreeMap<RoutineStatus, u32>,
    /// By kind of schedule ("When" filter); invalid files have none.
    pub when: BTreeMap<RoutineWhenKind, u32>,
    /// Local or team ("Source" filter).
    pub origin: BTreeMap<HookOrigin, u32>,
}

/// Which routines a listing keeps; `None` keeps all.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutineFilter {
    /// The kind of schedule.
    pub when: Option<RoutineWhenKind>,
    /// The status.
    pub status: Option<RoutineStatus>,
    /// Local or team.
    pub origin: Option<HookOrigin>,
}

/// The routines folder (and the team routines) as the Routines view shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutineListing {
    /// Local routines (valid ones global first, then `createdAt`, then `id`; invalid ones after
    /// them by id), then the team routines the same way.
    pub routines: Vec<RoutineView>,
    /// Why the folder is not read; `routines` then holds no local routine.
    pub folder_problem: Option<String>,
    /// The team routines' state; `None` while sharing is off or unavailable.
    pub team: Option<TeamRoutines>,
    /// The counts of each filter value over every listed routine.
    pub counts: RoutineCounts,
}

/// What the Routines view sends to create or edit a routine (`routine_save`). The desktop keeps
/// the file's `createdAt`, `createdBy` and unknown fields on an edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutineDraft {
    /// The routine to edit; `None` creates one.
    pub id: Option<String>,
    /// For an edit: `RoutineView.sha256` of the version the user edited.
    pub expected_sha256: Option<String>,
    /// 1 to 80 characters.
    pub name: String,
    /// At most 500 characters; empty means none.
    pub description: Option<String>,
    /// Global or one project.
    pub scope: HookScope,
    /// For a project routine: a folder of the project (its project root is stored).
    pub project: Option<String>,
    /// When it runs.
    pub when: RoutineWhen,
    /// What it does.
    pub action: RoutineAction,
    /// Whether the schedule is on.
    pub enabled: bool,
}

/// What `routines_delete_all` removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoutinesDeleted {
    /// How many routine files were deleted.
    pub deleted: u32,
}

/// What `routines_team_remove_mine` removed from the team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeamRoutinesRemoved {
    /// How many team routines the team database deleted.
    pub deleted: u32,
    /// Their ids.
    pub ids: Vec<String>,
}

/// The next runs of a schedule (`routine_schedule_preview`), or why it is not valid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SchedulePreview {
    /// The next slots (RFC 3339 UTC), earliest first; empty for app start or a problem.
    pub next: Vec<String>,
    /// Why the schedule is not valid.
    pub problem: Option<String>,
}

/// The routine a harness session was started for (the "Routine" tag in Sessions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionRoutineTag {
    /// The routine id.
    pub routine_id: String,
    /// The run that started the session.
    pub run_id: String,
    /// The routine's name when the run started.
    pub name: String,
}
