//! What the user does to a team hook on this machine, and the local halves of sharing and
//! removing one (the server half runs `harness-memory hooks put|delete`, see
//! [`crate::ecosystem::memory_hooks`]). App window only, under its hooks write lock.

use super::body::{local_copy, parse_team_hook, share_body};
use super::folder::{SyncCounts, TeamFolder};
use super::sync::TeamFetch;
use super::views::team_view;
use super::{ProjectKey, TeamRow};
use crate::error::{BusinessError, Result};
use crate::hooks::approvals::{Approvals, remove_snapshot};
use crate::hooks::folder::read_file;
use crate::hooks::sha256_hex;
use crate::hooks::state::{load_state, save_state};
use crate::hooks::store::{HookCapabilities, HookStore};
use crate::model::{HookApproval, HookScope, HookStatus, HookView};

/// A local hook the user shares (see [`share_request`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRequest {
    /// The hook id (the team copy keeps it).
    pub id: String,
    /// The SHA-256 of the local file shared.
    pub local_sha256: String,
    /// The project root of a project hook (`--project`), `None` for a global one.
    pub project: Option<String>,
    /// The SHA-256 of the team copy this machine has, when the team already has the hook: the
    /// share then updates it (`--expect-sha`).
    pub team_sha256: Option<String>,
    bytes: Vec<u8>,
}

impl ShareRequest {
    /// The body to send: the file, a project hook's `project` replaced by `key` (see
    /// [`share_body`]).
    pub fn body(&self, key: Option<&ProjectKey>) -> Result<Vec<u8>> {
        share_body(&self.id, &self.bytes, key)
    }
}

/// The local hook `id`, at version `sha256`, ready to share: approved, valid, not a Harness family
/// hook. `Conflict` when it changed since `sha256`, `NotFound` when there is no such file,
/// `InvalidInput` for an invalid or pending one, `Forbidden` for a family hook.
pub fn share_request(store: &HookStore, folder: &TeamFolder, id: &str, sha256: &str) -> Result<ShareRequest> {
    let view = store.get(id, HookCapabilities::default())?;

    if view.sha256 != sha256 {
        return Err(BusinessError::Conflict(
            "the hook changed since it was shown".to_owned(),
        ));
    }

    let Some(hook) = &view.hook else {
        return Err(BusinessError::InvalidInput("the hook breaks the format".to_owned()));
    };

    if view.status == HookStatus::Invalid {
        return Err(BusinessError::InvalidInput("the hook breaks the format".to_owned()));
    }

    if hook.seed.is_some() {
        return Err(BusinessError::Forbidden(
            "Harness family hooks are not shared, every machine adds its own".to_owned(),
        ));
    }

    if view.approval != HookApproval::Approved {
        return Err(BusinessError::InvalidInput(
            "the hook is waiting for review: approve it first".to_owned(),
        ));
    }

    let file = read_file(&store.file_path(id), id)
        .filter(|file| !file.unreadable && file.sha256 == sha256)
        .ok_or_else(|| BusinessError::Conflict("the hook changed since it was shown".to_owned()))?;

    let project = match hook.scope {
        HookScope::Global => None,
        HookScope::Project => hook.project.clone(),
    };

    Ok(ShareRequest {
        id: id.to_owned(),
        local_sha256: sha256.to_owned(),
        project,
        team_sha256: folder.row(id).ok().map(|row| row.sha256),
        bytes: file.bytes,
    })
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
    folder.write_row(row)?;

    if let Some((root, key)) = project {
        let mut projects = folder.projects();
        projects.insert(root.to_owned(), key.clone());
        folder.save_projects(&projects)?;
    }

    let mut state = load_state(store.config_dir())?;
    state.set_shared_from(&row.id, local_sha256);
    save_state(store.config_dir(), &state)
}

/// After a removal from the team: drops the copy of team hook `id`, its local entries, and its
/// approval (see [`forget_approvals`]).
pub fn forget_team_hook(store: &HookStore, folder: &TeamFolder, id: &str) -> Result<()> {
    folder.remove(id)?;

    let mut state = load_state(store.config_dir())?;
    state.forget_team(id);
    save_state(store.config_dir(), &state)?;

    forget_approvals(store, &[id.to_owned()])
}

