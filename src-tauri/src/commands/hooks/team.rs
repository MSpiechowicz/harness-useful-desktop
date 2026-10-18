//! Team hooks in the Hooks view: share a local hook (`hooks_share`), remove a team hook from the
//! team (`hooks_team_remove`), refresh the copies (`hooks_team_refresh`), copy one to the local
//! folder (`hooks_team_copy`) and turn one off on this machine only (`hooks_team_enable`).
//! Approving a team hook is `hook_approve`. See `crate::hooks::team`.
//!
//! Only the user shares: agents have no command or tool for it. Sharing and removing run
//! `harness-memory hooks put|delete`, which reach the team database; `unsupported` while hooks are
//! not shared.

use std::path::Path;

use harness_business::BusinessError;
use harness_business::ecosystem::memory_hooks::{delete_team_hook, project_key, put_team_hook};
use harness_business::hooks::team::{
    ProjectKey, TeamFolder, copy_team_hook, forget_team_hook, record_share, set_team_enabled, share_request, team_view,
};
use harness_business::model::{HookListing, HookView, TeamHooks};
use tauri::State;

use super::{listing, project_root};
use crate::error::CommandResult;
use crate::state::AppState;

/// Shares the local hook `id` at version `sha256` with the team (a project hook under its
/// repository key), or updates the team copy when the team already has it. Returns the team
/// copy. `conflict` when the local file or the team copy changed, `forbidden` for a Harness family
/// hook or a repository without an `origin` remote, `invalidInput` for a pending or invalid hook.
#[tauri::command]
pub async fn hooks_share(state: State<'_, AppState>, id: String, sha256: String) -> CommandResult<HookView> {
    share(&state, id, sha256).await
}

/// Removes team hook `id` (the cached version `sha256`) from the team: every machine drops it on
/// its next refresh. `conflict` when the team copy changed since.
#[tauri::command]
pub async fn hooks_team_remove(state: State<'_, AppState>, id: String, sha256: String) -> CommandResult<()> {
    remove(&state, id, sha256).await
}

/// Refreshes the team copies now, then lists the hooks as `hooks_list` does.
#[tauri::command]
pub async fn hooks_team_refresh(state: State<'_, AppState>, project: Option<String>) -> CommandResult<HookListing> {
    refresh(&state, project).await
}

/// Copies team hook `id` (version `sha256`) to the local folder as an approved hook with the same
/// id, which then runs instead of the team copy. A project hook needs `project` (the active or a
/// recent project of that repository). `invalidInput` when a local hook has the id.
#[tauri::command]
pub async fn hooks_team_copy(
    state: State<'_, AppState>,
    id: String,
    sha256: String,
    project: Option<String>,
) -> CommandResult<HookView> {
    copy(&state, id, sha256, project).await
}

/// Turns team hook `id` on or off on this machine only (never written to the team).
#[tauri::command]
pub async fn hooks_team_enable(state: State<'_, AppState>, id: String, enabled: bool) -> CommandResult<HookView> {
    enable(&state, id, enabled).await
}

pub(crate) async fn share(state: &AppState, id: String, sha256: String) -> CommandResult<HookView> {
    // A refresh round that listed before the put must not write over the copy it records.
    let round = state.hooks.hold_team_rounds().await;
    let (server, config) = state.hooks.sharing_server().await?;

    let request = state
        .hooks
        .read(move |store| share_request(store, &TeamFolder::of(store), &id, &sha256))
        .await?;

    let key = match &request.project {
        Some(root) => {
            // Later refreshes ask for this root too, so its team copy stays.
            state.hooks.note_root(root);

            Some(project_key(&state.env, &state.runner, &server, Path::new(root)).await?)
        }
        None => None,
    };

    let body = request.body(key.as_ref())?;
    let update = request.team_sha256.is_some();

    let row = put_team_hook(
        &state.env,
        &state.runner,
        &server,
        &body,
        request.project.as_deref().map(Path::new),
        request.team_sha256.as_deref(),
    )
    .await?;

    let capabilities = state.hooks.capabilities().await;

    let view = state
        .hooks
        .write(move |store| {
            let folder = TeamFolder::of(store);
            let project = request.project.as_deref().zip(key.as_ref());

            record_share(store, &folder, &row, &request.local_sha256, project)?;
            team_view(store, &folder, capabilities, config.trust, &row.id)
        })
        .await?;

    tracing::info!(hook = view.id, update, "hook shared with the team");
    drop(round);
    state.hooks.refresh_team_soon();

    Ok(view)
}

