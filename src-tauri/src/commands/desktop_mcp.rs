//! The app's own MCP server (`harness-desktop mcp`, agents manage hooks through it) in a harness's
//! user MCP file, at the user's request from the "Recommended setup" step
//! (`setup_register_desktop_mcp`). The webview sends only the harness id: the file follows from
//! it and the entry runs this app's own program. Its state is part of `setup_status`
//! (`HarnessSetup.desktopMcp`).

use harness_business::model::{HarnessId, McpState};
use tauri::State;

use super::harness::{find, installed_binary};
use crate::desktop_mcp::register_in;
use crate::error::CommandResult;
use crate::state::AppState;

/// Adds the app's MCP entry to the user MCP file of `harness` (also where the user removed it
/// before, and in a dev build) and returns the file's state (`refused`, nothing written, when the
/// file cannot be changed safely; `manualEdit` with the entry to paste when it has comments).
/// `notFound` when the harness is not installed; `unsupported` when it has no user MCP file.
#[tauri::command]
pub async fn setup_register_desktop_mcp(state: State<'_, AppState>, harness: HarnessId) -> CommandResult<McpState> {
    register(&state, harness).await
}

async fn register(state: &AppState, harness: HarnessId) -> CommandResult<McpState> {
    find(state, harness)?;
    installed_binary(state, harness).await?;

    let registered = register_in(state, harness).await?;
    tracing::info!(
        ?harness,
        configured = matches!(registered, McpState::Configured { .. }),
        "desktop MCP registration finished"
    );

    Ok(registered)
}

#[cfg(test)]
mod tests {
    use harness_business::model::IpcErrorCode;
    use tauri::async_runtime::block_on;

    use super::*;
    use crate::test_support::{TempDir, cache_detection, home_state};

    #[test]
    fn a_harness_that_is_not_installed_is_refused() {
        let dir = TempDir::new("desktop-mcp-command");
        let state = home_state(&dir);
        cache_detection(&state, HarnessId::Pi, None);

        let error = block_on(register(&state, HarnessId::Pi)).expect_err("not installed");

        assert_eq!(error.code(), IpcErrorCode::NotFound);
        assert!(!state.env.home.join(".pi").exists(), "nothing is written");
    }
}
