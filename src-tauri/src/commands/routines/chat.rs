//! The chat of an agent run in a window (`routine_run_attach`) and the folder global routines run
//! in (`routines_workspace`), see `crate::routines::relay`.

use harness_business::BusinessError;
use harness_business::model::{AgentEnvelope, AgentSessionInfo};
use tauri::State;
use tauri::ipc::Channel;

use super::super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

/// Shows the chat of the live agent run `run_id` (or of an ended one a window still shows) in
/// `on_event`: the events so far in order (the 3 startup events, then the newest, at most 20 000),
/// then the live ones. Answers the chat as `agent_start` does; answer its questions with
/// `agent_ui_respond`, close it with `agent_stop`. When the channel closes the agent keeps
/// running. `notFound` when the run has no chat (not an agent run, not started yet, or over).
#[tauri::command]
pub async fn routine_run_attach(
    state: State<'_, AppState>,
    run_id: String,
    on_event: Channel<AgentEnvelope>,
) -> CommandResult<AgentSessionInfo> {
    attach(&state, &run_id, on_event)
}

/// The folder global routines run in and keep their sessions in ("Global routines" in Sessions;
/// a valid `project` for the session commands), `null` while it does not exist.
#[tauri::command]
pub async fn routines_workspace(state: State<'_, AppState>) -> CommandResult<Option<String>> {
    let routines = state.routines.clone();

    blocking(move || {
        Ok(routines
            .workspace()
            .and_then(|folder| folder.into_os_string().into_string().ok()))
    })
    .await
}

pub(crate) fn attach(
    state: &AppState,
    run_id: &str,
    on_event: Channel<AgentEnvelope>,
) -> CommandResult<AgentSessionInfo> {
    let relay = state
        .routines
        .relays()
        .get(run_id)
        .ok_or_else(|| BusinessError::NotFound("the run's chat".to_owned()))?;

    // The chat must still run before a window is attached to it.
    let key = relay
        .key()
        .ok_or_else(|| BusinessError::NotFound("the run's chat".to_owned()))?;
    let info = state.agents.manager().info(&key)?;

    relay.attach(on_event)?;

    Ok(info)
}