/// Writes one refresh round ([`TeamFolder::apply`]) and drops the approvals of the copies it
/// removed (see [`forget_approvals`]).
pub fn apply_round(store: &HookStore, folder: &TeamFolder, fetch: &TeamFetch) -> Result<SyncCounts> {
    let counts = folder.apply(fetch)?;
    forget_approvals(store, &counts.removed)?;

    Ok(counts)
}

/// Deletes every team copy (sharing is off) and their approvals (see [`forget_approvals`]). True
/// when there was a team folder.
pub fn clear_team(store: &HookStore, folder: &TeamFolder) -> Result<bool> {
    let ids: Vec<String> = folder.ids().into_iter().collect();
    let cleared = folder.clear()?;

    forget_approvals(store, &ids)?;
    Ok(cleared)
}

/// Drops the approvals and approved copies of the team hooks `ids` that left this machine, so
/// the same bytes published again wait for approval. An id a local hook file has keeps them: that
/// approval belongs to the local file.
fn forget_approvals(store: &HookStore, ids: &[String]) -> Result<()> {
    let orphans: Vec<&String> = ids
        .iter()
        .filter(|id| std::fs::symlink_metadata(store.file_path(id)).is_err())
        .collect();

    if orphans.is_empty() {
        return Ok(());
    }

    let mut approvals = Approvals::load_for_update(store.config_dir())?;
    let mut changed = false;

    for id in orphans {
        changed |= approvals.remove(id);
        remove_snapshot(store.config_dir(), id);
    }

    if changed {
        approvals.save(store.config_dir())?;
    }

    Ok(())
}

/// The user approves the team copy of `id` at version `sha256` (the same approvals as local hooks,
/// with a kept copy for the diff of a later version). `Conflict` when the copy changed since,
/// `NotFound` without a copy, `InvalidInput` for an invalid body or a duplicate (the local hook
/// with this id is the one that runs).
pub fn approve_team_hook(store: &HookStore, folder: &TeamFolder, id: &str, sha256: &str) -> Result<()> {
    let row = folder.row(id)?;
    let bytes = row.body.as_bytes();

    if sha256_hex(bytes) != sha256 || row.sha256 != sha256 {
        return Err(BusinessError::Conflict(
            "the team hook changed since it was shown".to_owned(),
        ));
    }

    parse_team_hook(id, bytes).map_err(BusinessError::InvalidInput)?;

    if read_file(&store.file_path(id), id).is_some() {
        return Err(BusinessError::InvalidInput(
            "a local hook has this id, so it runs instead".to_owned(),
        ));
    }

    let approvals = Approvals::load_for_update(store.config_dir())?;
    store.record_approval(approvals, id, bytes).map(drop)
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
    let row = folder.row(id)?;

    if row.sha256 != sha256 || sha256_hex(row.body.as_bytes()) != sha256 {
        return Err(BusinessError::Conflict(
            "the team hook changed since it was shown".to_owned(),
        ));
    }

    if read_file(&store.file_path(id), id).is_some() {
        return Err(BusinessError::InvalidInput(
            "a local hook already has this id".to_owned(),
        ));
    }

    let status = team_view(store, folder, capabilities, trust, id)?.status;

    if !matches!(status, HookStatus::Active | HookStatus::Off | HookStatus::Paused) {
        return Err(BusinessError::InvalidInput(
            "approve the team hook before you copy it".to_owned(),
        ));
    }

    let team = parse_team_hook(id, row.body.as_bytes()).map_err(BusinessError::InvalidInput)?;

    let root = match (team.scope, project) {
        (HookScope::Global, _) => None,
        (HookScope::Project, Some((root, key))) if team.project.as_deref() == Some(key) => Some(root),
        (HookScope::Project, Some(_)) => {
            return Err(BusinessError::InvalidInput(
                "the hook belongs to another repository".to_owned(),
            ));
        }
        (HookScope::Project, None) => None,
    };

    let (hook, base) = local_copy(id, row.body.as_bytes(), root)?;
    let local_sha256 = store.create_approved_from(&hook, base)?;

    let mut state = load_state(store.config_dir())?;
    state.set_shared_from(id, &local_sha256);
    save_state(store.config_dir(), &state)?;

    store.get(id, capabilities)
}
