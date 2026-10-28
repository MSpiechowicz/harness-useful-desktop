//! The deterministic scripts agents keep in Harness Memory (`memory_scripts_list`,
//! `memory_script_get`, `memory_script_delete`, `memory_scripts_delete_all`,
//! `memory_scripts_trust`), and what deleting every note or script answers.
//!
//! The app lists them with `harness-memory scripts list --json`, reads and deletes them through
//! `harness-memory mcp` (`script_get`, `script_delete`), and never opens the memory database
//! itself.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Whether the installed Harness Memory can list scripts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum MemoryScriptsAvailability {
    /// `harness-memory mcp` started and offers `script_list`.
    Available,
    /// No `harness-memory` binary was found.
    NotInstalled,
    /// The installed server predates scripts (its tools lack `script_list`).
    Outdated,
    /// The binary exists but the MCP server could not be used.
    Failed {
        /// Short, secret-free reason.
        message: String,
    },
}

/// Which notes model a script belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ScriptScope {
    /// One repository's script.
    Repo,
    /// Available in every project.
    Global,
}

/// Where the server keeps a note or script, and which of them `memory_notes_list` and
/// `memory_scripts_list` keep (`place`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScriptPlace {
    /// The team database.
    Team,
    /// The local SQLite file on this machine.
    Local,
}

/// Whether this machine runs a script without asking the user first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ScriptApproval {
    /// Saved by an agent on this machine.
    Local,
    /// The user approved this version (in the app or with `harness-memory trust`).
    Approved,
    /// From the team database, trusted because `trust_team_scripts = true`.
    Team,
    /// Not approved: the server asks the user before running it.
    Needed,
}

/// The agent that wrote a script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ScriptAuthor {
    /// The harness that saved it (`claude-code`, `codex`, …).
    pub source: String,
    /// The project root it was saved from, if known.
    pub root: Option<String>,
}

/// One script without its body (`script_list`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScriptSummary {
    /// The server's id.
    #[ts(type = "number")]
    pub id: i64,
    /// Unique per scope (`^[a-z0-9][a-z0-9_-]{0,63}$`).
    pub name: String,
    /// What the script does, as the agent described it.
    pub description: String,
    /// The interpreter (`sh`, `bash`, `python`, `node`, `bun`).
    pub language: String,
    /// Repository or global.
    #[ts(inline)]
    pub scope: ScriptScope,
    /// The repository key (`github.com/o/r`) of a repository script.
    pub repo: Option<String>,
    /// Lowercase hex SHA-256 of the body.
    pub sha256: String,
    /// The run time limit in seconds.
    pub timeout_secs: u32,
    /// The team database or this computer.
    #[ts(inline)]
    pub place: ScriptPlace,
    /// Who saved it.
    #[ts(inline)]
    pub author: ScriptAuthor,
    /// RFC 3339.
    pub created_at: String,
    /// RFC 3339.
    pub updated_at: String,
    /// How often it ran.
    pub run_count: u32,
    /// RFC 3339, `None` before the first run.
    pub last_run_at: Option<String>,
    /// The exit code of the last run; `None` before the first run or after a timeout or signal.
    pub last_exit_code: Option<i32>,
    /// True when this machine trusts the current body (written here or reviewed and run).
    pub trusted: bool,
    /// Why it is trusted, or `needed`. A server that does not say is read from `trusted`
    /// (`approved` or `needed`).
    #[ts(inline)]
    pub approval: ScriptApproval,
    /// True for a team script while scripts are routed `remote+local`: it cannot be changed or
    /// deleted from this machine, only hidden here. False from a server that does not say.
    #[serde(default)]
    pub read_only: bool,
    /// True for a team script hidden on this machine (listed only when hidden scripts are asked
    /// for). False from a server that does not say.
    #[serde(default)]
    pub hidden: bool,
}

/// One script with its body (`script_get`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScriptDetail {
    /// Everything [`ScriptSummary`] holds.
    #[serde(flatten)]
    pub summary: ScriptSummary,
    /// The source code.
    pub body: String,
}

/// The scripts `memory_scripts_list` keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScriptGroup {
    /// The ones that need the user's approval (`approval: "needed"`).
    NeedsReview,
    /// The project's repository scripts.
    Repo,
    /// The global ones.
    Global,
}

/// Which scripts `memory_scripts_list` keeps by approval (with a server that combines filters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScriptReview {
    /// The ones that need the user's approval (`approval: "needed"`).
    Needed,
    /// Every other one (`local`, `approved` or `team`).
    Trusted,
}

