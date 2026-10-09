//! Typed arguments of the Harness Dashboard MCP tools, mirroring the input schemas in the
//! dashboard's `src/mcp/tools.ts` (every schema has `additionalProperties: false`).
//!
//! Field names are the dashboard's own (snake_case, e.g. `session_id`), not camelCase: these
//! objects are passed through as the `arguments` of an MCP `tools/call`. Text limits are counted
//! in UTF-16 code units, as the dashboard's validator counts them.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Largest `from`/`to` text.
pub const MAX_DASHBOARD_TIME_CHARS: usize = 40;

/// Largest `provider` or `tag` text.
pub const MAX_DASHBOARD_NAME_CHARS: usize = 100;

/// Largest `model`, `user` or `tool` text.
pub const MAX_DASHBOARD_ID_CHARS: usize = 200;

/// Largest `session_id` or `branch` text.
pub const MAX_DASHBOARD_REF_CHARS: usize = 300;

/// Largest `project` or `cwd` text (a path or a folder name).
pub const MAX_DASHBOARD_PATH_CHARS: usize = 1000;

/// Largest `limit` of `breakdown` (rows) and `failures` (recent failures).
pub const MAX_DASHBOARD_ROWS: u32 = 50;

/// Largest `limit` of `tips`.
pub const MAX_DASHBOARD_TIPS: u32 = 30;

/// Time range of a report, in the dashboard's local time (`RANGES` in `src/core/reports.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DashboardRange {
    /// Since local midnight.
    Today,
    /// The last 7 whole days, today included.
    #[serde(rename = "7d")]
    SevenDays,
    /// The last 30 whole days, today included.
    #[serde(rename = "30d")]
    ThirtyDays,
    /// This calendar month.
    Month,
    /// Everything recorded.
    All,
    /// From `from` to `to` (at least one of them is required).
    Custom,
}

/// What a `breakdown` groups by (`DIMENSIONS` in `src/mcp/tools.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DashboardDimension {
    /// Project folder.
    Project,
    /// Model id.
    Model,
    /// Harness (e.g. `omp`, `pi`, `claude`).
    Provider,
    /// User, on a database shared by several people.
    User,
    /// Skill the work ran under.
    Skill,
    /// Agent or subagent.
    Agent,
    /// Session tag set in the dashboard.
    Tag,
    /// Kind of work, from AI session labels.
    Kind,
}

/// Kind of work from AI session labels (`KINDS` in `src/core/labels.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DashboardKind {
    /// New functionality.
    Feature,
    /// A bug fix.
    Bugfix,
    /// Restructuring without behavior change.
    Refactor,
    /// Test work.
    Tests,
    /// Documentation.
    Docs,
    /// Investigation.
    Research,
    /// Operations and tooling.
    Ops,
    /// Anything else.
    Other,
}

/// Arguments of `usage_summary`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct UsageSummaryArgs {
    /// Range to sum up; the dashboard defaults to `today`.
    pub range: Option<DashboardRange>,
    /// Start of a `custom` range: a local date (`YYYY-MM-DD`) or an ISO 8601 time.
    pub from: Option<String>,
    /// End of a `custom` range: a date (included) or an ISO 8601 time (excluded); default now.
    pub to: Option<String>,
    /// Sum up only this harness, e.g. `omp` or `pi`.
    pub provider: Option<String>,
    /// Sum up only this project: its path or its folder name.
    pub project: Option<String>,
    /// Sum up only this model id.
    pub model: Option<String>,
    /// Sum up only this user.
    pub user: Option<String>,
    /// Sum up only sessions with this tag (their subagents included).
    pub tag: Option<String>,
    /// Sum up only sessions of this kind of work.
    pub kind: Option<DashboardKind>,
}

/// Arguments of `breakdown`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct BreakdownArgs {
    /// What to group by.
    pub dimension: DashboardDimension,
    /// Range to split; the dashboard defaults to `30d`.
    pub range: Option<DashboardRange>,
    /// Start of a `custom` range.
    pub from: Option<String>,
    /// End of a `custom` range.
    pub to: Option<String>,
    /// Split only the usage of this harness.
    pub provider: Option<String>,
    /// Split only the usage of this project (path or folder name).
    pub project: Option<String>,
    /// Split only the usage of this model id.
    pub model: Option<String>,
    /// Split only the usage of this user.
    pub user: Option<String>,
    /// Split only sessions with this tag.
    pub tag: Option<String>,
    /// Split only sessions of this kind of work.
    pub kind: Option<DashboardKind>,
    /// Rows to return, 1 to [`MAX_DASHBOARD_ROWS`]; the dashboard defaults to 10.
    pub limit: Option<u32>,
}

/// Arguments of `session_cost`. The app requires `session_id` or `cwd`: the dashboard's own
/// fallback is its working directory, which is the app's, not the project's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct SessionCostArgs {
    /// The harness session id (OMP/PI session uuid, `SessionState.sessionId`).
    pub session_id: Option<String>,
    /// Project folder whose latest session is taken when `session_id` is absent.
    pub cwd: Option<String>,
}

/// Arguments of `branch_cost`. The app requires `branch` (see [`SessionCostArgs`] for why).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct BranchCostArgs {
    /// Git branch name.
    pub branch: Option<String>,
    /// Project path or folder name, when the branch name exists in several projects.
    pub project: Option<String>,
}

/// Arguments of `failures`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct FailuresArgs {
    /// Range to look at; the dashboard defaults to `7d`.
    pub range: Option<DashboardRange>,
    /// Start of a `custom` range.
    pub from: Option<String>,
    /// End of a `custom` range.
    pub to: Option<String>,
    /// Only failures in this harness.
    pub provider: Option<String>,
    /// Only failures in this project (path or folder name).
    pub project: Option<String>,
    /// Only failures under this model id.
    pub model: Option<String>,
    /// Only failures of this tool, e.g. `Edit` or `Bash`.
    pub tool: Option<String>,
    /// Recent failures to return, 1 to [`MAX_DASHBOARD_ROWS`]; the dashboard defaults to 10.
    pub limit: Option<u32>,
}

/// Arguments of `tips`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct TipsArgs {
    /// Range the tips are drawn from; the dashboard defaults to `30d`.
    pub range: Option<DashboardRange>,
    /// Start of a `custom` range.
    pub from: Option<String>,
    /// End of a `custom` range.
    pub to: Option<String>,
    /// Tips for this harness's usage only.
    pub provider: Option<String>,
    /// Tips for this project's usage only (path or folder name).
    pub project: Option<String>,
    /// Tips for this model's usage only.
    pub model: Option<String>,
    /// Tips for this user's usage only.
    pub user: Option<String>,
    /// Tips for sessions with this tag only.
    pub tag: Option<String>,
    /// Tips for sessions of this kind of work only.
    pub kind: Option<DashboardKind>,
    /// Tips to return, 1 to [`MAX_DASHBOARD_TIPS`]; the dashboard defaults to 10.
    pub limit: Option<u32>,
}

/// The empty argument object of `limits` and `budget_status`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NoArgs {}
