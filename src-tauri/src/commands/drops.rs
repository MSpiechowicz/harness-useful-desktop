//! Receives the files dropped onto the window; see [`crate::drops`].

use harness_business::attachments::save_attachment;
use harness_business::model::{AttachedFile, DropEvent};
use tauri::State;
use tauri::ipc::Channel;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

/// Sends the window's drag and drop events to `on_drop`, replacing the previous listener: images
/// the user drops (read by the shell) and whether a drag is over the window.
#[tauri::command]
pub async fn drop_listen(state: State<'_, AppState>, on_drop: Channel<DropEvent>) -> CommandResult<()> {
    state.drops.set(on_drop);

    Ok(())
}

/// Saves a file the webview holds as bytes (base64 `data`: a picked file, pasted text) under the
/// app's attachment folder and returns where, so the message can name the path for the agent.
#[tauri::command]
pub async fn attachment_save(state: State<'_, AppState>, name: String, data: String) -> CommandResult<AttachedFile> {
    let root = state.attachments_dir.clone();

    blocking(move || Ok(save_attachment(&root, &name, &data)?)).await
}
