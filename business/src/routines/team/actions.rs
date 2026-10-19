//! What the user does to a team routine on this machine (turn it on or off, approve it, copy it to
//! the local folder) and the local halves of sharing and removing one: the generic team actions
//! of [`crate::signed_docs::team::actions`] with routines' catalogue rule. App window only, under
//! its routines write lock.

use std::path::Path;

use super::views::{may_run, team_view};
use super::{ProjectKey, TeamFolder, TeamRow};
use crate::error::{BusinessError, Result};
use crate::model::{HookScope, RoutineStatus, RoutineView};
use crate::routines::RoutineKind;
use crate::routines::format::parse_team_routine;
use crate::routines::state::{RoutinesState, load_state, save_state};
use crate::routines::store::RoutineStore;
use crate::signed_docs::project_identity;
use crate::signed_docs::team::actions;

/// A local routine the user shares (see [`share_request`]).
pub type ShareRequest = actions::ShareRequest<RoutineKind>;

/// The local routine `id`, at version `sha256`, ready to share: approved and valid. `Conflict`
/// when it changed since `sha256`, `NotFound` when there is no such file, `InvalidInput` for an
/// invalid or pending one.
pub fn share_request(store: &RoutineStore, folder: &TeamFolder, id: &str, sha256: &str) -> Result<ShareRequest> {
    actions::share_request::<RoutineKind>(store, folder, id, sha256)
}

/// After a share: keeps the `row` the server stored as the team copy, the repository `key` of the
/// project root `project`, and the local version shared (`sharedFrom`).
pub fn record_share(
    store: &RoutineStore,
    folder: &TeamFolder,
    row: &TeamRow,
    local_sha256: &str,
    project: Option<(&str, &ProjectKey)>,
) -> Result<()> {
    actions::record_share::<RoutineKind, RoutinesState>(store, folder, row, local_sha256, project)
}

/// After a removal from the team: drops the copy of team routine `id`, its `teamEnabled` and
/// `sharedFrom` entries, and its approval.
pub fn forget_team_routine(store: &RoutineStore, folder: &TeamFolder, id: &str) -> Result<()> {
    actions::forget_team_doc::<RoutineKind, RoutinesState>(store, folder, id)
}

/// The user approves the team copy of `id` at version `sha256` (the same approvals as local
/// routines, with a kept copy for the diff of a later version). Approval is not turning it on.
/// `Conflict` when the copy changed since, `NotFound` without a copy, `InvalidInput` for an
/// invalid body or a duplicate.
pub fn approve_team_routine(store: &RoutineStore, folder: &TeamFolder, id: &str, sha256: &str) -> Result<()> {
    store.approve_team_copy(folder, id, sha256)
}

/// Turns team routine `id` on or off on this machine (`teamEnabled`, never written to the team).
/// Turning on is not approval. A global routine needs no folder; a project routine runs in
/// `folder`, a project root (its own project identity) whose repository key `key` is the
/// routine's, recorded in `projects.json` too. `NotFound` without a copy, `InvalidInput` for an
/// invalid body, a project routine without a folder, or a folder of another repository.
pub fn set_team_enabled(
    store: &RoutineStore,
    folder: &TeamFolder,
    id: &str,
    enabled: bool,
    project: Option<(&str, &ProjectKey)>,
) -> Result<()> {
    let row = folder.row(id)?;

    let mut state = load_state(store.config_dir())?;

    if !enabled {
        state.set_team_enabled(id, None);
        return save_state(store.config_dir(), &state);
    }

    let routine = parse_team_routine(id, row.body.as_bytes()).map_err(BusinessError::InvalidInput)?;

    let on = match routine.scope {
        HookScope::Global => None,
        HookScope::Project => {
            let (root, key) = project.ok_or_else(|| {
                BusinessError::InvalidInput("choose the project folder the routine runs in".to_owned())
            })?;

            check_folder(root, key, routine.project.as_deref())?;

            let mut projects = folder.projects();
            projects.insert(root.to_owned(), key.clone());
            folder.save_projects(&projects)?;

            Some(root.to_owned())
        }
    };

    state.set_team_enabled(id, Some(on));
    save_state(store.config_dir(), &state)
}

/// `root` is a project root of the repository `repo`, as its key `key` says.
fn check_folder(root: &str, key: &ProjectKey, repo: Option<&str>) -> Result<()> {
    let identity = project_identity(Path::new(root))?;

    if identity != Path::new(root) {
        return Err(BusinessError::InvalidInput(
            "the folder is not the root of its project".to_owned(),
        ));
    }

    if Some(key.key.as_str()) != repo {
        return Err(BusinessError::InvalidInput(
            "the folder is not a copy of the routine's repository".to_owned(),
        ));
    }

    Ok(())
}

/// Copies the team routine `id` (version `sha256`) to the local folder as an approved routine
/// with the same id, which then wins over the team copy. Only a team routine that may run here is
/// copied: trusted (`trust`) or approved on this machine,
/// never a pending one. A project routine needs `project`, a local project root and its
/// repository key, which must be the routine's. `Conflict` when the copy changed, `NotFound`
/// without one, `InvalidInput` for a pending, invalid or duplicate team routine, without a
/// matching project or when a local file has the id. Nothing is written when it fails.
pub fn copy_team_routine(
    store: &RoutineStore,
    folder: &TeamFolder,
    trust: bool,
    id: &str,
    sha256: &str,
    project: Option<(&str, &str)>,
) -> Result<RoutineView> {
    let may_copy = || {
        let view = team_view(store, folder, trust, id)?;

        let copyable = match (&view.routine, view.status) {
            (_, RoutineStatus::Invalid | RoutineStatus::Duplicate) => false,
            (Some(_), _) => may_run(view.approval, trust),
            (None, _) => false,
        };

        Ok(copyable)
    };

    actions::copy_team_doc::<RoutineKind, RoutinesState>(store, folder, id, sha256, project, may_copy)?;

    store.get(id)
}
