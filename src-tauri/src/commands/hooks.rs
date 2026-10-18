//! The Hooks view: list the hook files with their status (`hooks_list`), create, edit, delete,
//! approve and reject them, the Harness family hooks (`hooks_family`, `hooks_family_apply`), and
//! the changes of the folder (`hooks_listen`). The team hooks shared through Harness Memory are in
//! [`team`]. See `crate::hooks`.
//!
//! Every write is the app window's: what it saves is approved at once. A project is the active or
//! a recent one (`forbidden` otherwise); its hooks are stored under its project root.

use std::time::SystemTime;

use harness_business::hooks::team::{TeamFolder, app_listing, approve_team_hook, team_view};
use harness_business::hooks::{apply_family, family_status, project_identity};
use harness_business::model::{
    HookAuthor, HookDraft, HookFamilyEntry, HookListing, HookScope, HookSeed, HookView, HooksDeleted, HooksEvent,
};
use harness_business::{BusinessError, Result};
use tauri::State;
use tauri::ipc::Channel;

use super::{authorized_dir, blocking};
use crate::error::CommandResult;
use crate::state::AppState;

/// The global hook files, and those of the project at `project` (the active or a recent one;
/// `None`: global only), with the status each has, then (while hooks are shared) the team hooks of
/// that chat. A hook over its scope's limit is listed with its scope; a file that is not a valid
/// hook is always listed.
#[tauri::command]
pub async fn hooks_list(state: State<'_, AppState>, project: Option<String>) -> CommandResult<HookListing> {
    list(&state, project).await
}

/// Creates (`draft.id` `None`) or edits a hook and approves the written version.
#[tauri::command]
pub async fn hook_save(state: State<'_, AppState>, draft: HookDraft) -> CommandResult<HookView> {
    save(&state, draft).await
}

/// Deletes the hook file `id` (the version `sha256`) and its approval. `notFound` when it is
/// gone, `conflict` when it changed.
#[tauri::command]
pub async fn hook_delete(state: State<'_, AppState>, id: String, sha256: String) -> CommandResult<()> {
    delete(&state, id, sha256).await
}

/// Deletes every hook the Hooks view lists for `project` (the active or a recent one; `None`:
/// global only): the global hooks, that project's and every invalid file, pending ones included,
/// with their approvals. Harness family hooks deleted this way are not seeded again at launch.
#[tauri::command]
pub async fn hooks_delete_all(state: State<'_, AppState>, project: Option<String>) -> CommandResult<HooksDeleted> {
    delete_all(&state, project).await
}

/// Approves the version `sha256` of hook `id`, a local file or (without one) a team hook.
/// `conflict` when it changed.
#[tauri::command]
pub async fn hook_approve(state: State<'_, AppState>, id: String, sha256: String) -> CommandResult<HookView> {
    approve(&state, id, sha256).await
}

/// Rejects the pending version `sha256` of hook `id`: a changed hook gets its approved version
/// back (returned), a never-approved one is deleted (`None`). `conflict` when the file changed.
#[tauri::command]
pub async fn hook_reject(state: State<'_, AppState>, id: String, sha256: String) -> CommandResult<Option<HookView>> {
    reject(&state, id, sha256).await
}

/// The Harness family hooks against the folder.
#[tauri::command]
pub async fn hooks_family(state: State<'_, AppState>) -> CommandResult<Vec<HookFamilyEntry>> {
    family(&state).await
}

/// Adds the missing and updates the outdated family hooks of `seeds` (approved), then lists the
/// global hooks.
#[tauri::command]
pub async fn hooks_family_apply(state: State<'_, AppState>, seeds: Vec<HookSeed>) -> CommandResult<HookListing> {
    family_apply(&state, seeds).await
}

/// Sends `changed` to `on_event` whenever the hooks folder or the approvals change, replacing the
/// previous listener.
#[tauri::command]
pub async fn hooks_listen(state: State<'_, AppState>, on_event: Channel<HooksEvent>) -> CommandResult<()> {
    state.hooks.listen(on_event);

    Ok(())
}

pub(crate) async fn list(state: &AppState, project: Option<String>) -> CommandResult<HookListing> {
    let root = match project {
        Some(project) => Some(project_root(state, project).await?),
        None => None,
    };

    listing(state, root).await
}

/// The listing with the global hooks and those of `root` (a project root), local then team.
pub(crate) async fn listing(state: &AppState, root: Option<String>) -> CommandResult<HookListing> {
    let capabilities = state.hooks.capabilities().await;

    if let Some(root) = &root {
        state.hooks.note_root(root);
    }

    let team = state.hooks.team_settings();
    let verified = root.as_deref().is_some_and(|root| state.hooks.root_verified(root));

    let listing = state
        .hooks
        .read(move |store| {
            Ok(app_listing(
                store,
                &TeamFolder::of(store),
                capabilities,
                root.as_deref(),
                team.as_ref(),
                verified,
            ))
        })
        .await?;

    Ok(listing)
}

