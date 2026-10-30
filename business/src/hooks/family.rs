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
//! - `code-graph-first` also needs the code graph of Harness Memory (the capability
//!   `code_graph_available`: the installed server lists `code-index` and the Code index switch is
//!   on). Without it launch seeding and [`restore_member`] skip it (it stays unrecorded, so it is
//!   seeded once the code graph is there: at a launch, after the switch or after Harness Memory
//!   was updated), the dialog shows it as needing the member, and a file already in the folder
//!   is paused.
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
pub const SEED_MEMORY_SEARCH_TEXT: &str = "Before you start, search Harness Memory for notes about this task with a few English keywords \
     (note_search) and check the saved scripts (script_list). Reuse what fits. Pass project_path on every \
     Harness Memory call: the absolute path of the repository you are working in (its git root), so nothing \
     is read from or saved to another repository.";

/// The `memory-save` follow-up.
pub const SEED_MEMORY_SAVE_TEXT: &str = "Most turns need no note: reply \"nothing to save\" unless this turn produced knowledge that a future \
     agent or a teammate could not get from the code or git history, such as the root cause of a \
     non-obvious bug and its fix, a decision with the reason behind it, a constraint or gotcha, or where \
     something non-obvious lives. Never save routine work: installing or configuring tools or MCP \
     servers, running commands, editing files, progress or status, or anything this turn merely did. \
     Before saving, search with note_search and update the existing note (note_update, by id) instead of \
     adding a near duplicate. Write a specific title and a body that stands alone for a reader with no \
     context: what, why, and the repo-relative paths involved; no transcripts or step lists. Save as many \
     notes as clear this bar and none otherwise: one focused note per finding, repository-specific \
     knowledge in the repository scope and knowledge useful across projects in the global scope. Write \
     each note in English and add 3 to 5 lowercase tags, including synonyms someone might search for. Save a \
     command sequence as a script (script_save) only when you have run it more than once and it will \
     recur. Always pass project_path (the absolute path of the repository you are working in, its git \
     root) to note_save, note_update and script_save: without it a note can land in the wrong repository. \
     Reply briefly.";

/// The `dashboard-limits` context.
pub const SEED_DASHBOARD_LIMITS_TEXT: &str = "Before a long or expensive task, check the plan limits and budgets \
     with Harness Dashboard (limits, budget_status). Say so if a limit is close.";

/// The `useful-skills` context.
pub const SEED_USEFUL_SKILLS_TEXT: &str = "If Useful Skills are installed, read the ones that apply to this \
     message before you act. When it starts a new task, even late in the session, load the workflow skill and \
     decide the workflow first (plan, implement, review) before the first edit. Then read any other skill that \
     fits and follow its limits (for example the 500-line file maximum). Do not skip a matching skill.";

/// The `branch-sync` context.
pub const SEED_BRANCH_SYNC_TEXT: &str = "At the start of the session, compare the branch you are working on with \
     the repository's main branch: main, master or prod, whichever exists (git symbolic-ref \
     refs/remotes/origin/HEAD names the default). Run git fetch, find which of the two has the most recent \
     commits, and list the commits of the main branch your branch does not contain (git log \
     HEAD..origin/<main> --oneline). If there are any, pull them into your branch unless the work tree is dirty \
     or the merge would conflict, then tell me what came in and whether it touches the files in the scope of \
     this work. Never discard local changes or rewrite history; ask me first when unsure.";

/// The `memory-first` context.
pub const SEED_MEMORY_FIRST_TEXT: &str = "Before you act on this message, search Harness Memory for notes about it \
     with a few English keywords (note_search) and check the saved scripts (script_list). Reuse what fits, \
     and say so if a note is outdated. Pass project_path on every Harness Memory call: the absolute path of \
     the repository you are working in (its git root), so nothing is read from or saved to another \
     repository. If the memory settings say saving is off, still search; just do not save.";

