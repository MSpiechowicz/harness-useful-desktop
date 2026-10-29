//! Hooks: small JSON files that add context to a prompt, send a follow-up or run a command when
//! something happens in a desktop chat (`hooks_list`, `hook_save`, `hook_delete`, `hook_approve`,
//! `hooks_family`), and the team hooks shared through Harness Memory (`hooks_share`,
//! `hooks_team_remove`, `hooks_team_refresh`, `hooks_team_copy`, `hooks_team_enable`).
//!
//! The file format is the contract in `docs/hooks-format.md`; [`crate::hooks`] reads, validates,
//! writes and runs the files. The app window and its MCP server (`harness-desktop mcp`, used by
//! agents) share them. These are the shapes the webview sees.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// When a hook fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HookEvent {
    /// The first eligible prompt of a new desktop chat.
    SessionStart,
    /// Every eligible prompt.
    PromptSubmit,
    /// A tool call ended (and the matcher, if any, names it).
    ToolEnd,
    /// The agent settled after a user run.
    TurnEnd,
    /// The chat exited or was stopped (commands only).
    SessionEnd,
}

impl HookEvent {
    /// The name in the file (`sessionStart`, …).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionStart => "sessionStart",
            Self::PromptSubmit => "promptSubmit",
            Self::ToolEnd => "toolEnd",
            Self::TurnEnd => "turnEnd",
            Self::SessionEnd => "sessionEnd",
        }
    }

    /// The event named `name` in a file.
    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::SessionStart,
            Self::PromptSubmit,
            Self::ToolEnd,
            Self::TurnEnd,
            Self::SessionEnd,
        ]
        .into_iter()
        .find(|event| event.as_str() == name)
    }
}

/// Where a hook applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HookScope {
    /// Every desktop chat.
    Global,
    /// Chats whose project root is the hook's `project`.
    Project,
}

/// What a hook does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum HookAction {
    /// Puts `text` in front of the prompt (`sessionStart`, `promptSubmit`).
    AddContext {
        /// 1 to 4096 characters.
        text: String,
    },
    /// Sends `text` after the agent settles (`toolEnd`, `turnEnd`).
    FollowUp {
        /// 1 to 4096 characters.
        text: String,
    },
    /// Runs `command` in a shell; its stdout is used like the text of the matching action.
    Command {
        /// 1 to 2048 characters.
        command: String,
        /// 1 to 30 seconds; `None` means 10.
        timeout_secs: Option<u32>,
    },
}

/// Who wrote a hook file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HookAuthor {
    /// The user, in the Harness Desktop window (including the Harness family hooks).
    Desktop,
    /// An agent, through the Harness Desktop MCP server (`harness-desktop mcp`).
    Agent,
}

/// Something a hook needs before it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum HookRequirement {
    /// Harness Memory is installed and its memory setting is on.
    #[serde(rename = "harness-memory")]
    HarnessMemory,
    /// Harness Dashboard is installed.
    #[serde(rename = "harness-dashboard")]
    HarnessDashboard,
}

impl HookRequirement {
    /// The name in the file.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HarnessMemory => "harness-memory",
            Self::HarnessDashboard => "harness-dashboard",
        }
    }

    /// The requirement named `name`.
    pub fn parse(name: &str) -> Option<Self> {
        [Self::HarnessMemory, Self::HarnessDashboard]
            .into_iter()
            .find(|requirement| requirement.as_str() == name)
    }
}

/// Which Harness family hook a hook is (the catalog in [`crate::hooks::family`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum HookSeed {
    /// Harness Memory: "Search Harness Memory first" (`sessionStart`).
    MemorySearch,
    /// Harness Memory: "Save what you learned" (`turnEnd`).
    MemorySave,
    /// Harness Dashboard: "Check limits before long work" (`sessionStart`).
    DashboardLimits,
    /// Harness Memory: "Check Useful Skills first" (`promptSubmit`).
    UsefulSkills,
    /// Harness Memory: "Check the branch against main" (`sessionStart`).
    BranchSync,
    /// Harness Memory: "Check Memory first" (`promptSubmit`).
    MemoryFirst,
    /// Harness Memory: "Use the code graph first" (`promptSubmit`). Offered only while the installed
    /// Harness Memory lists `code-index` (and runs only while the Code index switch is on).
    CodeGraphFirst,
}

