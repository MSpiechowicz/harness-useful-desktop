//! The Routines view: list the routine files with their status and next run (`routines_list`,
//! `routine_get`), create, edit, delete, approve and reject them, and hear about changes and runs
//! (`routines_listen`). Runs, "Run now", "Stop" and the schedule preview are in [`runs`], the chat
//! of an agent run and the folder of global runs in [`chat`], the team routines shared through
//! Harness Memory in [`team`]. See `crate::routines`.
//!
//! Every write is the app window's: what it saves is approved at once. A project is the active or
//! a recent one (`forbidden` otherwise); its routines are stored under its project root. Deleting
//! a routine checks the version first, then stops its run, takes no new run in until the delete
//! is over, and deletes its run records, schedule entry and workspace folder.

pub mod chat;
pub mod runs;
pub mod team;

use std::sync::Arc;
use std::time::SystemTime;

use harness_business::BusinessError;
use harness_business::model::{
    HookAuthor, HookScope, RoutineDraft, RoutineFilter, RoutineListing, RoutineView, RoutinesDeleted, RoutinesEvent,
};
use harness_business::routines::local_zone;
use harness_business::routines::team::{TeamFolder, app_listing, approve_team_routine, team_view};
use jiff::Timestamp;
use tauri::State;
use tauri::ipc::Channel;

use super::authorized_dir;
use super::hooks::project_root;
use crate::error::CommandResult;
use crate::routines::RoutineRuntime;
use crate::routines::job::StopReason;
use crate::routines::runner::STOP_BUDGET;
use crate::state::AppState;

/// The global routine files, and those of the project at `project` (the active or a recent one;
/// `None`: global only), then (while routines are shared) the team routines of that project and
/// every one turned on here, with their status and next run, kept by `filter` (`None`: all). The
/// counts are those of every listed routine.
#[tauri::command]
pub async fn routines_list(
    state: State<'_, AppState>,
    project: Option<String>,
    filter: Option<RoutineFilter>,
) -> CommandResult<RoutineListing> {
    list(&state, project, filter.unwrap_or_default()).await
}

/// The routine file `id` as `routines_list` shows it. `notFound` when it is gone.
#[tauri::command]
pub async fn routine_get(state: State<'_, AppState>, id: String) -> CommandResult<RoutineView> {
    get(&state, id).await
}

/// Creates (`draft.id` `None`) or edits a routine and approves the written version.
#[tauri::command]
pub async fn routine_save(state: State<'_, AppState>, draft: RoutineDraft) -> CommandResult<RoutineView> {
    save(&state, draft).await
}

/// Deletes the routine file `id` (the version `sha256`), its approval, run records and schedule
/// entry, after stopping its run. `conflict` when it changed.
#[tauri::command]
pub async fn routine_delete(state: State<'_, AppState>, id: String, sha256: String) -> CommandResult<()> {
    delete(&state, id, sha256).await
}

/// Deletes every routine `routines_list` shows for `project` (the active or a recent one; `None`:
/// global only), pending and invalid ones included, with their runs. Team routines stay.
#[tauri::command]
pub async fn routines_delete_all(
    state: State<'_, AppState>,
    project: Option<String>,
) -> CommandResult<RoutinesDeleted> {
    delete_all(&state, project).await
}

/// Approves the version `sha256` of routine `id`, a local file or (without one) a team routine.
/// `conflict` when it changed.
#[tauri::command]
pub async fn routine_approve(state: State<'_, AppState>, id: String, sha256: String) -> CommandResult<RoutineView> {
    approve(&state, id, sha256).await
}

/// Rejects the pending version `sha256` of routine `id`: a changed routine gets its approved
/// version back (returned), a never-approved one is deleted (`None`). `conflict` when it changed.
#[tauri::command]
pub async fn routine_reject(
    state: State<'_, AppState>,
    id: String,
    sha256: String,
) -> CommandResult<Option<RoutineView>> {
    reject(&state, id, sha256).await
}

/// Sends `changed` when the folder or the approvals change, and the runs as they start, end or
/// wait for an answer, to `on_event`, replacing the previous listener.
#[tauri::command]
pub async fn routines_listen(state: State<'_, AppState>, on_event: Channel<RoutinesEvent>) -> CommandResult<()> {
    state.routines.listen(on_event);

    Ok(())
}

pub(crate) async fn list(
    state: &AppState,
    project: Option<String>,
    filter: RoutineFilter,
) -> CommandResult<RoutineListing> {
    let root = match project {
        Some(project) => Some(project_root(state, project).await?),
        None => None,
    };

    let team = state.routines.team_settings();

    if let Some(root) = &root {
        state.routines.team().note_root(&state.env, root);
    }

    let verified = root
        .as_deref()
        .is_some_and(|root| state.routines.team().root_verified(root));

    let live = state.routines.live_routines();

    let listing = state
        .routines
        .read(move |store| {
            let mut listing = app_listing(store, &TeamFolder::of(store), root.as_deref(), team.as_ref(), verified);
            listing.finish(&live, Timestamp::now(), &local_zone());

            Ok(listing)
        })
        .await?;

    Ok(listing.filtered(&filter))
}

pub(crate) async fn get(state: &AppState, id: String) -> CommandResult<RoutineView> {
    let view = state.routines.read(move |store| store.get(&id)).await?;

    finished(&state.routines, view).await
}

/// `view` as a listing shows it: paused while its project folder is gone, with its running or
/// waiting run and its next run.
async fn finished(runtime: &RoutineRuntime, view: RoutineView) -> CommandResult<RoutineView> {
    let live = runtime.live_routines();

    let finished = runtime
        .read(move |_| {
            let mut listing = RoutineListing {
                routines: vec![view],
                folder_problem: None,
                team: None,
                counts: Default::default(),
            };

            listing.finish(&live, Timestamp::now(), &local_zone());

            Ok(listing.routines.remove(0))
        })
        .await?;

    Ok(finished)
}

