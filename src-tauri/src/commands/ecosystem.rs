//! Ecosystem commands: which companion tools are installed, and the Harness Dashboard's data
//! through its local, read-only MCP server (`harness-dashboard mcp`, pooled in [`AppState`]).

use harness_business::ecosystem;
use harness_business::model::{DashboardAvailability, DashboardQuery, DashboardResult, EcosystemStatus};
use tauri::State;
use tauri::async_runtime::block_on;

use super::blocking;
use crate::error::CommandResult;
use crate::state::AppState;

/// The dashboard, the skills and the sidebar with their detection on this machine.
#[tauri::command]
pub async fn ecosystem_status(state: State<'_, AppState>) -> CommandResult<EcosystemStatus> {
    let env = state.env.clone();
    let runner = state.runner;

    // Detection reads the OMP plugin registries besides probing the dashboard's version.
    blocking(move || Ok(block_on(ecosystem::ecosystem_status(&env, &runner)))).await
}

/// Whether `dashboard_query` can be answered (starts the dashboard's MCP server when needed).
#[tauri::command]
pub async fn dashboard_availability(state: State<'_, AppState>) -> CommandResult<DashboardAvailability> {
    Ok(state.dashboard.availability(&state.env).await)
}

/// Calls one dashboard tool with validated arguments; `notFound` when the dashboard is not
/// installed, `invalidInput` for arguments the tool does not accept.
#[tauri::command]
pub async fn dashboard_query(state: State<'_, AppState>, query: DashboardQuery) -> CommandResult<DashboardResult> {
    Ok(state.dashboard.query(&state.env, &query).await?)
}
