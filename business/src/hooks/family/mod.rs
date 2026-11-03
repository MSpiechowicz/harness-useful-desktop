//! Harness family hooks: a catalog of hooks for the family members agents use over MCP (Harness
//! Memory, Harness Dashboard, Harness Knowledge, Harness Code, Useful Skills), each marked by its
//! `seed`.
//!
//! - Steps: most hooks are numbered steps (`FamilyHook::step`) that the composer puts in front of
//!   the message in a fixed order ([`super::steps`]). The position comes from the file's `seed`, so
//!   an edited file still composes as its step.
//! - Launch seeding ([`seed_defaults`]): a member's hooks are created once while it is
//!   available, global, `requires` the member, written and approved by the desktop. Each seed is
//!   recorded in `<app_config_dir>/hooks-state.json` and never created at launch again, even after
//!   the user deletes it. A seed hook already in the folder (a lost record) is recorded, not
//!   duplicated.
//! - The Harness family hooks dialog ([`family_status`], [`apply_family`]) adds missing hooks and
//!   updates outdated ones on the user's request.
//! - After a member is installed, [`restore_member`] adds its missing hooks again (never
//!   overwriting one that exists).
//! - Updates: a file that still holds a catalog version word for word, under the entry's name or an
//!   earlier one and on its event or an earlier one, is stale: the automatic update rewrites it in
//!   place (same id, `enabled` kept). A file the user edited is outdated but never rewritten
//!   without the user's request, and keeps composing as its step.
//! - Retired hooks (`FamilyHook::retired`, merged into another step) are never seeded or restored.
//!   An untouched file of one is stale: applying its seed deletes it. An edited one is left alone
//!   and runs as an ordinary hook, without a step.
//! - `code-graph-first` is Harness Code's: its member is available when the code graph is (the
//!   capability `code_graph_available`: the installed server lists `code-index`, the code graph
//!   setting and the Code index switch are on). Without it launch seeding and [`restore_member`]
//!   skip it (it stays unrecorded, so it is seeded once the code graph is there: at a launch, after
//!   the switch or after Harness Code was updated), the dialog shows it as needing the member, and
//!   a file already in the folder is paused. A file from before the graph left Harness Memory
//!   still requires `harness-memory` and holds an earlier text: it pauses with the graph too, and
//!   the automatic update (or the dialog) brings it to Harness Code.
//! - `useful-skills` is Useful Skills': available when they are installed for a harness (the
//!   capability `skills_available`), and in a chat only while that chat's own harness has them.
//! - The Harness Knowledge hook needs the member (installed, its setting on, `mcp` listed). The app
//!   runs the recall for it (see [`super::recall`]).
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

mod catalog;
mod earlier;
mod stale;

pub use catalog::FAMILY;
pub use stale::update_stale_family;

/// What a step of the numbered list is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// Its text alone.
    Rule,
    /// The plan limits the app looked up, then its text. No step without them.
    Limits,
    /// How the branch stands against main (the app looked it up), then its text. No step without it.
    Branch,
    /// Its text, then the notes the app found for the message.
    Memory,
    /// Its text, then the documents the app found for the message.
    Knowledge,
}