pub(crate) async fn save(state: &AppState, mut draft: RoutineDraft) -> CommandResult<RoutineView> {
    if draft.scope == HookScope::Project {
        draft.project = project_folder(state, &draft).await?;
    }

    let saved = state
        .routines
        .write(move |store| store.save(&draft, HookAuthor::Desktop, SystemTime::now()))
        .await?;

    tracing::info!(routine = saved.id, status = ?saved.status, "routine saved");

    finished(&state.routines, saved).await
}

/// The project of a project draft: the root the edited file already names, else a folder of the
/// active or a recent project.
async fn project_folder(state: &AppState, draft: &RoutineDraft) -> CommandResult<Option<String>> {
    let Some(folder) = draft.project.clone().filter(|folder| !folder.is_empty()) else {
        return Ok(None);
    };

    let edited = draft.id.clone();
    let stored = match edited {
        Some(id) => {
            state
                .routines
                .read(move |store| Ok(store.get(&id).ok().and_then(|view| view.routine?.project)))
                .await?
        }
        None => None,
    };

    if stored.as_deref() == Some(folder.as_str()) {
        return Ok(Some(folder));
    }

    let dir = authorized_dir(state, folder).await?;

    Ok(Some(dir.to_string_lossy().into_owned()))
}

pub(crate) async fn delete(state: &AppState, id: String, sha256: String) -> CommandResult<()> {
    let checked = id.clone();
    let view = state.routines.read(move |store| store.get(&checked)).await?;

    // A stale version stops nothing.
    if view.sha256 != sha256 {
        return Err(BusinessError::Conflict("the routine changed since it was shown".to_owned()).into());
    }

    let ids = vec![id.clone()];
    let deleting = Deleting::begin(&state.routines, ids.clone());

    state.routines.stop_routines(&ids, StopReason::User, STOP_BUDGET).await;

    state
        .routines
        .write(move |store| store.delete(&id, HookAuthor::Desktop, Some(&sha256)))
        .await?;

    drop_runs(&state.routines, ids).await;
    drop(deleting);
    tracing::info!(routine = view.id, "routine deleted");

    Ok(())
}

/// Routines being deleted: no run of them is taken in until this is dropped (the delete ended,
/// or failed).
struct Deleting<'a> {
    runtime: &'a RoutineRuntime,
    ids: Vec<String>,
}

impl<'a> Deleting<'a> {
    fn begin(runtime: &'a RoutineRuntime, ids: Vec<String>) -> Self {
        runtime.begin_delete(&ids);

        Self { runtime, ids }
    }
}

impl Drop for Deleting<'_> {
    fn drop(&mut self) {
        self.runtime.end_delete(&self.ids);
    }
}

pub(crate) async fn delete_all(state: &AppState, project: Option<String>) -> CommandResult<RoutinesDeleted> {
    let root = match project {
        Some(project) => Some(project_root(state, project).await?),
        None => None,
    };

    let listed = root.clone();
    let shown: Vec<String> = state
        .routines
        .read(move |store| Ok(store.list_for(listed.as_deref()).routines))
        .await?
        .into_iter()
        .map(|view| view.id)
        .collect();

    let deleting = Deleting::begin(&state.routines, shown.clone());
    state
        .routines
        .stop_routines(&shown, StopReason::User, STOP_BUDGET)
        .await;

    let ids = state
        .routines
        .write(move |store| store.delete_all(root.as_deref()))
        .await?;

    let deleted = u32::try_from(ids.len()).unwrap_or(u32::MAX);
    drop_runs(&state.routines, ids).await;
    drop(deleting);
    tracing::info!(deleted, "every listed routine deleted");

    Ok(RoutinesDeleted { deleted })
}

pub(crate) async fn approve(state: &AppState, id: String, sha256: String) -> CommandResult<RoutineView> {
    let team = state.routines.team_settings();

    let approved = state
        .routines
        .write(move |store| match store.approve(&id, &sha256) {
            Ok(()) => store.get(&id),
            // No local file: a team routine, while routines are shared.
            Err(BusinessError::NotFound(missing)) => {
                let Some(team) = team else {
                    return Err(BusinessError::NotFound(missing));
                };

                let folder = TeamFolder::of(store);
                approve_team_routine(store, &folder, &id, &sha256)?;
                team_view(store, &folder, team.trust, &id)
            }
            Err(error) => Err(error),
        })
        .await?;

    tracing::info!(routine = approved.id, "routine approved");

    finished(&state.routines, approved).await
}

pub(crate) async fn reject(state: &AppState, id: String, sha256: String) -> CommandResult<Option<RoutineView>> {
    let rejected = id.clone();
    let restored = state
        .routines
        .write(move |store| store.reject(&rejected, &sha256))
        .await?;

    tracing::info!(routine = id, restored = restored.is_some(), "routine version rejected");

    match restored {
        Some(view) => Ok(Some(finished(&state.routines, view).await?)),
        None => {
            drop_runs(&state.routines, vec![id]).await;
            Ok(None)
        }
    }
}

/// Deletes the run records, schedule entries and workspace folders of the deleted routines `ids`.
async fn drop_runs(runtime: &Arc<RoutineRuntime>, ids: Vec<String>) {
    let deleted = ids.clone();
    let records = runtime
        .record(move |runs| deleted.iter().try_for_each(|id| runs.delete(id)))
        .await;

    if let Err(error) = records {
        tracing::warn!(code = ?error.code(), "routine run records not deleted");
    }

    runtime.forget_schedule(&ids).await;
    runtime.forget_workspaces_soon().await;
}
