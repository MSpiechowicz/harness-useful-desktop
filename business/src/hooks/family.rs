//! Harness family hooks: a catalog of hooks for the family members agents use over MCP (Harness
//! Memory, Harness Dashboard), each marked by its `seed`.
//!
//! - Launch seeding ([`seed_defaults`]): a member's hooks are created once while it is
//!   available, global, `requires` the member, written and approved by the desktop. Each seed is
//!   recorded in `<app_config_dir>/hooks-state.json` and never created at launch again, even after
//!   the user deletes it. A seed hook already in the folder (a lost record) is recorded, not
//!   duplicated.
//! - The Harness family hooks dialog ([`family_status`], [`apply_family`]) adds missing hooks and
//!   updates outdated ones on the user's request.
//! - After a member is installed, [`restore_member`] adds its missing hooks again (never
//!   overwriting one that exists).
//! - Uninstalling a member deletes nothing: its hooks wait (paused) for it.
//! - Only an approved file counts as a seed's hook. A pending file with a seed (an edit the user
//!   has not reviewed, or a file someone else wrote) is replaced by the catalog version when the
//!   app adds that seed, approved like every app write.

use std::time::SystemTime;

use super::state::{load_state, save_state};
use super::store::{HookCapabilities, HookStore};
use crate::agent::rfc3339;
use crate::error::Result;
use crate::model::{
    Hook, HookAction, HookApproval, HookAuthor, HookEvent, HookFamilyEntry, HookFamilyStatus, HookListing,
    HookRequirement, HookScope, HookSeed, HookView,
};

/// The `memory-search` context.
pub const SEED_MEMORY_SEARCH_TEXT: &str = "Before you start, search Harness Memory for notes about this task \
     (memory_search) and check the saved scripts (script_list). Reuse what fits.";

/// The `memory-save` follow-up.
pub const SEED_MEMORY_SAVE_TEXT: &str = "If this turn taught you something worth keeping, save it to Harness \
     Memory (memory_remember or memory_update). Save any command sequence you would run again as a script \
     (script_save). Reply briefly, or say nothing needs saving.";

/// The `dashboard-limits` context.
pub const SEED_DASHBOARD_LIMITS_TEXT: &str = "Before a long or expensive task, check the plan limits and budgets \
     with Harness Dashboard (limits, budget_status). Say so if a limit is close.";

/// One catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamilyHook {
    /// Its seed.
    pub seed: HookSeed,
    /// The member it is for; also its only requirement.
    pub member: HookRequirement,
    /// Its name.
    pub name: &'static str,
    /// Its description.
    pub description: &'static str,
    /// When it fires.
    pub event: HookEvent,
    /// The text it adds (`sessionStart`: context) or sends (`turnEnd`: a follow-up).
    pub text: &'static str,
}

impl FamilyHook {
    /// The action of the current catalog version.
    pub fn action(&self) -> HookAction {
        let text = self.text.to_owned();

        match self.event {
            HookEvent::SessionStart | HookEvent::PromptSubmit => HookAction::AddContext { text },
            HookEvent::ToolEnd | HookEvent::TurnEnd | HookEvent::SessionEnd => HookAction::FollowUp { text },
        }
    }

    /// True when `hook` matches this version (name, event and action).
    fn matches(&self, hook: &Hook) -> bool {
        hook.name == self.name && hook.event == self.event && hook.action == self.action()
    }

    /// A new file of this entry, created now with a fresh id.
    fn new_hook(&self, now: SystemTime) -> Hook {
        self.hook_with_id(uuid::Uuid::new_v4().simple().to_string(), now)
    }

    /// The current version of this entry as hook `id`, written `now`.
    fn hook_with_id(&self, id: String, now: SystemTime) -> Hook {
        let now = rfc3339(now);

        Hook {
            id,
            name: self.name.to_owned(),
            description: Some(self.description.to_owned()),
            scope: HookScope::Global,
            project: None,
            event: self.event,
            matcher: None,
            action: self.action(),
            enabled: true,
            requires: vec![self.member],
            seed: Some(self.seed),
            created_at: now.clone(),
            created_by: HookAuthor::Desktop,
            updated_at: now,
            updated_by: HookAuthor::Desktop,
        }
    }
}

/// The catalog, in the order the dialog lists it.
pub const FAMILY: [FamilyHook; 3] = [
    FamilyHook {
        seed: HookSeed::MemorySearch,
        member: HookRequirement::HarnessMemory,
        name: "Search Harness Memory first",
        description: "Seeded default. Asks the agent to look up notes and scripts before it starts.",
        event: HookEvent::SessionStart,
        text: SEED_MEMORY_SEARCH_TEXT,
    },
    FamilyHook {
        seed: HookSeed::MemorySave,
        member: HookRequirement::HarnessMemory,
        name: "Save what you learned",
        description: "Seeded default. Asks the agent to keep lessons and repeatable commands after a turn.",
        event: HookEvent::TurnEnd,
        text: SEED_MEMORY_SAVE_TEXT,
    },
    FamilyHook {
        seed: HookSeed::DashboardLimits,
        member: HookRequirement::HarnessDashboard,
        name: "Check limits before long work",
        description: "Seeded default. Asks the agent to check plan limits and budgets before a long task.",
        event: HookEvent::SessionStart,
        text: SEED_DASHBOARD_LIMITS_TEXT,
    },
];

