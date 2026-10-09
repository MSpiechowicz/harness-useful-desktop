//! App-level commands: build information and user preferences.

use harness_business::model::{AppInfo, AppPrefs};
use tauri::{AppHandle, State};

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

/// Product name, version, OS, architecture and build profile.
#[tauri::command]
pub async fn app_info(app: AppHandle) -> AppInfo {
    let package = app.package_info();

    AppInfo::current(package.name.clone(), package.version.to_string())
}

/// The stored preferences (defaults when none are stored or the file is corrupt).
#[tauri::command]
pub async fn prefs_get(state: State<'_, AppState>) -> CommandResult<AppPrefs> {
    let prefs = state.prefs.clone();

    blocking(move || Ok(prefs.with(|store| store.load()))).await
}

/// Validates, normalizes and stores the UI-managed fields of `prefs`; returns what was stored.
///
/// The server-managed fields (`activeProject`, `recentProjects`, `binaryOverrides`) keep their
/// stored values whatever the webview sends; the project and harness commands change them.
#[tauri::command]
pub async fn prefs_set(state: State<'_, AppState>, prefs: AppPrefs) -> CommandResult<AppPrefs> {
    let store = state.prefs.clone();

    blocking(move || Ok(store.with(|store| store.save_ui(&prefs))?)).await
}
