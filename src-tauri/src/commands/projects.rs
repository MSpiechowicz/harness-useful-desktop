//! Project commands. New directories arrive only through the native folder picker; the webview can
//! re-open or forget only projects the app already knows. `dialog_pick_folder` only hands a picked
//! folder's path back.

use std::path::PathBuf;

use harness_business::BusinessError;
use harness_business::model::{ProjectInfo, ProjectScope};
use harness_business::project::{Projects, with_keys};
use tauri::{AppHandle, State};

use super::{blocking, saved_language};
use crate::dialogs::{self, Pick};
use crate::error::CommandResult;
use crate::state::AppState;

/// Lets the user choose a project folder in a native dialog, validates it, makes it active and
/// remembers it (when recent projects are remembered). `None` when cancelled.
#[tauri::command]
pub async fn project_pick(app: AppHandle, state: State<'_, AppState>) -> CommandResult<Option<ProjectInfo>> {
    // Start among the projects: given a folder, KDE's dialog opens its parent with that folder
    // highlighted, so passing the most recent project shows its siblings (other projects) rather than
    // the home directory and its loose files. Without one, fall back to the home directory.
    let recent = with_projects(&state, |projects| Ok(projects.recent())).await?;
    let start = recent
        .iter()
        .find(|project| project.exists)
        .map(|project| PathBuf::from(&project.path))
        .unwrap_or_else(|| state.env.home.clone());

    let title = crate::native_text::native_text(&saved_language(&state).await?).open_project_folder;

    let Some(picked) = dialogs::pick(&app, &state.dialog, Pick::Folder, title, Some(&start)).await? else {
        return Ok(None);
    };

    let opened = with_projects(&state, move |projects| projects.open(&picked)).await?;
    state.project_keys.forget(&opened.path);

    Ok(Some(opened))
}

/// Lets the user choose a folder in a native dialog, opened in the home folder, and returns its
/// path; `None` when cancelled. Nothing else happens: no project is opened or remembered (for
/// "Add repository…" in the memory guards). `invalidInput` for a path that is not text.
#[tauri::command]
pub async fn dialog_pick_folder(app: AppHandle, state: State<'_, AppState>) -> CommandResult<Option<String>> {
    let title = crate::native_text::native_text(&saved_language(&state).await?).choose_folder;

    let Some(picked) = dialogs::pick(&app, &state.dialog, Pick::Folder, title, Some(&state.env.home)).await? else {
        return Ok(None);
    };

    let path = picked
        .into_os_string()
        .into_string()
        .map_err(|_| BusinessError::InvalidInput("the folder's path is not valid text".to_owned()))?;

    Ok(Some(path))
}

/// Re-opens the active project or a recent one; any other path is `forbidden`.
#[tauri::command]
pub async fn project_open(state: State<'_, AppState>, path: String) -> CommandResult<ProjectInfo> {
    state.project_keys.forget(&path);

    with_projects(&state, move |projects| projects.reopen(&path)).await
}

/// Clears the active project.
#[tauri::command]
pub async fn project_close(state: State<'_, AppState>) -> CommandResult<()> {
    with_projects(&state, |projects| projects.close()).await
}

/// The recent projects, most recent first.
#[tauri::command]
pub async fn project_recent(state: State<'_, AppState>) -> CommandResult<Vec<ProjectInfo>> {
    with_projects(&state, |projects| Ok(projects.recent())).await
}

/// Removes a project from the recent list (and clears it when active); returns the new list.
#[tauri::command]
pub async fn project_forget(state: State<'_, AppState>, path: String) -> CommandResult<Vec<ProjectInfo>> {
    state.project_keys.forget(&path);

    with_projects(&state, move |projects| projects.forget(&path)).await
}

/// The active project, then the recent ones, each with its project root and Harness Memory
/// repository key (kept for the session; `null` when Harness Memory is missing, outdated or does
/// not answer). Never fails for a key.
#[tauri::command]
pub async fn project_scopes(state: State<'_, AppState>) -> CommandResult<Vec<ProjectScope>> {
    scopes(&state).await
}

pub(crate) async fn scopes(state: &AppState) -> CommandResult<Vec<ProjectScope>> {
    let scopes = with_projects(state, |projects| Ok(projects.scopes())).await?;

    Ok(with_keys(&state.env, &state.runner, &state.project_keys, scopes).await)
}

/// Runs `operation` on the stored projects in a blocking task.
async fn with_projects<T, F>(state: &AppState, operation: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce(Projects<'_>) -> harness_business::Result<T> + Send + 'static,
{
    let prefs = state.prefs.clone();
    let env = state.env.clone();

    blocking(move || Ok(prefs.with(|store| operation(Projects::new(store, &env)))?)).await
}

/// The active project, if any (for commands that scope access to it).
pub(super) async fn active_project(state: &AppState) -> CommandResult<Option<PathBuf>> {
    with_projects(state, |projects| Ok(projects.active())).await
}