impl HookSeed {
    /// Every family hook, in catalog order.
    pub const ALL: [Self; 7] = [
        Self::MemorySearch,
        Self::MemorySave,
        Self::DashboardLimits,
        Self::UsefulSkills,
        Self::BranchSync,
        Self::MemoryFirst,
        Self::CodeGraphFirst,
    ];

    /// The name in the file and in `hooks-state.json`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemorySearch => "memory-search",
            Self::MemorySave => "memory-save",
            Self::DashboardLimits => "dashboard-limits",
            Self::UsefulSkills => "useful-skills",
            Self::BranchSync => "branch-sync",
            Self::MemoryFirst => "memory-first",
            Self::CodeGraphFirst => "code-graph-first",
        }
    }

    /// True when the hook also needs the code graph of Harness Memory (see
    /// `HookCapabilities::code_graph_available`), beyond its `requires`.
    pub fn needs_code_graph(self) -> bool {
        self == Self::CodeGraphFirst
    }

    /// The seed named `name`.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|seed| seed.as_str() == name)
    }
}

/// How a Harness family hook in the folder compares with the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HookFamilyStatus {
    /// A valid file with this seed matches the catalog's name, event and action.
    Installed,
    /// A valid file with this seed differs from the catalog (an older text, or the user's edit).
    Outdated,
    /// No valid file has this seed.
    Missing,
    /// The family member it is for is not installed.
    NeedsMember,
}

/// One Harness family hook and its state (`hooks_family`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HookFamilyEntry {
    /// Which family hook.
    pub seed: HookSeed,
    /// The family member it is for (also its requirement).
    pub member: HookRequirement,
    /// Its catalog name.
    pub name: String,
    /// When it fires.
    pub event: HookEvent,
    /// How the folder compares with the catalog.
    pub status: HookFamilyStatus,
    /// The file with this seed, when there is one.
    pub hook_id: Option<String>,
    /// An `outdated` file that still holds an earlier catalog version word for word (not an edit of
    /// the user's), so it may be updated without asking.
    pub stale: bool,
}

/// A valid hook file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Hook {
    /// 32 lowercase hex characters, the file name stem.
    pub id: String,
    /// 1 to 80 characters.
    pub name: String,
    /// At most 500 characters.
    pub description: Option<String>,
    /// Global or one project.
    pub scope: HookScope,
    /// The canonical project root of a project hook.
    pub project: Option<String>,
    /// When it fires.
    pub event: HookEvent,
    /// `toolEnd` only: tool names joined by `|` (ASCII case-insensitive); `None` means every tool.
    pub matcher: Option<String>,
    /// What it does.
    pub action: HookAction,
    /// Off hooks never run.
    pub enabled: bool,
    /// What it needs before it runs.
    pub requires: Vec<HookRequirement>,
    /// Set on the seeded defaults.
    pub seed: Option<HookSeed>,
    /// RFC 3339 UTC.
    pub created_at: String,
    /// Who created the file (decides who may remove it).
    pub created_by: HookAuthor,
    /// RFC 3339 UTC.
    pub updated_at: String,
    /// The last writer.
    pub updated_by: HookAuthor,
}

/// Whether a hook runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HookStatus {
    /// Valid, enabled, approved and its requirements are met: it runs.
    Active,
    /// Valid and approved, but turned off.
    Off,
    /// Valid, but this exact file was never approved (an agent wrote or changed it), or a team
    /// hook whose exact bytes are not approved while "Trust team hooks" is off.
    Pending,
    /// Valid, enabled and approved, but waiting for a requirement (Harness Memory).
    Paused,
    /// The file breaks the format (see `HookView.problem`); it never runs.
    Invalid,
    /// A team hook whose id a local hook also has: the local one wins, this one never runs.
    Duplicate,
}

/// Where a listed hook comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HookOrigin {
    /// A file in this machine's hooks folder.
    #[default]
    Local,
    /// A copy of a hook shared through the Harness Memory team database.
    Team,
}

/// Why a local hook cannot be shared with the team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ShareBlock {
    /// A Harness family hook: every machine seeds its own.
    Seed,
    /// Waiting for review: approve or reject it first.
    Pending,
    /// The file breaks the format.
    Invalid,
    /// A project hook in a repository without an `origin` remote (a `path:` identity).
    NoOrigin,
    /// The team already has a hook with this id: update the team copy instead.
    OnTeam,
}

/// How the approvals relate to a hook file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HookApproval {
    /// The current bytes are approved.
    Approved,
    /// An earlier version was approved; the file changed since.
    Changed,
    /// Never approved.
    Never,
}

