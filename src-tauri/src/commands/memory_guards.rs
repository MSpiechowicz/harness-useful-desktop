//! What agents may save to Harness Memory (`memory_guards_get`, `memory_guards_update`,
//! `memory_chat_guards_set`), the saves it held for the user's review (`memory_held_list`,
//! `memory_held_save`, `memory_held_skip`, `memory_held_listen`), and a chat's "Check again"
//! (`chat_memory_check_again`), and the one-time notice that saving became opt-in
//! (`memory_opt_in_notice_done`). See `harness_business::ecosystem::memory_guards` and
//! `memory_held` for the files and commands.

use std::path::Path;

use harness_business::ecosystem::{
    HeldSave, guarded_server, list_held, memory_data_dir, read_guards, save_held, saving_root_listed, skip_held,
    update_guards,
};
use harness_business::model::{
    AppPrefs, HeldItem, HeldSaved, MemoryChatGuards, MemoryChatGuardsSet, MemoryGuards, MemoryGuardsPatch,
    MemoryHeldEvent,
};
use harness_business::{BusinessError, Result};
use serde_json::Value;
use tauri::State;
use tauri::ipc::Channel;

use super::blocking;
use crate::error::CommandResult;
use crate::hooks::chat::check_again;
use crate::state::AppState;

/// The guards of every chat: saving off while nothing was saved. A file of the older version 1 is
/// moved to version 2 first, and a failure of that move is only logged: it reads as version 2 would
/// hold it (the pauses, saving off) with `migrationPending` set, and the move is tried again at
/// the next read. `protocol` for a guards file that cannot be read (Harness Memory then refuses
/// every save until it is saved again), `unsupported` while the data folder is unknown. Each kind's
/// `changedRepos` names the entries whose project key is no longer the one they had when turned
/// on, from one question to the installed server (empty when it cannot be asked).
#[tauri::command]
pub async fn memory_guards_get(state: State<'_, AppState>) -> CommandResult<MemoryGuards> {
    let data_dir = data_dir(&state)?;

    Ok(read_guards(&state.env, &state.runner, &state.memory_features, data_dir).await?)
}

/// Applies one change to the guards of every chat, to what is saved now, and returns them as
/// saved: pause or resume a kind, turn saving on for every project (`everyProject`), turn a
/// repository on (`addRepo`) or off again (`removeRepo`). Each repository is kept as its saving
/// root (a git worktree as its main repository), once, sorted. Changes made at once all apply.
/// Turning a repository on asks the installed server which project the folder given is and pins
/// the key it answers to the entry (turning a listed repository on again pins the key it has
/// now, which is how the user accepts a changed remote): when the server cannot be asked, or
/// cannot use the folder, nothing is written (`notFound`, `unsupported`, `process` or
/// `invalidInput` with the reason). `invalidInput` for a repository that is not an absolute folder
/// path, or more than 256 of them; `protocol` while the saved file cannot be read. The answer has
/// `changedRepos` as `memory_guards_get` does.
#[tauri::command]
pub async fn memory_guards_update(state: State<'_, AppState>, patch: MemoryGuardsPatch) -> CommandResult<MemoryGuards> {
    let data_dir = data_dir(&state)?;

    Ok(update_guards(&state.env, &state.runner, &state.memory_features, data_dir, patch).await?)
}

/// The saving root of `folder`: its project root, a git worktree as its main repository (the git
/// folder itself when that is not named `.git`). When the repositories saving is on for list a
/// checkout that keeps its git folder elsewhere and `folder` is one of its worktrees, the listed
/// checkout instead, so the UI's comparison with `onRepos` finds it. The answer changes when the
/// guards do, so the UI must read it again after a guards update. `None` for a folder that is gone
/// or not a folder.
#[tauri::command]
pub async fn memory_saving_root(state: State<'_, AppState>, folder: String) -> CommandResult<Option<String>> {
    let data_dir = memory_data_dir(&state.env);

    blocking(move || Ok(saving_root_listed(data_dir.as_deref(), Path::new(&folder)))).await
}

/// Records that the one-time notice about saving being opt-in was shown (`memoryOptInNoticeDone`
/// in the preferences, which a fresh install starts with), and returns the stored preferences. The
/// preferences saved by `prefs_set` never change it.
#[tauri::command]
pub async fn memory_opt_in_notice_done(state: State<'_, AppState>) -> CommandResult<AppPrefs> {
    let prefs = state.prefs.clone();

    blocking(move || Ok(prefs.with(|store| store.update(|prefs| prefs.memory_opt_in_notice_done = true))?)).await
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
