//! Hooks: small JSON files in a shared folder that add context to a prompt, send a follow-up or
//! run a command when something happens in a desktop chat. `docs/hooks-format.md` is the file
//! contract. The app window runs hooks; its MCP server (`harness-desktop mcp`) writes hook files
//! for agents through the same [`HookStore`] but never runs them.
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
pub mod store;
mod unicode;
mod view;

use std::path::{Path, PathBuf};

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
pub use store::{HookCapabilities, HookStore, MAX_HOOKS_PER_SCOPE};

use sha2::{Digest, Sha256};

use crate::app_dirs::shared_data_dir;
use crate::env::Env;
use crate::error::{BusinessError, Result};

/// `HARNESS_HOOKS_DIR`: overrides the hooks root (absolute, a leading `~` expanded).
pub const DIR_VAR: &str = "HARNESS_HOOKS_DIR";

/// The folder under the platform base.
pub const APP_FOLDER: &str = "harness-hooks";

/// The folder inside the root that holds the hook files.
pub const HOOKS_FOLDER: &str = "hooks";

/// The approvals file in the app's config folder.
pub const APPROVALS_FILE: &str = "hook-approvals.json";

/// The seed record in the app's config folder.
pub const STATE_FILE: &str = "hooks-state.json";

/// The hooks root: [`DIR_VAR`] when it is absolute (after `~` expansion), else the platform data
/// folder followed by [`APP_FOLDER`] (`$XDG_DATA_HOME` or `~/.local/share`,
/// `~/Library/Application Support`, `%LOCALAPPDATA%`). `None` on Windows without `LOCALAPPDATA`:
/// hooks are then unavailable. Resolved like `memory_data_dir` ([`shared_data_dir`]).
pub fn hooks_root(env: &Env) -> Option<PathBuf> {
    shared_data_dir(env, DIR_VAR, APP_FOLDER)
}

/// The project root of `folder` (see "Project identity" in the format doc): its real path
/// (symbolic links resolved, no `\\?\` prefix on Windows), then the nearest ancestor-or-self that
/// contains `.git` (a folder or a file), else the folder itself. `NotFound` for a missing path,
/// `InvalidInput` for one that is not a folder.
pub fn project_identity(folder: &Path) -> Result<PathBuf> {
    let real = dunce::canonicalize(folder)
        .map_err(|error| BusinessError::from_io(format!("resolving {}", folder.display()), error))?;

    if !real.is_dir() {
        return Err(BusinessError::InvalidInput(format!(
            "{} is not a folder",
            folder.display()
        )));
    }

    let root = real
        .ancestors()
        .find(|dir| std::fs::metadata(dir.join(".git")).is_ok_and(|metadata| metadata.is_dir() || metadata.is_file()))
        .unwrap_or(&real)
        .to_path_buf();

    Ok(root)
}

/// Lowercase hex SHA-256 of `bytes` (a hook file's approval hash).
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