/// The catalog entry of `seed`.
pub fn family_hook(seed: HookSeed) -> &'static FamilyHook {
    FAMILY.iter().find(|entry| entry.seed == seed).unwrap_or(&FAMILY[0])
}

/// The first approved valid hook of `listing` with `seed`, in run order.
fn seeded_in(listing: &HookListing, seed: HookSeed) -> Option<&Hook> {
    seed_file(listing, seed, true).map(|(_, hook)| hook)
}

/// The first valid file of `listing` with `seed` that is approved (`approved`) or pending.
fn seed_file(listing: &HookListing, seed: HookSeed, approved: bool) -> Option<(&HookView, &Hook)> {
    listing.hooks.iter().find_map(|view| {
        let hook = view.hook.as_ref()?;
        let is_approved = view.approval == HookApproval::Approved;

        (hook.seed == Some(seed) && is_approved == approved).then_some((view, hook))
    })
}

/// Each catalog entry against the folder. `installed`: which members are installed (Harness
/// Memory counts whatever its memory setting).
pub fn family_status(listing: &HookListing, installed: HookCapabilities) -> Vec<HookFamilyEntry> {
    FAMILY
        .iter()
        .map(|entry| {
            let hook = seeded_in(listing, entry.seed);

            let status = match hook {
                _ if !installed.meets(entry.member) => HookFamilyStatus::NeedsMember,
                None => HookFamilyStatus::Missing,
                Some(hook) if entry.matches(hook) => HookFamilyStatus::Installed,
                Some(_) => HookFamilyStatus::Outdated,
            };

            HookFamilyEntry {
                seed: entry.seed,
                member: entry.member,
                name: entry.name.to_owned(),
                event: entry.event,
                status,
                hook_id: hook.map(|hook| hook.id.clone()),
            }
        })
        .collect()
}

/// Launch seeding: creates the hooks of every member `available` (Harness Memory: installed and
/// its setting on) that were never seeded. Returns the seeds created now.
pub fn seed_defaults(store: &HookStore, available: HookCapabilities, now: SystemTime) -> Result<Vec<HookSeed>> {
    let mut state = load_state(store.config_dir())?;
    let due: Vec<&FamilyHook> = FAMILY
        .iter()
        .filter(|entry| available.meets(entry.member) && !state.has(entry.seed))
        .collect();

    if due.is_empty() {
        return Ok(Vec::new());
    }

    let listing = store.list(available);
    let mut created = Vec::new();

    for entry in due {
        if seeded_in(&listing, entry.seed).is_none() {
            install(store, &listing, entry, now)?;
            created.push(entry.seed);
        }

        state.record(entry.seed);
    }

    save_state(store.config_dir(), &state)?;
    Ok(created)
}

/// The user's request in the dialog: adds the missing hooks of `seeds` and brings outdated ones to
/// the current catalog version (name, description, event, action), as desktop writes (approved).
/// The member need not be installed: its hook then waits for it. Returns the seeds changed.
pub fn apply_family(
    store: &HookStore,
    seeds: &[HookSeed],
    capabilities: HookCapabilities,
    now: SystemTime,
) -> Result<Vec<HookSeed>> {
    let listing = store.list(capabilities);
    let mut state = load_state(store.config_dir())?;
    let mut changed = Vec::new();

    for entry in FAMILY.iter().filter(|entry| seeds.contains(&entry.seed)) {
        match seed_file(&listing, entry.seed, true) {
            None => {
                install(store, &listing, entry, now)?;
                changed.push(entry.seed);
            }
            Some((_, hook)) if entry.matches(hook) => {}
            Some((view, hook)) => {
                store.rewrite_approved(&updated(entry, hook, now), &view.sha256)?;
                changed.push(entry.seed);
            }
        }

        state.record(entry.seed);
    }

    save_state(store.config_dir(), &state)?;
    Ok(changed)
}

/// Adds `entry` approved: over its pending file when there is one, else as a new file.
fn install(store: &HookStore, listing: &HookListing, entry: &FamilyHook, now: SystemTime) -> Result<()> {
    match seed_file(listing, entry.seed, false) {
        Some((view, hook)) => store.rewrite_approved(&updated(entry, hook, now), &view.sha256),
        None => store.create_approved(&entry.new_hook(now)),
    }
    .map(drop)
}

/// `hook` brought to the current version of `entry` (name, description, event, action, the
/// member as its requirement), keeping its id, creation, scope and `enabled`.
fn updated(entry: &FamilyHook, hook: &Hook, now: SystemTime) -> Hook {
    Hook {
        scope: hook.scope,
        project: hook.project.clone(),
        enabled: hook.enabled,
        created_at: hook.created_at.clone(),
        created_by: hook.created_by,
        ..entry.hook_with_id(hook.id.clone(), now)
    }
}

/// After `member` was installed: adds its family hooks that are missing, even ones the user
/// deleted before. Never overwrites an existing one. Returns the seeds added.
pub fn restore_member(store: &HookStore, member: HookRequirement, now: SystemTime) -> Result<Vec<HookSeed>> {
    let seeds: Vec<HookSeed> = FAMILY
        .iter()
        .filter(|entry| entry.member == member)
        .map(|entry| entry.seed)
        .collect();

    let listing = store.list(HookCapabilities::default());
    let missing: Vec<HookSeed> = seeds
        .into_iter()
        .filter(|seed| seeded_in(&listing, *seed).is_none())
        .collect();

    if missing.is_empty() {
        return Ok(Vec::new());
    }

    apply_family(store, &missing, HookCapabilities::default(), now)
}
