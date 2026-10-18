//! The status of each hook file (see [`HookStatus`]), run order, and the per-scope limit.

use std::collections::BTreeMap;

use super::approvals::Approvals;
use super::format::{loose_name, parse_hook_file, timestamp_key};
use crate::model::{Hook, HookApproval, HookOrigin, HookRequirement, HookScope, HookStatus, HookView};

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
#[derive(Debug, Clone)]
pub(crate) struct HookFile {
    pub id: String,
    pub bytes: Vec<u8>,
    pub sha256: String,
    pub unreadable: bool,
}

/// The scope a hook counts against: `None` for global, the project root otherwise.
pub(crate) fn scope_key(hook: &Hook) -> Option<&str> {
    match hook.scope {
        HookScope::Global => None,
        HookScope::Project => hook.project.as_deref(),
    }
}

/// Whose a listed file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileOwner {
    /// A global hook, used or over the limit.
    Global,
    /// A hook of this project root, used or over the limit.
    Project(String),
    /// A file that is not a valid hook: its scope is unknown.
    Unknown,
}

impl FileOwner {
    fn of(hook: &Hook) -> Self {
        match (hook.scope, &hook.project) {
            (HookScope::Global, _) => Self::Global,
            (HookScope::Project, Some(project)) => Self::Project(project.clone()),
            (HookScope::Project, None) => Self::Unknown,
        }
    }

    /// True when a list for a chat in `project` (a project root; `None`: global only) shows the
    /// file: a global hook, a hook of that project, or a file whose scope is unknown.
    pub(crate) fn shown_in(&self, project: Option<&str>) -> bool {
        match self {
            Self::Global | Self::Unknown => true,
            Self::Project(root) => project == Some(root.as_str()),
        }
    }
}

/// Where a hook runs: by origin and scope, then `createdAt`, then `id`.
pub(crate) type RunOrder = (u8, [u32; 7], String);

/// Run order of a local hook: global before project, then `createdAt`, then `id`.
pub(crate) fn order_key(hook: &Hook) -> RunOrder {
    ranked(hook, 0)
}

/// Run order of a team hook: after every local hook, team global before team project.
pub(crate) fn team_order_key(hook: &Hook) -> RunOrder {
    ranked(hook, 2)
}

fn ranked(hook: &Hook, base: u8) -> RunOrder {
    let rank = match hook.scope {
        HookScope::Global => base,
        HookScope::Project => base + 1,
    };

    (
        rank,
        timestamp_key(&hook.created_at).unwrap_or_default(),
        hook.id.clone(),
    )
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
    let mut valid: Vec<(Hook, HookFile)> = Vec::new();
    let mut invalid: Vec<(HookFile, String, FileOwner)> = Vec::new();

    for file in files {
        if file.unreadable {
            invalid.push((file, "the file could not be read".to_owned(), FileOwner::Unknown));
            continue;
        }

        match parse_hook_file(&file.id, &file.bytes) {
            Ok(hook) => valid.push((hook, file)),
            Err(problem) => invalid.push((file, problem, FileOwner::Unknown)),
        }
    }

    // Per scope MAX_HOOKS_PER_SCOPE are used: approved ones first (so files waiting for review
    // cannot push them out), the oldest first within each group.
    valid.sort_by_key(|(hook, file)| (!approvals.is_approved(&file.id, &file.sha256), order_key(hook)));

    let mut counts: BTreeMap<Option<String>, usize> = BTreeMap::new();
    let mut used = Vec::new();

    for (hook, file) in valid {
        let count = counts.entry(scope_key(&hook).map(str::to_owned)).or_default();
        *count += 1;

        // An over-the-limit hook is listed as invalid, but only where its scope is.
        if *count > MAX_HOOKS_PER_SCOPE {
            let problem = format!("over the limit of {MAX_HOOKS_PER_SCOPE} hooks in this scope");
            invalid.push((file, problem, FileOwner::of(&hook)));
        } else {
            let owner = FileOwner::of(&hook);
            used.push((view_of(hook, file.sha256, approvals, capabilities, approved), owner));
        }
    }

    // Listed in run order.
    used.sort_by_key(|(view, _)| view.hook.as_ref().map(order_key));

    invalid.sort_by(|left, right| left.0.id.cmp(&right.0.id));

    used.extend(invalid.into_iter().map(|(file, problem, owner)| {
        let approval = approval_of(approvals, &file.id, &file.sha256);

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

    used
}

/// The view of the valid `hook` whose file hashes to `sha256`.
pub(crate) fn view_of(
    hook: Hook,
    sha256: String,
    approvals: &Approvals,
    capabilities: HookCapabilities,
    approved: &dyn Fn(&str) -> Option<Hook>,
) -> HookView {
    let approval = approval_of(approvals, &hook.id, &sha256);

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
    }
}

pub(crate) fn earlier_version(
    approval: HookApproval,
    id: &str,
    approved: &dyn Fn(&str) -> Option<Hook>,
) -> Option<Hook> {
    match approval {
        HookApproval::Changed => approved(id),
        HookApproval::Approved | HookApproval::Never => None,
    }
}

pub(crate) fn approval_of(approvals: &Approvals, id: &str, sha256: &str) -> HookApproval {
    match approvals.get(id) {
        Some(approved) if approved == sha256 => HookApproval::Approved,
        Some(_) => HookApproval::Changed,
        None => HookApproval::Never,
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