/// The project root of `project`, the active or a recent project.
pub(crate) async fn project_root(state: &AppState, project: String) -> CommandResult<String> {
    let dir = authorized_dir(state, project).await?;

    blocking(move || {
        let root = project_identity(&dir)?;

        Ok(utf8(root.into_os_string().into_string())?)
    })
    .await
}

fn utf8(text: std::result::Result<String, std::ffi::OsString>) -> Result<String> {
    text.map_err(|_| BusinessError::InvalidInput("the project path is not valid UTF-8".to_owned()))
}

pub(crate) async fn save(state: &AppState, mut draft: HookDraft) -> CommandResult<HookView> {
    if draft.scope == HookScope::Project {
        draft.project = authorized_project(state, &draft).await?;
    }

    let capabilities = state.hooks.capabilities().await;

    let saved = state
        .hooks
        .write(move |store| store.save(&draft, HookAuthor::Desktop, capabilities, SystemTime::now()))
        .await?;

    tracing::info!(hook = saved.id, status = ?saved.status, "hook saved");

    Ok(saved)
}

/// The project folder of a project draft: the root the edited file already names, else a folder
/// of the active or a recent project.
async fn authorized_project(state: &AppState, draft: &HookDraft) -> CommandResult<Option<String>> {
    let Some(folder) = draft.project.clone().filter(|folder| !folder.is_empty()) else {
        return Ok(None);
    };

    if let Some(id) = draft.id.clone() {
        let capabilities = state.hooks.capabilities().await;
        let stored = state
            .hooks
            .read(move |store| Ok(store.get(&id, capabilities).ok().and_then(|view| view.hook?.project)))
            .await?;

        if stored.as_deref() == Some(folder.as_str()) {
            return Ok(Some(folder));
        }
    }

    let dir = authorized_dir(state, folder).await?;

    Ok(Some(utf8(dir.into_os_string().into_string())?))
}

pub(crate) async fn delete(state: &AppState, id: String, sha256: String) -> CommandResult<()> {
    let capabilities = state.hooks.capabilities().await;
    let deleted_id = id.clone();

    state
        .hooks
        .write(move |store| {
            store.get(&id, capabilities)?;
            store.delete(&id, HookAuthor::Desktop, Some(&sha256))
        })
        .await?;

    tracing::info!(hook = deleted_id, "hook deleted");

    Ok(())
}

pub(crate) async fn delete_all(state: &AppState, project: Option<String>) -> CommandResult<HooksDeleted> {
    let root = match project {
        Some(project) => Some(project_root(state, project).await?),
        None => None,
    };

    let deleted = state
        .hooks
        .write(move |store| store.delete_all(root.as_deref()))
        .await?;

    tracing::info!(deleted, "every listed hook deleted");

    Ok(HooksDeleted {
        deleted: u32::try_from(deleted).unwrap_or(u32::MAX),
    })
}

pub(crate) async fn approve(state: &AppState, id: String, sha256: String) -> CommandResult<HookView> {
    let capabilities = state.hooks.capabilities().await;
    let team = state.hooks.team_settings();

    let approved = state
        .hooks
        .write(move |store| match store.approve(&id, &sha256) {
            Ok(()) => store.get(&id, capabilities),
            // No local file: a team hook, while hooks are shared.
            Err(BusinessError::NotFound(missing)) => {
                let Some(team) = team else {
                    return Err(BusinessError::NotFound(missing));
                };

                let folder = TeamFolder::of(store);
                approve_team_hook(store, &folder, &id, &sha256)?;
                team_view(store, &folder, capabilities, team.trust, &id)
            }
            Err(error) => Err(error),
        })
        .await?;

    tracing::info!(hook = approved.id, "hook approved");

    Ok(approved)
}

async fn reject(state: &AppState, id: String, sha256: String) -> CommandResult<Option<HookView>> {
    let capabilities = state.hooks.capabilities().await;
    let rejected_id = id.clone();

    let restored = state
        .hooks
        .write(move |store| store.reject(&id, &sha256, capabilities))
        .await?;

    tracing::info!(
        hook = rejected_id,
        restored = restored.is_some(),
        "hook version rejected"
    );

    Ok(restored)
}

async fn family(state: &AppState) -> CommandResult<Vec<HookFamilyEntry>> {
    let capabilities = state.hooks.capabilities().await;
    let installed = state.hooks.installed().await;

    Ok(state
        .hooks
        .read(move |store| Ok(family_status(&store.list(capabilities), installed)))
        .await?)
}

async fn family_apply(state: &AppState, seeds: Vec<HookSeed>) -> CommandResult<HookListing> {
    let capabilities = state.hooks.capabilities().await;

    let changed = state
        .hooks
        .write(move |store| apply_family(store, &seeds, capabilities, SystemTime::now()))
        .await?;

    tracing::info!(changed = changed.len(), "harness family hooks applied");

    listing(state, None).await
}

pub mod team;
#[cfg(test)]
mod tests;