/// One hook file, or one team hook (`origin: team`), as the Hooks view lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HookView {
    /// The file name stem.
    pub id: String,
    /// The parsed hook; `None` when the file is invalid.
    pub hook: Option<Hook>,
    /// A readable name even for an invalid file (its `name` when that is a string, else `None`).
    pub name: Option<String>,
    /// Whether it runs.
    pub status: HookStatus,
    /// Lowercase SHA-256 of the file bytes; pass it back to approve, edit or delete this version.
    pub sha256: String,
    /// Why the file is invalid.
    pub problem: Option<String>,
    /// Whether this or an earlier version was approved.
    pub approval: HookApproval,
    /// The approved earlier version, for a diff: set when `approval` is `changed` and the app kept
    /// a copy of that version.
    pub approved: Option<Hook>,
    /// A local file or a team hook.
    pub origin: HookOrigin,
    /// Team hooks: who shared it (a free-text label, the sharer's user name by default).
    pub team_author: Option<String>,
    /// Team hooks: when the team copy last changed (RFC 3339 UTC).
    pub team_updated_at: Option<String>,
    /// Team hooks: the repository key (`git:host/owner/repo`) of a project hook, `None` for a global
    /// one.
    pub team_repo: Option<String>,
    /// Team hooks: the local hook with the same id changed since it was shared ("Update team
    /// copy").
    pub team_changed: bool,
    /// Local hooks: why "Share with team" is not possible, `None` when it is. Always `None` on a
    /// team hook.
    pub share_block: Option<ShareBlock>,
    /// Team hooks: turned off on this machine only (never written to the team).
    pub team_disabled: bool,
    /// The project root of a project hook: a local file's (also over its scope's limit), a team
    /// hook's only once a refresh of this session confirmed a root of its repository. `None` for
    /// a global hook or an unknown scope.
    pub root: Option<String>,
    /// The repository key of a project hook: a team hook's, or the one a refresh of this session
    /// confirmed for a local file's root. `None` for a global hook or while unknown.
    pub repo_key: Option<String>,
}

/// The team hooks as this machine sees them, while sharing is on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TeamHooks {
    /// "Trust team hooks" is on: team hooks run without local approval.
    pub trust: bool,
    /// Why the latest refresh failed (the copies of the one before are used), `None` after a good
    /// one.
    pub problem: Option<String>,
    /// When the copies were last refreshed (RFC 3339 UTC), `None` before the first refresh.
    pub refreshed_at: Option<String>,
}

/// The hooks folder (and the team hooks) as the Hooks view shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HookListing {
    /// Every `<id>.json` file: valid ones in run order (global first, then `createdAt`, then
    /// `id`), invalid ones after them by id. Then the team hooks the same way (`origin: team`).
    pub hooks: Vec<HookView>,
    /// Why the folder is not read (for example writable by other users); `hooks` is then empty.
    pub folder_problem: Option<String>,
    /// The team hooks' state; `None` while sharing is off, the installed Harness Memory has no
    /// team hooks, or it is not installed. Team hooks are listed after the local ones.
    pub team: Option<TeamHooks>,
}

/// What `hooks_delete_all` removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HooksDeleted {
    /// How many hook files were deleted.
    pub deleted: u32,
}

/// What the Hooks view sends to create or edit a hook (`hook_save`). The desktop keeps the
/// file's `createdAt`, `createdBy`, `seed`, `requires` and unknown fields on an edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HookDraft {
    /// The hook to edit; `None` creates one.
    pub id: Option<String>,
    /// For an edit: `HookView.sha256` of the version the user edited. A file changed since is a
    /// conflict.
    pub expected_sha256: Option<String>,
    /// 1 to 80 characters.
    pub name: String,
    /// At most 500 characters; empty means none.
    pub description: Option<String>,
    /// Global or one project.
    pub scope: HookScope,
    /// For a project hook: a folder of the project (its project root is stored).
    pub project: Option<String>,
    /// When it fires.
    pub event: HookEvent,
    /// `toolEnd` only; empty means every tool.
    pub matcher: Option<String>,
    /// What it does.
    pub action: HookAction,
    /// Whether it runs.
    pub enabled: bool,
}

/// What `hooks_listen` sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum HooksEvent {
    /// The hooks folder, the approvals or the team hooks changed: read the list again.
    Changed,
}
