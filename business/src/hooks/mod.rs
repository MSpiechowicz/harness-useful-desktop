//! Hooks: small JSON files in a shared folder that add context to a prompt, send a follow-up or
//! run a command when something happens in a desktop chat. `docs/hooks-format.md` is the file
//! contract. The app window runs hooks; its MCP server (`harness-desktop mcp`) writes hook files
//! for agents through the same [`HookStore`] but never runs them.
//!
//! Hooks are one kind of signed document ([`HookKind`]): the folder, the approvals and the team
//! copies are [`crate::signed_docs`]'s, this module adds what a hook is and does.
//!
//! - [`store`]: the folder ([`hooks_root`]), reading and validating the files ([`format`]),
//!   listing them with their status ([`HookCapabilities`]), and writes and deletes by the app
//!   window or an agent.
//! - [`approvals`]: `hook-approvals.json` and the approved copies in the app's config folder. A
//!   hook runs only while the SHA-256 of its exact file bytes is approved, so a file an agent
//!   wrote never runs unreviewed.
//! - [`family`]: the Harness family hooks (Harness Memory, Harness Dashboard): seeded once at
//!   launch, added or updated on request, restored after a member is installed.
//! - [`engine`]: a pure state machine per chat that decides which hooks fire for each action and
//!   event, including the follow-up loop rule. It never runs anything itself.
//! - [`command`]: runs the command hooks of one firing (sequential, a shared 30 s budget).
//! - [`compose`]: the `<harness-hook-context>` block and the follow-up prompt, and
//!   [`strip_hook_context`] for titles and previews.
//! - [`team`]: the team hooks shared through Harness Memory, kept as copies in `<root>/team` that
//!   only the app window reads (never [`HookStore::list`] or the MCP server).
//! - `hooks-state.json`: the seed record, the team hooks turned off on this machine and what was
//!   shared from which local version.
//!
//! Command hooks are the one place the app runs a shell (`sh -c`, `cmd.exe /D /S /C`): the
//! user wrote or approved that exact command text.

pub mod approvals;
mod cleanup;
pub mod command;
pub mod compose;
mod draft;
pub mod engine;
pub mod family;
mod folder;
pub mod format;
mod plan;
mod run;
mod state;
pub mod store;
pub mod team;
mod view;

use std::path::PathBuf;

use serde_json::{Map, Value};

pub use approvals::Approvals;
pub use command::{CommandFailure, CommandFailureKind, FIRING_BUDGET, FiringResult, run_firing, run_firings};
pub use compose::{
    CONTEXT_CLOSE, CONTEXT_OPEN, HookContextItem, MAX_COMPOSED_BYTES, ResolvedPart, compose_context, compose_follow_up,
    compose_prompt, parse_hook_context, strip_hook_context, with_hook_context,
};
pub use engine::{
    ChatContext, CommandRequest, EventOutcome, Firing, FollowUpPlan, HookSession, HookSet, PlannedPart, SendOrigin,
    SendPlan,
};
pub use family::{
    FAMILY, FamilyHook, SEED_DASHBOARD_LIMITS_TEXT, SEED_MEMORY_SAVE_TEXT, SEED_MEMORY_SEARCH_TEXT, apply_family,
    family_hook, family_status, restore_member, seed_defaults,
};
pub use format::{MAX_HOOK_FILE_BYTES, is_hook_id, parse_hook_file};
pub use plan::MAX_TOUCHED_BYTES;
pub use run::handled_without_run;
pub use store::{HookCapabilities, HookStore, MAX_HOOKS_PER_SCOPE};

pub use crate::signed_docs::{project_identity, sha256_hex};

use crate::env::Env;
use crate::model::Hook;
use crate::signed_docs::{DocKind, doc_root};

/// `HARNESS_HOOKS_DIR`: overrides the hooks root (absolute, a leading `~` expanded).
pub const DIR_VAR: &str = "HARNESS_HOOKS_DIR";

/// The folder under the platform base.
pub const APP_FOLDER: &str = "harness-hooks";

/// The folder inside the root that holds the hook files.
pub const HOOKS_FOLDER: &str = "hooks";

/// The approvals file in the app's config folder.
pub const APPROVALS_FILE: &str = "hook-approvals.json";

/// The seed record and the team hooks' local state in the app's config folder.
pub const STATE_FILE: &str = "hooks-state.json";

/// Hooks as a kind of signed document (see [`crate::signed_docs`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HookKind;

impl DocKind for HookKind {
    const NOUN: &'static str = "hook";
    const PLURAL: &'static str = HOOKS_FOLDER;
    const ROOT_VAR: &'static str = DIR_VAR;
    const APP_FOLDER: &'static str = APP_FOLDER;
    const MAX_FILE_BYTES: usize = MAX_HOOK_FILE_BYTES;
    const MAX_PER_SCOPE: usize = MAX_HOOKS_PER_SCOPE;
    const APPROVALS_FILE: &'static str = APPROVALS_FILE;
    const SNAPSHOT_FOLDER: &'static str = approvals::SNAPSHOT_FOLDER;
    const STATE_FILE: &'static str = STATE_FILE;
    const TRUST_KEY: &'static str = "trustTeamHooks";

    type Doc = Hook;

    fn parse(id: &str, bytes: &[u8]) -> Result<Hook, String> {
        parse_hook_file(id, bytes)
    }

    fn parse_team(id: &str, bytes: &[u8]) -> Result<Hook, String> {
        team::parse_team_hook(id, bytes)
    }

    fn to_bytes(hook: &Hook, base: Map<String, Value>) -> Vec<u8> {
        format::hook_bytes(hook, base)
    }

    fn not_shared(hook: &Hook) -> Option<String> {
        hook.seed
            .map(|_| "Harness family hooks are not shared, every machine adds its own".to_owned())
    }
}

/// The hooks root: [`DIR_VAR`] when it is absolute (after `~` expansion), else the platform data
/// folder followed by [`APP_FOLDER`] (`$XDG_DATA_HOME` or `~/.local/share`,
/// `~/Library/Application Support`, `%LOCALAPPDATA%`). `None` on Windows without `LOCALAPPDATA`:
/// hooks are then unavailable. Resolved like `memory_data_dir` ([`doc_root`]).
pub fn hooks_root(env: &Env) -> Option<PathBuf> {
    doc_root::<HookKind>(env)
}
