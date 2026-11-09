//! What the page and the chat alerts tell each other (see `crate::chat_alerts`): the page listens
//! for toasts and notification clicks, and reports which chat is on screen and what the chats are
//! called.

use harness_business::model::{ChatAlertEvent, ChatAlertsView};
use tauri::State;
use tauri::ipc::Channel;

use crate::error::CommandResult;
use crate::state::AppState;

/// Sends the chat alert toasts and the clicks on chat notifications to `channel`, replacing the
/// previous listener (a page reload makes a new one).
#[tauri::command]
pub async fn chat_alerts_listen(state: State<'_, AppState>, channel: Channel<ChatAlertEvent>) -> CommandResult<()> {
    state.chat_alerts.listen(channel);

    Ok(())
}

/// Stores what the page shows: the chat on screen and the title of every open chat. At most 64
/// titles are kept, a key over 128 bytes is dropped, and a title is trimmed and cut at 200
/// characters. Anything over a limit is dropped, never an error.
#[tauri::command]
pub async fn chat_alerts_view(state: State<'_, AppState>, view: ChatAlertsView) -> CommandResult<()> {
    state.chat_alerts.set_view(view);

    Ok(())
}
