//! The status of each hook file (see [`HookStatus`]), run order, and the per-scope limit (the
//! scope rules of [`crate::signed_docs::scope`]).

use super::approvals::Approvals;
use super::format::loose_name;
use crate::model::{Hook, HookApproval, HookOrigin, HookRequirement, HookStatus, HookView};
use crate::signed_docs::listing::sort_files;
use crate::signed_docs::scope::{ranked, scoped_doc};

pub(crate) use crate::signed_docs::listing::earlier_version;

pub(crate) use crate::signed_docs::scope::{FileOwner, RunOrder};

/// Most hooks used per scope (global, and each project separately).
pub const MAX_HOOKS_PER_SCOPE: usize = 64;

/// What the hooks' `requires` can be checked against.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HookCapabilities {
    /// Harness Memory is installed and its memory setting is on (`harness-memory`).
    pub memory_available: bool,
    /// Harness Dashboard is installed (`harness-dashboard`).
    pub dashboard_available: bool,
}

impl HookCapabilities {
    /// True when `requirement` is met.
    pub fn meets(self, requirement: HookRequirement) -> bool {
        match requirement {
            HookRequirement::HarnessMemory => self.memory_available,
            HookRequirement::HarnessDashboard => self.dashboard_available,
        }
    }
}

/// One `<id>.json` file as read.
pub(crate) use crate::signed_docs::DocFile as HookFile;

scoped_doc!(Hook);

/// Run order of a local hook: global before project, then `createdAt`, then `id`.
pub(crate) fn order_key(hook: &Hook) -> RunOrder {
    ranked(hook, 0)
}

/// Run order of a team hook: after every local hook, team global before team project.
pub(crate) fn team_order_key(hook: &Hook) -> RunOrder {
    ranked(hook, 2)
}

/// The list with whose each file is: valid hooks in run order, then invalid files by id.
/// `approved` returns the kept copy of an approved earlier version (for [`HookApproval::Changed`]
/// files).
pub(crate) fn views(
    files: Vec<HookFile>,
    approvals: &Approvals,
    capabilities: HookCapabilities,
    approved: &dyn Fn(&str) -> Option<Hook>,
) -> Vec<(HookView, FileOwner)> {
    let sorted = sort_files(files, approvals);

    let mut views: Vec<(HookView, FileOwner)> = sorted
        .used
        .into_iter()
        .map(|(hook, file)| {
            let owner = FileOwner::of(&hook);
            (view_of(hook, file.sha256, approvals, capabilities, approved), owner)
        })
        .collect();

    views.extend(sorted.invalid.into_iter().map(|(file, problem, owner)| {
        let approval = approvals.approval(&file.id, &file.sha256);

        let view = HookView {
            name: loose_name(&file.bytes),
            status: HookStatus::Invalid,
            approved: earlier_version(approval, &file.id, approved),
            approval,
            problem: Some(problem),
            hook: None,
            id: file.id,
            sha256: file.sha256,
            ..local_defaults()
        };

        (view, owner)
    }));

    views
}

/// The view of the valid `hook` whose file hashes to `sha256`.
pub(crate) fn view_of(
    hook: Hook,
    sha256: String,
    approvals: &Approvals,
    capabilities: HookCapabilities,
    approved: &dyn Fn(&str) -> Option<Hook>,
) -> HookView {
    let approval = approvals.approval(&hook.id, &sha256);

    HookView {
        id: hook.id.clone(),
        name: Some(hook.name.clone()),
        status: status_of(&hook, approval, capabilities),
        sha256,
        problem: None,
        approved: earlier_version(approval, &hook.id, approved),
        approval,
        hook: Some(hook),
        ..local_defaults()
    }
}

/// The fields of a local file's view that only team hooks (or the app window's listing) fill.
pub(crate) fn local_defaults() -> HookView {
    HookView {
        id: String::new(),
        hook: None,
        name: None,
        status: HookStatus::Invalid,
        sha256: String::new(),
        problem: None,
        approval: HookApproval::Never,
        approved: None,
        origin: HookOrigin::Local,
        team_author: None,
        team_updated_at: None,
        team_repo: None,
        team_changed: false,
        share_block: None,
        team_disabled: false,
        root: None,
        repo_key: None,
    }
}

/// Pending until this exact file is approved, then off, waiting for a requirement, or active.
fn status_of(hook: &Hook, approval: HookApproval, capabilities: HookCapabilities) -> HookStatus {
    if approval != HookApproval::Approved {
        return HookStatus::Pending;
    }

    if !hook.enabled {
        return HookStatus::Off;
    }

    if !hook.requires.iter().all(|requirement| capabilities.meets(*requirement)) {
        return HookStatus::Paused;
    }

    HookStatus::Active
}
