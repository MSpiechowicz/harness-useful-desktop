//! Team routines in the Routines view: share a local routine or update its team copy
//! (`routines_share`), remove a team routine from the team (`routines_team_remove`) or every one
//! this user shared (`routines_team_remove_mine`), refresh the copies (`routines_team_refresh`),
//! copy one to the local routines (`routines_team_copy`) and turn one on or off on this machine
//! only (`routines_team_enable`). Approving a team routine is `routine_approve`. See
//! `crate::routines::team`.
//!
//! Only the user shares: agents have no command or tool for it. Sharing and removing run
//! `harness-memory routines put|delete`, which reach the team database; `unsupported` while
//! routines are not shared. Turning a team routine off here or removing it stops its run, and a
//! removed one's run records are deleted (see `RoutineRuntime::reconcile_team`).

use std::path::Path;

use harness_business::BusinessError;
use harness_business::ecosystem::memory_team_docs::{delete_mine_team_docs, delete_team_doc, project_key, put_shared};
use harness_business::model::{RoutineFilter, RoutineListing, RoutineView, TeamRoutines, TeamRoutinesRemoved};
use harness_business::routines::RoutineKind;
use harness_business::routines::team::{
    ProjectKey, TeamFolder, copy_team_routine, forget_team_routine, record_share, set_team_enabled, share_request,
    team_view,
};
use tauri::State;

use super::{finished, list, project_root};
use crate::error::CommandResult;
use crate::state::AppState;

/// Why a team routine is refused when its view on screen is not the current version.
const CHANGED: &str = "the team routine changed since it was shown";

/// Shares the local routine `id` at version `sha256` with the team (a project routine under its
/// repository key), or updates the team copy when the team already has it ("Update team copy").
/// Returns the team copy. `conflict` when the local file or the team copy changed, `forbidden` for
/// a repository without an `origin` remote, `invalidInput` for a pending or invalid routine.
#[tauri::command]
pub async fn routines_share(state: State<'_, AppState>, id: String, sha256: String) -> CommandResult<RoutineView> {
    share(&state, id, sha256).await
}

/// Removes team routine `id` (the cached version `sha256`) from the team: every machine drops it
/// on its next refresh. `conflict` when the team copy changed since.
#[tauri::command]
pub async fn routines_team_remove(state: State<'_, AppState>, id: String, sha256: String) -> CommandResult<()> {
    remove(&state, id, sha256).await
}

/// Removes every team routine whose author is the name Harness Memory records for this user
/// ("Remove all my team routines"). The author is a label anyone can write, so this cleans up
/// after oneself and is no permission.
#[tauri::command]
pub async fn routines_team_remove_mine(state: State<'_, AppState>) -> CommandResult<TeamRoutinesRemoved> {
    remove_mine(&state).await
}

/// Refreshes the team copies now (the roots in use and `project`, the active or a recent one),
/// then lists the routines as `routines_list` does without a filter.
#[tauri::command]
pub async fn routines_team_refresh(
    state: State<'_, AppState>,
    project: Option<String>,
) -> CommandResult<RoutineListing> {
    refresh(&state, project).await
}

/// Copies team routine `id` (version `sha256`) to the local routines as an approved routine with
/// the same id, which then runs instead of the team copy. Only a team routine that may run here
/// (trusted or approved) is copied, and one trusted but not approved only while the team database
/// still holds those bytes (`conflict` otherwise, as when the copy changed). A project routine
/// needs `project` (the active or a recent project of that repository). `invalidInput` when a
/// local routine has the id.
#[tauri::command]
pub async fn routines_team_copy(
    state: State<'_, AppState>,
    id: String,
    sha256: String,
    project: Option<String>,
) -> CommandResult<RoutineView> {
    copy(&state, id, sha256, project).await
}

/// Turns team routine `id` on or off on this machine only (never written to the team). A project
/// routine is turned on for one folder, `project` (the active or a recent project), whose
/// repository must be the routine's. Turning on is not approval.
#[tauri::command]
pub async fn routines_team_enable(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
    project: Option<String>,
) -> CommandResult<RoutineView> {
    enable(&state, id, enabled, project).await
}

pub(crate) async fn share(state: &AppState, id: String, sha256: String) -> CommandResult<RoutineView> {
    let team = state.routines.team();

    // A refresh round that listed before the put must not write over the copy it records.
    let round = team.hold_rounds().await;
    let (server, config) = team.sharing_server(&state.env).await?;

    let request = state
        .routines
        .read(move |store| share_request(store, &TeamFolder::of(store), &id, &sha256))
        .await?;

    // Later refreshes ask for a project routine's root too, so its team copy stays.
    if let Some(root) = &request.project {
        team.note_root(&state.env, root);
    }

    let shared = put_shared::<RoutineKind>(&state.env, &state.runner, &server, request).await?;
    let update = shared.update;

    let view = state
        .routines
        .write(move |store| {
            let folder = TeamFolder::of(store);

            record_share(store, &folder, &shared.row, &shared.local_sha256, shared.project())?;
            team_view(store, &folder, config.trust, &shared.row.id)
        })
        .await?;

    tracing::info!(routine = view.id, update, "routine shared with the team");
    drop(round);
    team.refresh_soon();

    finished(&state.routines, view).await
}