/// A hook's place in the numbered steps of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FamilyStep {
    /// Steps are ordered by this (10, 20, ...) and numbered from 1 per message.
    pub position: u8,
    /// What the step holds.
    pub kind: StepKind,
}

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
    /// The text it adds (`sessionStart`, `promptSubmit`: context) or sends (`turnEnd`: a follow-up).
    pub text: &'static str,
    /// Its place in the numbered steps, `None` when it is not one.
    pub step: Option<FamilyStep>,
    /// Merged into another hook: never seeded, and an untouched file of it is deleted.
    pub retired: bool,
    /// The texts of its earlier catalog versions, newest first. A file still holding one of them
    /// word for word is stale, not edited, and automatic updates may replace it.
    pub earlier: &'static [&'static str],
    /// The events of its earlier catalog versions, when one fired on another event.
    pub earlier_events: &'static [HookEvent],
    /// The names of its earlier catalog versions, when one had another name.
    pub earlier_names: &'static [&'static str],
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

    /// True when `hook` holds a catalog version of this entry word for word (the current text, or
    /// an earlier one), under its name or an earlier one, on its event or an earlier one, and has
    /// the catalog's shape ([`has_catalog_shape`]).
    fn is_earlier(&self, hook: &Hook) -> bool {
        if !has_catalog_shape(hook) {
            return false;
        }

        let text = match &hook.action {
            HookAction::AddContext { text } | HookAction::FollowUp { text } => text.as_str(),
            HookAction::Command { .. } => return false,
        };

        let event = hook.event == self.event || self.earlier_events.contains(&hook.event);

        event && self.is_version(&hook.name, text)
    }

    /// True when a hook named `name` with `text` holds a catalog version of this entry word for
    /// word, whatever its event (a part of a prompt knows no event).
    pub(crate) fn is_version(&self, name: &str, text: &str) -> bool {
        let name = name == self.name || self.earlier_names.contains(&name);

        name && (text == self.text || self.earlier.contains(&text))
    }

    /// True when `hook` is a catalog version of this entry, current or earlier, with the catalog's
    /// shape: not an edit, so an automatic update may rewrite or delete it.
    fn is_untouched(&self, hook: &Hook) -> bool {
        has_catalog_shape(hook) && (self.matches(hook) || self.is_earlier(hook))
    }

    /// True when a machine with `capabilities` can use this entry: its member is there.
    fn offered_by(&self, capabilities: HookCapabilities) -> bool {
        capabilities.meets(self.member)
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

/// True when `hook` has the parts of the catalog's shape that a user can change: global scope, no
/// project, no matcher. The description and requirements differ between catalog versions, so they
/// are not compared.
pub(crate) fn has_catalog_shape(hook: &Hook) -> bool {
    hook.scope == HookScope::Global && hook.project.is_none() && hook.matcher.is_none()
}

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
/// Memory counts whatever its memory setting, the code graph whatever its switch). A retired entry
/// is listed only while an untouched file of it is left (`retired` and `stale`: applying it
/// deletes the file). An edited file of one is an ordinary hook, not the catalog's.
pub fn family_status(listing: &HookListing, installed: HookCapabilities) -> Vec<HookFamilyEntry> {
    FAMILY
        .iter()
        .filter_map(|entry| {
            let hook = seeded_in(listing, entry.seed);

            if entry.retired {
                return hook
                    .filter(|hook| entry.is_untouched(hook))
                    .map(|hook| entry_of(entry, HookFamilyStatus::Outdated, Some(hook), true));
            }

            let status = match hook {
                _ if !entry.offered_by(installed) => HookFamilyStatus::NeedsMember,
                None => HookFamilyStatus::Missing,
                Some(hook) if entry.matches(hook) => HookFamilyStatus::Installed,
                Some(_) => HookFamilyStatus::Outdated,
            };

            let stale = status == HookFamilyStatus::Outdated && hook.is_some_and(|hook| entry.is_earlier(hook));

            Some(entry_of(entry, status, hook, stale))
        })
        .collect()
}

fn entry_of(entry: &FamilyHook, status: HookFamilyStatus, hook: Option<&Hook>, stale: bool) -> HookFamilyEntry {
    HookFamilyEntry {
        seed: entry.seed,
        member: entry.member,
        name: entry.name.to_owned(),
        event: entry.event,
        status,
        hook_id: hook.map(|hook| hook.id.clone()),
        stale,
        step: entry.step.map(|step| step.position),
        retired: entry.retired,
    }
}

/// Launch seeding: creates the hooks of every member `available` (Harness Memory: installed and
/// its setting on) that were never seeded. Returns the seeds created now.
pub fn seed_defaults(store: &HookStore, available: HookCapabilities, now: SystemTime) -> Result<Vec<HookSeed>> {
    let mut state = load_state(store.config_dir())?;
    let due: Vec<&FamilyHook> = FAMILY
        .iter()
        .filter(|entry| !entry.retired && entry.offered_by(available) && !state.has(entry.seed))
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

/// The user's request in the dialog: adds the missing hooks of `seeds`, brings outdated ones to
/// the current catalog version (name, description, event, action, requirement; the file keeps its
/// id and its `enabled`), as desktop writes (approved), and deletes the untouched files of retired
/// seeds. The member need not be installed: its hook then waits for it. Returns the seeds changed.
pub fn apply_family(
    store: &HookStore,
    seeds: &[HookSeed],
    capabilities: HookCapabilities,
    now: SystemTime,
) -> Result<Vec<HookSeed>> {
    apply_listed(store, &store.list(capabilities), seeds, Switch::Keep, now)
}

/// What a rewrite does with a hook's on/off switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Switch {
    /// The user's switch stays (an update the user asked for in the dialog).
    Keep,
    /// The hook is switched on (the automatic family update: every migrated family hook runs).
    On,
}

/// [`apply_family`] against `listing`, the files as they were read: every rewrite and delete is
/// guarded by the sha of that listing.
fn apply_listed(
    store: &HookStore,
    listing: &HookListing,
    seeds: &[HookSeed],
    switch: Switch,
    now: SystemTime,
) -> Result<Vec<HookSeed>> {
    let mut state = load_state(store.config_dir())?;
    let mut changed = Vec::new();

    for entry in FAMILY.iter().filter(|entry| seeds.contains(&entry.seed)) {
        if entry.retired {
            if remove_retired(store, listing, entry)? {
                changed.push(entry.seed);
            }

            continue;
        }

        match seed_file(listing, entry.seed, true) {
            None => {
                install(store, listing, entry, now)?;
                changed.push(entry.seed);
            }
            Some((_, hook)) if entry.matches(hook) => {}
            Some((view, hook)) => {
                store.rewrite_approved(&updated(entry, hook, switch, now), &view.sha256)?;
                changed.push(entry.seed);
            }
        }

        state.record(entry.seed);
    }

    save_state(store.config_dir(), &state)?;
    Ok(changed)
}

/// Deletes the approved files of the retired `entry` that hold a catalog version word for word.
/// An edited file stays. True when a file was deleted.
fn remove_retired(store: &HookStore, listing: &HookListing, entry: &FamilyHook) -> Result<bool> {
    let mut removed = false;

    for view in &listing.hooks {
        let Some(hook) = view.hook.as_ref() else {
            continue;
        };

        let untouched =
            hook.seed == Some(entry.seed) && view.approval == HookApproval::Approved && entry.is_untouched(hook);

        if untouched {
            store.delete(&view.id, HookAuthor::Desktop, Some(&view.sha256))?;
            removed = true;
        }
    }

    Ok(removed)
}

/// Adds `entry` approved: over its pending file when there is one, else as a new file.
fn install(store: &HookStore, listing: &HookListing, entry: &FamilyHook, now: SystemTime) -> Result<()> {
    match seed_file(listing, entry.seed, false) {
        Some((view, hook)) => store.rewrite_approved(&updated(entry, hook, Switch::Keep, now), &view.sha256),
        None => store.create_approved(&entry.new_hook(now)),
    }
    .map(drop)
}

/// `hook` brought to the current version of `entry` (name, description, event, action, the
/// member as its requirement), keeping its id, creation and scope, and its `enabled` unless `switch` turns it on.
fn updated(entry: &FamilyHook, hook: &Hook, switch: Switch, now: SystemTime) -> Hook {
    Hook {
        scope: hook.scope,
        project: hook.project.clone(),
        enabled: hook.enabled || switch == Switch::On,
        created_at: hook.created_at.clone(),
        created_by: hook.created_by,
        ..entry.hook_with_id(hook.id.clone(), now)
    }
}

/// After `member` was installed: adds its family hooks that are missing, even ones the user
/// deleted before. Never overwrites an existing one. A hook that needs the code graph is added
/// only when `available` has it. Returns the seeds added.
pub fn restore_member(
    store: &HookStore,
    member: HookRequirement,
    available: HookCapabilities,
    now: SystemTime,
) -> Result<Vec<HookSeed>> {
    let seeds: Vec<HookSeed> = FAMILY
        .iter()
        .filter(|entry| {
            !entry.retired
                && entry.member == member
                && (available.code_graph_available || !entry.seed.needs_code_graph())
        })
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