/// How many notes or scripts of each scope match the other filters (`memory_notes_list`: the
/// query and kind, `memory_scripts_list`: the query and review).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryScopeCounts {
    /// The project's repository entries.
    pub repo: u32,
    /// The global ones.
    pub global: u32,
}

/// How many notes or scripts of each place match every other filter (`memory_notes_list`: the
/// query, kind and scope, `memory_scripts_list`: the query, scope and review).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryPlaceCounts {
    /// Those in the local file on this machine.
    pub local: u32,
    /// Those in the team database.
    pub team: u32,
}

/// How many scripts need the user's approval and how many do not, under the query and scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScriptReviewCounts {
    /// Those that need the user's approval.
    pub needed: u32,
    /// Every other one.
    pub trusted: u32,
}

/// How `memory_scripts_list` sorts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScriptSort {
    /// By the last change.
    Updated,
    /// By name.
    Name,
    /// By the language's name.
    Language,
    /// Global scripts before the repository's (ascending).
    Scope,
    /// Where they are kept (the server keeps every listed script in one place).
    Place,
    /// By how often they ran.
    Runs,
    /// By the latest run; a script never run comes last whichever the order.
    LastRun,
}

/// How many scripts of each group match the query. The groups are disjoint: a script that needs
/// review counts only in `needs_review`, `repo` and `global` count the trusted ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScriptGroupCounts {
    /// Those that need the user's approval.
    pub needs_review: u32,
    /// The project's repository scripts.
    pub repo: u32,
    /// The global ones.
    pub global: u32,
}

/// The answer of `memory_scripts_list`: one page of scripts when available, empty otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryScriptsPage {
    /// Whether the list could be read.
    pub availability: MemoryScriptsAvailability,
    /// How many scripts match the query and every filter asked for (all pages).
    pub total: u32,
    /// The scripts of each group under the query, whatever group was asked for.
    pub groups: ScriptGroupCounts,
    /// The scripts of each scope under the query, review and place; `None` from a server that
    /// does not combine filters.
    pub scopes: Option<MemoryScopeCounts>,
    /// The scripts that need approval and the others under the query, scope and place; `None`
    /// from a server that does not combine filters.
    pub review: Option<ScriptReviewCounts>,
    /// The scripts of each place under the query, scope and review; `None` from a server that
    /// does not filter by place, whose list then keeps every place (a `place` asked for was
    /// dropped).
    pub places: Option<MemoryPlaceCounts>,
    /// Whether the server combines `scope` and `review` (and answered their counts): false from
    /// an older one, whose list then follows the query and group only (a `scope` or `review`
    /// asked for was dropped).
    pub combined_filters: bool,
    /// This page, as the server sorts it.
    pub scripts: Vec<ScriptSummary>,
    /// How many team scripts in the view are hidden on this machine; `None` without a team
    /// database or from a server that does not say.
    pub hidden_count: Option<u32>,
    /// How many scripts of each scope the local file on this machine holds, every repository
    /// counted; `None` without a team database or from a server that does not say.
    pub local_counts: Option<MemoryScopeCounts>,
    /// The server could not reach the team database and answered from the scripts on this
    /// computer instead, so a team's scripts are missing from this page: it is not what is kept.
    pub offline: bool,
    /// With `offline`, when this page was last read from the team database and kept on this
    /// computer; the page is that copy. `None` when no copy of this page was kept (the page is
    /// empty) or the page is live.
    pub cached_at: Option<String>,
}

/// The answer of `memory_notes_delete_all` and `memory_scripts_delete_all`: how many notes or
/// scripts `harness-memory notes|scripts delete --all` deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryDeleteAll {
    /// How many were deleted, wherever they were kept.
    pub deleted: u32,
    /// How many of them were in the team database; `None` when the server does not say.
    pub team: Option<u32>,
    /// How many team notes were left in place in scopes routed `remote+local` (they can only be
    /// hidden there); `None` when the server does not say.
    pub kept: Option<u32>,
}

/// One script the user approves (`memory_scripts_trust`): its id and the SHA-256 of the version
/// the user reviewed, so a version changed since then is not approved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTrustItem {
    /// The server's id.
    pub id: i64,
    /// The reviewed version's SHA-256 (64 lowercase hex characters).
    pub sha256: String,
}
