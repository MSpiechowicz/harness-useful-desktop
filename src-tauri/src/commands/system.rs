//! Commands that hand something to the OS: revealing a file in the file manager and opening a
//! link in the default browser. Both are limited by the rules in [`crate::access`].

use std::path::{Path, PathBuf};

use harness_business::BusinessError;
use tauri::State;

use super::blocking;
use super::projects::active_project;
use crate::access::{allowed_link, link_allowlist, reveal_target};
use crate::error::CommandResult;
use crate::state::AppState;

/// Opens the OS file manager with `path` selected. The path must exist and lie inside a harness
/// configuration directory (config root or agent dir of any registered harness) or the active
/// project; anything else is `forbidden`.
#[tauri::command]
pub async fn reveal_path(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    let active = active_project(&state).await?;
    let roots = reveal_roots(&state, active.as_deref());

    blocking(move || {
        let target = reveal_target(&roots, Path::new(&path))?;

        tauri_plugin_opener::reveal_item_in_dir(&target)
            .map_err(|error| BusinessError::Process(format!("cannot open the file manager: {error}")).into())
    })
    .await
}

/// Opens an allowlisted `https` link in the default browser; anything else is `forbidden`.
#[tauri::command]
pub async fn open_external(state: State<'_, AppState>, url: String) -> CommandResult<()> {
    let url = allowed_link(&link_allowlist(&state.registry), &url)?;

    blocking(move || {
        tauri_plugin_opener::open_url(url.as_str(), None::<&str>)
            .map_err(|error| BusinessError::Process(format!("cannot open the browser: {error}")).into())
    })
    .await
}

/// Directories whose contents may be revealed: each harness's config root and agent dir, plus the
/// active project.
fn reveal_roots(state: &AppState, active: Option<&Path>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    for harness in state.registry.all() {
        let paths = harness.paths(&state.env, active);
        roots.push(paths.config_root);
        roots.push(paths.agent_dir);
    }

    roots.extend(active.map(Path::to_path_buf));

    roots
}