/// The `code-graph-first` context.
pub const SEED_CODE_GRAPH_FIRST_TEXT: &str = "Before your first tool call, decide which Harness Memory code tool answers the question and call it \
     first: code_map for an overview of the repository or an area, code_symbol for where a name is \
     defined and used, code_impact for what a change affects, code_path for how A reaches B. Pass \
     project_path, the repository root. grep, find and file reads come after, for literal text and for \
     details the graph does not show. A small repository is no reason to skip them: one code_map call \
     costs less than listing and reading files. A question about a flow (how a request, an event or data \
     gets from one part to another) is answered by code_path from the entry point to the part it ends in, \
     after code_map named both: do not reconstruct the flow by reading files one by one. If the code \
     tools are listed as deferred tools, load them with ToolSearch first. If they are not available in \
     this chat, say so once and go on with the usual tools.";

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
    /// The texts of its earlier catalog versions. A file still holding one of them word for word
    /// is stale, not edited, and automatic updates may replace it.
    pub earlier: &'static [&'static str],
    /// The events of its earlier catalog versions, when one fired on another event.
    pub earlier_events: &'static [HookEvent],
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

    /// True when `hook` is an earlier catalog version of this entry, word for word.
    fn is_earlier(&self, hook: &Hook) -> bool {
        let text = match &hook.action {
            HookAction::AddContext { text } | HookAction::FollowUp { text } => text.as_str(),
            HookAction::Command { .. } => return false,
        };

        let event = hook.event == self.event || self.earlier_events.contains(&hook.event);

        hook.name == self.name && event && self.earlier.contains(&text)
    }

    /// True when a machine with `capabilities` can use this entry: its member, and the code graph
    /// when its seed needs it.
    fn offered_by(&self, capabilities: HookCapabilities) -> bool {
        capabilities.meets(self.member) && (capabilities.code_graph_available || !self.seed.needs_code_graph())
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
pub const FAMILY: [FamilyHook; 7] = [
    FamilyHook {
        seed: HookSeed::MemorySearch,
        member: HookRequirement::HarnessMemory,
        name: "Search Harness Memory first",
        description: "Seeded default. Asks the agent to look up notes and scripts before it starts.",
        event: HookEvent::SessionStart,
        text: SEED_MEMORY_SEARCH_TEXT,
        earlier: &[
            "Before you start, search Harness Memory for notes about this task \
     (note_search) and check the saved scripts (script_list). Reuse what fits. Pass project_path on every \
     Harness Memory call: the absolute path of the repository you are working in (its git root), so nothing \
     is read from or saved to another repository.",
            "Before you start, search Harness Memory for notes about this task (memory_search) and check \
                    the saved scripts (script_list). Reuse what fits.",
            "Before you start, search Harness Memory for notes about this task (note_search) and check the \
                    saved scripts (script_list). Reuse what fits.",
        ],
        earlier_events: &[],
    },
    FamilyHook {
        seed: HookSeed::MemorySave,
        member: HookRequirement::HarnessMemory,
        name: "Save what you learned",
        description: "Seeded default. Asks the agent to keep lessons and repeatable commands after a turn.",
        event: HookEvent::TurnEnd,
        text: SEED_MEMORY_SAVE_TEXT,
        earlier: &[
            "Most turns need no note: reply \"nothing to save\" unless this turn produced knowledge that a future agent or a teammate could not get from the code or git history, such as the root cause of a non-obvious bug and its fix, a decision with the reason behind it, a constraint or gotcha, or where something non-obvious lives. Never save routine work: installing or configuring tools or MCP servers, running commands, editing files, progress or status, or anything this turn merely did. Before saving, search with note_search and update the existing note (note_update, by id) instead of adding a near duplicate. Write a specific title and a body that stands alone for a reader with no context: what, why, and the repo-relative paths involved; no transcripts or step lists. Save as many notes as clear this bar and none otherwise: one focused note per finding, repository-specific knowledge in the repository scope and knowledge useful across projects in the global scope. Save a command sequence as a script (script_save) only when you have run it more than once and it will recur. Always pass project_path (the absolute path of the repository you are working in, its git root) to note_save, note_update and script_save: without it a note can land in the wrong repository. Reply briefly.",
            "Most turns need no note: reply \"nothing to save\" unless this turn produced knowledge that a future agent or a teammate could not get from the code or git history, such as the root cause of a non-obvious bug and its fix, a decision with the reason behind it, a constraint or gotcha, or where something non-obvious lives. Never save routine work: installing or configuring tools or MCP servers, running commands, editing files, progress or status, or anything this turn merely did. Before saving, search with note_search and update the existing note (note_update, by id) instead of adding a near duplicate. Write a specific title and a body that stands alone for a reader with no context: what, why, and the repo-relative paths involved; no transcripts or step lists. Save at most one note per turn. Save a command sequence as a script (script_save) only when you have run it more than once and it will recur. Always pass project_path (the absolute path of the repository you are working in, its git root) to note_save, note_update and script_save: without it a note can land in the wrong repository. Reply briefly.",
            "If this turn taught you something worth keeping, save it to Harness Memory (note_save or note_update). Save any command sequence you would run again as a script (script_save). Always pass project_path (the absolute path of the repository you are working in, its git root) to note_save, note_update and script_save: without it a note can land in the wrong repository. If note_save says a note with that title exists, call note_update with its id instead. Reply briefly, or say nothing needs saving.",
            "If this turn taught you something worth keeping, save it to Harness Memory (memory_remember \
                    or memory_update). Save any command sequence you would run again as a script (script_save). \
                    Reply briefly, or say nothing needs saving.",
            "If this turn taught you something worth keeping, save it to Harness Memory (note_save or \
                    note_update). Save any command sequence you would run again as a script (script_save). \
                    Reply briefly, or say nothing needs saving.",
        ],
        earlier_events: &[],
    },
    FamilyHook {
        seed: HookSeed::DashboardLimits,
        member: HookRequirement::HarnessDashboard,
        name: "Check limits before long work",
        description: "Seeded default. Asks the agent to check plan limits and budgets before a long task.",
        event: HookEvent::SessionStart,
        text: SEED_DASHBOARD_LIMITS_TEXT,
        earlier: &[],
        earlier_events: &[],
    },
    FamilyHook {
        seed: HookSeed::UsefulSkills,
        member: HookRequirement::HarnessMemory,
        name: "Check Useful Skills first",
        description: "Seeded default. Asks the agent, on every message, to read the installed Useful Skills and pick the workflow before it acts.",
        event: HookEvent::PromptSubmit,
        text: SEED_USEFUL_SKILLS_TEXT,
        earlier: &[
            "If Useful Skills are installed, read the ones that apply before you act: decide the workflow \
                    first (plan, implement, review), then read any other skill that fits the task and follow its \
                    limits (for example the 500-line file maximum). Do not skip a matching skill.",
        ],
        earlier_events: &[HookEvent::SessionStart],
    },
    FamilyHook {
        seed: HookSeed::BranchSync,
        member: HookRequirement::HarnessMemory,
        name: "Check the branch against main",
        description: "Seeded default. Asks the agent to compare the current branch with the main branch and bring in its newer commits.",
        event: HookEvent::SessionStart,
        text: SEED_BRANCH_SYNC_TEXT,
        earlier: &[],
        earlier_events: &[],
    },
    FamilyHook {
        seed: HookSeed::MemoryFirst,
        member: HookRequirement::HarnessMemory,
        name: "Check Memory first",
        description: "Seeded default. Asks the agent, on every message, to search Harness Memory notes and scripts before it acts.",
        event: HookEvent::PromptSubmit,
        text: SEED_MEMORY_FIRST_TEXT,
        earlier: &[],
        earlier_events: &[],
    },
    FamilyHook {
        seed: HookSeed::CodeGraphFirst,
        member: HookRequirement::HarnessMemory,
        name: "Use the code graph first",
        description: "Seeded default. Asks the agent, on every message, to answer code-structure questions with the Harness Memory code tools before grep.",
        event: HookEvent::PromptSubmit,
        text: SEED_CODE_GRAPH_FIRST_TEXT,
        earlier: &[
            "Use the Harness Memory code tools (code_map, code_symbol, code_impact, code_path) for code-structure \
            questions before grep, ls, find or reading files, also in a small repository, and pass project_path. \
            In Claude Code they may be listed as deferred tools: load them first with ToolSearch, query \
            \"select:mcp__harness-memory__code_map,mcp__harness-memory__code_symbol,mcp__harness- \
            memory__code_impact,mcp__harness-memory__code_path\". Use code_map for an unfamiliar area or an \
            overview, code_symbol for where something is defined and who uses it, code_impact before changing a \
            signature, and one code_path call to trace how A reaches B (a symbol, a file path, a command name or \
            plain words work as an endpoint). If you grep to find a starting point, switch to code_path from the \
            function you found. Use grep only for literal text, error messages, config values, or when a code \
            tool says the repository is not indexed. If the code tools are not available in this chat, say so \
            once and go on with the usual tools.",
            "If the Harness Memory code tools (code_map, code_symbol, code_impact, code_path) are available, use them \
            for code-structure questions before grep or reading files, and pass project_path. Use \
            code_map for an unfamiliar area, code_symbol for where something is defined and who uses it, \
            code_impact before changing a signature, and one code_path call to trace how A reaches B (a \
            symbol, a file path, a command name or plain words work as an endpoint). If you grep to find \
            a starting point, switch to code_path from the function you found. Use grep only for literal \
            text, error messages, config values, or when a code tool says the repository is not indexed.",
        ],
        earlier_events: &[],
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
/// Memory counts whatever its memory setting, the code graph whatever its switch).
pub fn family_status(listing: &HookListing, installed: HookCapabilities) -> Vec<HookFamilyEntry> {
    FAMILY
        .iter()
        .map(|entry| {
            let hook = seeded_in(listing, entry.seed);

            let status = match hook {
                _ if !entry.offered_by(installed) => HookFamilyStatus::NeedsMember,
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
                stale: status == HookFamilyStatus::Outdated && hook.is_some_and(|hook| entry.is_earlier(hook)),
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
        .filter(|entry| entry.offered_by(available) && !state.has(entry.seed))
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
        .filter(|entry| entry.member == member && (available.code_graph_available || !entry.seed.needs_code_graph()))
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
