//! What the user does to a team hook on this machine, and the local halves of sharing and
//! removing one (the server half runs `harness-memory hooks put|delete`, see
//! [`crate::ecosystem::memory_hooks`]): [`crate::signed_docs::team::actions`] for hooks. App window
//! only, under its hooks write lock.

use super::views::team_view;
use super::{ProjectKey, TeamFolder, TeamRow};
use crate::error::{BusinessError, Result};
use crate::hooks::HookKind;
use crate::hooks::state::{HooksState, load_state, save_state};
use crate::hooks::store::{HookCapabilities, HookStore};
use crate::model::{HookStatus, HookView};
use crate::signed_docs::team::actions;

/// A local hook the user shares (see [`share_request`]).
pub type ShareRequest = actions::ShareRequest<HookKind>;

/// The local hook `id`, at version `sha256`, ready to share: approved, valid, not a Harness family
/// hook. `Conflict` when it changed since `sha256`, `NotFound` when there is no such file,
/// `InvalidInput` for an invalid or pending one, `Forbidden` for a family hook.
pub fn share_request(store: &HookStore, folder: &TeamFolder, id: &str, sha256: &str) -> Result<ShareRequest> {
    actions::share_request(store, folder, id, sha256)
}

/// After a share: keeps the `row` the server stored as the team copy, the repository `key` of the
/// project root `project`, and the local version shared (`sharedFrom`).
pub fn record_share(
    store: &HookStore,
    folder: &TeamFolder,
    row: &TeamRow,
    local_sha256: &str,
    project: Option<(&str, &ProjectKey)>,
) -> Result<()> {
    actions::record_share::<HookKind, HooksState>(store, folder, row, local_sha256, project)
}

/// After a removal from the team: drops the copy of team hook `id`, its local entries, and its
/// approval (see [`HookStore::forget_team_approvals`]).
pub fn forget_team_hook(store: &HookStore, folder: &TeamFolder, id: &str) -> Result<()> {
    actions::forget_team_doc::<HookKind, HooksState>(store, folder, id)
}

/// The user approves the team copy of `id` at version `sha256` (the same approvals as local hooks,
/// with a kept copy for the diff of a later version). `Conflict` when the copy changed since,
/// `NotFound` without a copy, `InvalidInput` for an invalid body or a duplicate (the local hook
/// with this id is the one that runs).
pub fn approve_team_hook(store: &HookStore, folder: &TeamFolder, id: &str, sha256: &str) -> Result<()> {
    store.approve_team_copy(folder, id, sha256)
}

/// Turns team hook `id` on or off on this machine (`teamDisabled`; never written to the team).
/// `NotFound` without a copy.
pub fn set_team_enabled(store: &HookStore, folder: &TeamFolder, id: &str, enabled: bool) -> Result<()> {
    if folder.read_one(id).is_none() {
        return Err(BusinessError::NotFound("the team hook".to_owned()));
    }

    let mut state = load_state(store.config_dir())?;
    state.set_team_disabled(id, !enabled);
    save_state(store.config_dir(), &state)
}

/// Copies the team hook `id` (version `sha256`) to the local folder as an approved hook with the
/// same id, which then wins over the team copy. Only a team hook that may run here is copied:
/// trusted (`trust`, "Trust team hooks") or approved on this machine, so a copy never approves what
/// the user has not. A project hook needs `project`, a project root whose repository key is `key`
/// and matches the hook's. Recorded as shared from the new file, so a later local edit offers
/// "Update team copy". `Conflict` when the copy changed, `NotFound` without one, `InvalidInput`
/// for a pending, invalid or duplicate team hook, without a matching project or when a local file
/// has the id. Nothing is written when it fails.
pub fn copy_team_hook(
    store: &HookStore,
    folder: &TeamFolder,
    capabilities: HookCapabilities,
    trust: bool,
    id: &str,
    sha256: &str,
    project: Option<(&str, &str)>,
) -> Result<HookView> {
    let may_copy = || {
        let status = team_view(store, folder, capabilities, trust, id)?.status;

        Ok(matches!(
            status,
            HookStatus::Active | HookStatus::Off | HookStatus::Paused
        ))
    };

    actions::copy_team_doc::<HookKind, HooksState>(store, folder, id, sha256, project, may_copy)?;

    store.get(id, capabilities)
}
