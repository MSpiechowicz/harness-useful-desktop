//! What agents may save to Harness Memory (`memory_guards_get`, `memory_guards_update`,
//! `memory_chat_guards_set`), the saves it held for the user's review (`memory_held_list`,
//! `memory_held_save`, `memory_held_skip`, `memory_held_listen`), and a chat's "Check again"
//! (`chat_memory_check_again`). See `harness_business::ecosystem::memory_guards` and
//! `memory_held` for the files and commands.

use harness_business::ecosystem::{
    HeldSave, guarded_server, list_held, memory_data_dir, read_global, save_held, skip_held, update_global,
};
use harness_business::model::{
    HeldItem, HeldSaved, MemoryChatGuards, MemoryChatGuardsSet, MemoryGuards, MemoryGuardsPatch, MemoryHeldEvent,
};
use harness_business::{BusinessError, Result};
use serde_json::Value;
use tauri::State;
use tauri::ipc::Channel;

use super::blocking;
use crate::error::CommandResult;
use crate::hooks::chat::check_again;
use crate::state::AppState;

/// The guards of every chat: none while nothing was saved. `protocol` for a guards file that
/// cannot be read (Harness Memory then refuses every save until it is saved again),
/// `unsupported` while the data folder is unknown.
#[tauri::command]
pub async fn memory_guards_get(state: State<'_, AppState>) -> CommandResult<MemoryGuards> {
    let data_dir = data_dir(&state)?;

    blocking(move || Ok(read_global(&data_dir)?)).await
}

/// Applies one change to the guards of every chat, to what is saved now, and returns them as
/// saved: pause or resume a kind, turn a repository off or on again (each repository kept as its
/// project root, once, sorted). Changes made at once all apply. `invalidInput` for a repository
/// that is not an absolute folder path, or more than 256 of them; `protocol` while the saved file
/// cannot be read.
#[tauri::command]
pub async fn memory_guards_update(state: State<'_, AppState>, patch: MemoryGuardsPatch) -> CommandResult<MemoryGuards> {
    let data_dir = data_dir(&state)?;

    blocking(move || Ok(update_global(&data_dir, &patch)?)).await
}

/// Saves the guards of chat `key` (a lowercase UUID, `invalidInput` otherwise) at `revision`.
/// They apply to its next save at once. A write whose revision is not higher than the last one
/// saved for the chat came late: it is ignored (and succeeds with `applied: false`), so it never
/// undoes a newer one. The answer names the last revision saved for the chat. The chat file keeps
/// the root of the chat's project, rebuilt from the running chat when the file cannot be read.
#[tauri::command]
pub async fn memory_chat_guards_set(
    state: State<'_, AppState>,
    key: String,
    guards: MemoryChatGuards,
    revision: u64,
) -> CommandResult<MemoryChatGuardsSet> {
    let data_dir = data_dir(&state)?;
    let writes = state.memory_chat_writes.clone();
    let project_root = state.hooks.memory_project(&key);

    blocking(move || Ok(writes.set_guards(&data_dir, &key, &guards, revision, project_root.as_deref())?)).await
}

/// The saves held for review of chat `chat_key`, newest first. `invalidInput` for a key that is
/// not a lowercase UUID, `notFound` while Harness Memory is not installed, `unsupported` while it
/// predates save guards.
#[tauri::command]
pub async fn memory_held_list(state: State<'_, AppState>, chat_key: String) -> CommandResult<Vec<HeldItem>> {
    let server = server(&state).await?;

    Ok(list_held(&state.env, &state.runner, &server, &chat_key).await?)
}

/// Saves held save `held_id`, as the agent sent it or with the user's `edited` draft: a note, or a
/// script that waits for the user's trust before it runs. With `revision` (the one the user
/// reviewed) it saves only that version. `notFound` when it is gone, `conflict` when the note it
/// changes was deleted (it stays held), `changed` when the agent held it again since the user saw
/// `revision` (it stays held, newer), `invalidInput` for a draft the server refuses.
#[tauri::command]
pub async fn memory_held_save(
    state: State<'_, AppState>,
    held_id: String,
    edited: Option<Value>,
    revision: Option<String>,
) -> CommandResult<HeldSaved> {
    let server = server(&state).await?;
    let save = HeldSave {
        held_id: &held_id,
        revision: revision.as_deref(),
        edited: edited.as_ref(),
    };

    Ok(save_held(&state.env, &state.runner, &server, save).await?)
}

/// Drops held save `held_id` without saving it. With `revision` (the one the user reviewed) it
/// drops only that version. `notFound` when it is gone, `changed` when the agent held it again
/// since the user saw `revision` (it stays held, newer), `invalidInput` for a revision that is not
/// 64 lowercase hex digits.
#[tauri::command]
pub async fn memory_held_skip(
    state: State<'_, AppState>,
    held_id: String,
    revision: Option<String>,
) -> CommandResult<()> {
    let server = server(&state).await?;

    Ok(skip_held(&state.env, &state.runner, &server, &held_id, revision.as_deref()).await?)
}

/// Sends `changed` to `on_event` whenever the held saves change, replacing the previous listener.
#[tauri::command]
pub async fn memory_held_listen(state: State<'_, AppState>, on_event: Channel<MemoryHeldEvent>) -> CommandResult<()> {
    state.memory_held.listen(on_event);

    Ok(())
}

/// Asks the agent of chat `agent_id` (its session key) to save what it learned now, as the
/// "Save what you learned" hook does after a turn. `invalidInput` while the chat is paused or
/// notes and scripts are both off for it, `notFound` for a chat that is not running.
#[tauri::command]
pub async fn chat_memory_check_again(state: State<'_, AppState>, agent_id: String) -> CommandResult<()> {
    Ok(check_again(&state, &agent_id).await?)
}

/// Harness Memory's data folder; `unsupported` while it is unknown (Windows without
/// `LOCALAPPDATA`).
fn data_dir(state: &AppState) -> Result<std::path::PathBuf> {
    memory_data_dir(&state.env).ok_or_else(|| {
        BusinessError::Unsupported("Harness Memory's data folder is unknown on this computer".to_owned())
    })
}

async fn server(state: &AppState) -> Result<std::path::PathBuf> {
    guarded_server(&state.env, &state.runner, &state.memory_features).await
}