pub(crate) async fn remove(state: &AppState, id: String, sha256: String) -> CommandResult<()> {
    let team = state.routines.team();

    // A refresh round that listed before the delete must not bring the copy back.
    let round = team.hold_rounds().await;
    let (server, _) = team.sharing_server(&state.env).await?;

    let read_id = id.clone();
    let row = state
        .routines
        .read(move |store| TeamFolder::of(store).row(&read_id))
        .await?;

    if row.sha256 != sha256 {
        return Err(BusinessError::Conflict(CHANGED.to_owned()).into());
    }

    let deleted = delete_team_doc::<RoutineKind>(&state.env, &state.runner, &server, &id, Some(&sha256)).await?;

    forget(state, vec![id.clone()]).await?;

    tracing::info!(routine = id, deleted, "routine removed from the team");
    drop(round);
    team.refresh_soon();

    Ok(())
}

pub(crate) async fn remove_mine(state: &AppState) -> CommandResult<TeamRoutinesRemoved> {
    let team = state.routines.team();
    let round = team.hold_rounds().await;
    let (server, _) = team.sharing_server(&state.env).await?;

    let removed = delete_mine_team_docs::<RoutineKind>(&state.env, &state.runner, &server).await?;

    forget(state, removed.ids.clone()).await?;

    tracing::info!(
        deleted = removed.deleted,
        "every team routine of this user removed from the team"
    );
    drop(round);
    team.refresh_soon();

    Ok(TeamRoutinesRemoved {
        deleted: u32::try_from(removed.deleted).unwrap_or(u32::MAX),
        ids: removed.ids,
    })
}

/// Drops the team copies `ids` the team no longer has, with their approvals and state, their runs
/// going on and their run records.
async fn forget(state: &AppState, ids: Vec<String>) -> CommandResult<()> {
    state
        .routines
        .write(move |store| {
            let folder = TeamFolder::of(store);
            ids.iter().try_for_each(|id| forget_team_routine(store, &folder, id))
        })
        .await?;

    state.routines.reconcile_team().await;

    Ok(())
}

pub(crate) async fn refresh(state: &AppState, project: Option<String>) -> CommandResult<RoutineListing> {
    if let Some(project) = &project {
        let root = project_root(state, project.clone()).await?;
        state.routines.team().note_root(&state.env, &root);
    }

    crate::team_sync::refresh(&*state.routines).await;

    list(state, project, RoutineFilter::default()).await
}

pub(crate) async fn copy(
    state: &AppState,
    id: String,
    sha256: String,
    project: Option<String>,
) -> CommandResult<RoutineView> {
    let team = sharing(state)?;

    let project = match project {
        Some(project) => {
            let root = project_root(state, project).await?;
            let key = repo_key(state, &root).await?;

            Some((root, key.key))
        }
        None => None,
    };

    // A copy that may run on trust alone is what the team database holds, not what the file in
    // the team folder says.
    let read_id = id.clone();
    let trust = team.trust;
    let shown = state
        .routines
        .read(move |store| team_view(store, &TeamFolder::of(store), trust, &read_id))
        .await?;

    // The version asked for is the one confirmed and copied: any other is refused.
    if shown.sha256 != sha256 {
        return Err(BusinessError::Conflict(CHANGED.to_owned()).into());
    }

    if let Err(reason) = state.routines.confirm_team_copy(&shown).await {
        return Err(BusinessError::Conflict(reason.to_owned()).into());
    }

    let copied_id = id.clone();

    let view = state
        .routines
        .write(move |store| {
            let project = project.as_ref().map(|(root, key)| (root.as_str(), key.as_str()));

            copy_team_routine(store, &TeamFolder::of(store), team.trust, &id, &sha256, project)
        })
        .await?;

    tracing::info!(routine = copied_id, "team routine copied to the local routines");

    finished(&state.routines, view).await
}

pub(crate) async fn enable(
    state: &AppState,
    id: String,
    enabled: bool,
    project: Option<String>,
) -> CommandResult<RoutineView> {
    let team = sharing(state)?;

    let folder = match project.filter(|_| enabled) {
        Some(project) => {
            let root = project_root(state, project).await?;
            let key = repo_key(state, &root).await?;
            state.routines.team().note_root(&state.env, &root);

            Some((root, key))
        }
        None => None,
    };

    let changed_id = id.clone();

    let view = state
        .routines
        .write(move |store| {
            let team_folder = TeamFolder::of(store);
            let chosen = folder.as_ref().map(|(root, key)| (root.as_str(), key));

            set_team_enabled(store, &team_folder, &id, enabled, chosen)?;
            team_view(store, &team_folder, team.trust, &id)
        })
        .await?;

    tracing::info!(
        routine = changed_id,
        enabled,
        "team routine turned on or off on this machine"
    );

    if !enabled {
        state.routines.reconcile_team().await;
    }

    finished(&state.routines, view).await
}

/// The team routines' state; `unsupported` while routines are not shared.
fn sharing(state: &AppState) -> CommandResult<TeamRoutines> {
    state
        .routines
        .team_settings()
        .ok_or_else(|| BusinessError::Unsupported("sharing routines with the team is off".to_owned()).into())
}

/// The repository key Harness Memory gives the project root `root` now (a folder a team routine
/// is turned on for or copied to is checked against the server, not an earlier answer).
async fn repo_key(state: &AppState, root: &str) -> CommandResult<ProjectKey> {
    let (server, _) = state.routines.team().sharing_server(&state.env).await?;

    Ok(project_key::<RoutineKind>(&state.env, &state.runner, &server, Path::new(root)).await?)
}