pub(crate) async fn remove(state: &AppState, id: String, sha256: String) -> CommandResult<()> {
    // A refresh round that listed before the delete must not bring the copy back.
    let round = state.hooks.hold_team_rounds().await;
    let (server, _) = state.hooks.sharing_server().await?;

    let read_id = id.clone();
    let row = state
        .hooks
        .read(move |store| TeamFolder::of(store).row(&read_id))
        .await?;

    if row.sha256 != sha256 {
        return Err(BusinessError::Conflict("the team hook changed since it was shown".to_owned()).into());
    }

    let deleted = delete_team_hook(&state.env, &state.runner, &server, &id, Some(&sha256)).await?;

    let removed_id = id.clone();
    state
        .hooks
        .write(move |store| forget_team_hook(store, &TeamFolder::of(store), &removed_id))
        .await?;

    tracing::info!(hook = id, deleted, "hook removed from the team");
    drop(round);
    state.hooks.refresh_team_soon();

    Ok(())
}

pub(crate) async fn refresh(state: &AppState, project: Option<String>) -> CommandResult<HookListing> {
    let root = match project {
        Some(project) => Some(project_root(state, project).await?),
        None => None,
    };

    if let Some(root) = &root {
        state.hooks.note_root(root);
    }

    state.hooks.refresh_team().await;

    listing(state, root).await
}

pub(crate) async fn copy(
    state: &AppState,
    id: String,
    sha256: String,
    project: Option<String>,
) -> CommandResult<HookView> {
    let team = sharing(state)?;

    let project = match project {
        Some(project) => {
            let root = project_root(state, project).await?;
            let key = key_of(state, &root).await?;

            Some((root, key.key))
        }
        None => None,
    };

    let capabilities = state.hooks.capabilities().await;
    let copied_id = id.clone();

    let view = state
        .hooks
        .write(move |store| {
            let project = project.as_ref().map(|(root, key)| (root.as_str(), key.as_str()));

            copy_team_hook(
                store,
                &TeamFolder::of(store),
                capabilities,
                team.trust,
                &id,
                &sha256,
                project,
            )
        })
        .await?;

    tracing::info!(hook = copied_id, "team hook copied to the local hooks");

    Ok(view)
}

pub(crate) async fn enable(state: &AppState, id: String, enabled: bool) -> CommandResult<HookView> {
    let team = sharing(state)?;
    let capabilities = state.hooks.capabilities().await;
    let changed_id = id.clone();

    let view = state
        .hooks
        .write(move |store| {
            let folder = TeamFolder::of(store);

            set_team_enabled(store, &folder, &id, enabled)?;
            team_view(store, &folder, capabilities, team.trust, &id)
        })
        .await?;

    tracing::info!(hook = changed_id, enabled, "team hook turned on or off on this machine");

    Ok(view)
}

/// The team hooks' state; `unsupported` while hooks are not shared.
fn sharing(state: &AppState) -> CommandResult<TeamHooks> {
    state
        .hooks
        .team_settings()
        .ok_or_else(|| BusinessError::Unsupported("sharing hooks with the team is off".to_owned()).into())
}

/// The repository key of the project root `root`: from the last refresh, else asked now.
async fn key_of(state: &AppState, root: &str) -> CommandResult<ProjectKey> {
    let saved_root = root.to_owned();
    let saved = state
        .hooks
        .read(move |store| Ok(TeamFolder::of(store).projects().remove(&saved_root)))
        .await?;

    if let Some(key) = saved {
        return Ok(key);
    }

    let (server, _) = state.hooks.sharing_server().await?;

    Ok(project_key(&state.env, &state.runner, &server, Path::new(root)).await?)
}
