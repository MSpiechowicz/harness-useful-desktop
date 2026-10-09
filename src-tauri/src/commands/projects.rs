//! Project commands. New directories arrive only through the native folder picker; the webview can
//! re-open or forget only projects the app already knows.

use std::path::PathBuf;

use harness_business::model::ProjectInfo;
use harness_business::project::Projects;
use tauri::{AppHandle, State};

use super::blocking;
use crate::dialogs::{self, Pick};
use crate::error::CommandResult;
use crate::state::AppState;

/// Lets the user choose a project folder in a native dialog, validates it, makes it active and
/// remembers it (when recent projects are remembered). `None` when cancelled.
#[tauri::command]
pub async fn project_pick(app: AppHandle, state: State<'_, AppState>) -> CommandResult<Option<ProjectInfo>> {
    // Start where projects live: next to the most recent one, else the home directory.
    let recent = with_projects(&state, |projects| Ok(projects.recent())).await?;
    let start = recent
        .iter()
        .find(|project| project.exists)
        .and_then(|project| PathBuf::from(&project.path).parent().map(PathBuf::from))
        .unwrap_or_else(|| state.env.home.clone());

    let Some(picked) = dialogs::pick(&app, &state.dialog, Pick::Folder, "Open a project folder", Some(&start)).await?
    else {
        return Ok(None);
    };

    with_projects(&state, move |projects| projects.open(&picked).map(Some)).await
}

/// Re-opens the active project or a recent one; any other path is `forbidden`.
#[tauri::command]
pub async fn project_open(state: State<'_, AppState>, path: String) -> CommandResult<ProjectInfo> {
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
    with_projects(&state, move |projects| projects.forget(&path)).await
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
